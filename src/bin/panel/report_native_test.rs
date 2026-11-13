//! Real report surfaces in the private Xvfb fixture, never the user's desktop.
//! Original keyboard identities exercise the report's press/release helper;
//! the appended hold phases send real X11 keys to this fixture's own window.
//! Pointer, IME and window routing traverse PanelApp's production handlers.
use super::*;
use crate::report_view::ReportSnapshot;
use suzaku_map::ime::gpu::SettingsCategory;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceId, ElementState, Ime, MouseButton},
    keyboard::{KeyCode, PhysicalKey},
    platform::x11::EventLoopBuilderExtX11,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        ConnectionExt, EventMask, InputFocus, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, KeyButMask,
        KeyPressEvent, MapState,
    },
    rust_connection::RustConnection,
};

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_report_details_are_read_only_and_require_presented_gestures() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    for name in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "SUZAKU_IME_CONFIG",
    ] {
        let path = std::path::PathBuf::from(std::env::var_os(name).expect("private test path"));
        assert!(path.starts_with(std::env::temp_dir()), "{name}");
        assert_ne!(
            path,
            std::env::temp_dir(),
            "a dedicated test path is required"
        );
    }
    assert_ne!(
        std::env::var("DISPLAY").unwrap().split('.').next().unwrap(),
        ":0"
    );
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    let mut probe = ReportProbe {
        proxy: events.create_proxy(),
        completed: false,
        held: None,
        initialized: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct ReportProbe {
    proxy: EventLoopProxy<PanelUserEvent>,
    completed: bool,
    held: Option<ReportHeldProbe>,
    initialized: bool,
}

struct ReportHeldProbe {
    app: PanelApp,
    connection: RustConnection,
    window: WindowId,
    started: Instant,
    stage: usize,
    case: usize,
    keys: Vec<(PhysicalKey, ElementState, bool)>,
    before: Option<EditingState>,
    device: Option<DeviceId>,
    received_keys: usize,
    received_repeats: usize,
}

#[derive(Debug, PartialEq)]
struct EditingState {
    seed: String,
    search: String,
    category: SettingsCategory,
    scroll: f32,
    search_focused: bool,
    keyboard_focus: Option<InteractionKind>,
    saved: app_state::PersistedDisplaySettings,
}

impl From<&PanelState> for EditingState {
    fn from(state: &PanelState) -> Self {
        Self {
            seed: state.chrome.seed_text.clone(),
            search: state.chrome.settings_search_query.clone(),
            category: state.chrome.settings_category,
            scroll: state.chrome.settings_scroll_offset,
            search_focused: state.chrome.settings_search_focused,
            keyboard_focus: state.chrome.settings_keyboard_focus,
            saved: app_state::PersistedDisplaySettings::from(&state.chrome),
        }
    }
}

fn assert_main_untouched(app: &PanelApp) {
    let panel = app.panel.as_ref().unwrap();
    assert!(
        !app.panel_visible,
        "reading a report must not show the main panel"
    );
    assert_eq!(panel.window.is_visible(), Some(false));
    assert!(
        panel.chrome.compact_mode,
        "reading a report must not unfold the main panel"
    );
    assert_eq!(panel.chrome.seed_text, "report fixture draft 日 {}");
    assert!(!panel.chrome.settings_open);
    assert!(!panel.chrome.llm_enabled);
}

fn present_report(state: &mut PanelState) {
    let requested = PhysicalSize::new(480, 320);
    let _ = state.window.request_inner_size(requested);
    let actual = state.window.inner_size();
    assert_eq!(
        actual, requested,
        "the private report viewport must be real"
    );
    state.resize(actual.width, actual.height);
    state
        .render()
        .expect("successfully present the actual report surface");
    assert!(
        state
            .report_view
            .as_ref()
            .unwrap()
            .test_button_rect(None)
            .is_some(),
        "only a successful report presentation publishes its own pointer targets"
    );
}

fn point(rect: [f32; 4]) -> (f32, f32) {
    (rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5)
}

fn config_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot inspect the private fixture configuration: {error}"),
    }
}

fn mouse(app: &mut PanelApp, events: &ActiveEventLoop, point: (f32, f32), state: ElementState) {
    let settings = app.settings.as_mut().unwrap();
    settings.is_focused = true;
    settings.cursor_position = Some(point);
    let window = settings.window.id();
    app.window_event(
        events,
        window,
        WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        },
    );
}

fn assert_read_only_inputs(app: &mut PanelApp, events: &ActiveEventLoop, before: &EditingState) {
    let window = app.settings.as_ref().unwrap().window.id();
    for event in [
        WindowEvent::Focused(true),
        WindowEvent::Ime(Ime::Enabled),
        WindowEvent::Ime(Ime::Preedit("readonly 拼".into(), Some((0, 3)))),
        WindowEvent::Ime(Ime::Commit("must not enter the search or draft".into())),
        WindowEvent::Ime(Ime::Preedit(String::new(), None)),
        WindowEvent::Ime(Ime::Disabled),
    ] {
        app.window_event(events, window, event);
        assert_eq!(&EditingState::from(app.settings.as_ref().unwrap()), before);
        assert_main_untouched(app);
    }
    let settings = app.settings.as_mut().unwrap();
    for key in [
        KeyCode::KeyA,
        KeyCode::Backspace,
        KeyCode::Enter,
        KeyCode::Tab,
        KeyCode::Space,
    ] {
        assert!(!settings.handle_report_key(PhysicalKey::Code(key), ElementState::Pressed, false));
        assert!(!settings.handle_report_key(PhysicalKey::Code(key), ElementState::Released, false));
    }
    assert_eq!(&EditingState::from(&*settings), before);
    assert!(!settings.text_input.composing());
}

fn assert_report_input_error(app: &mut PanelApp, events: &ActiveEventLoop, legacy: &str) {
    let window = app.settings.as_ref().unwrap().window.id();
    app.window_event(events, window, WindowEvent::Focused(true));
    let main_focused = app.panel.as_ref().unwrap().is_focused;
    let native_auto_shown = app.native_auto_shown;
    let settings = app.settings.as_mut().unwrap();
    let previous_feedback = settings.last_commit_feedback.replace(legacy.into());
    let previous_ticks = std::mem::replace(&mut settings.commit_feedback_ticks, 37);
    let before = EditingState::from(&*settings);
    present_report(settings);
    let snapshot = settings.view_snapshot();
    let (_, overlays) = settings.current_frame_with_snapshot(&snapshot);
    assert!(
        overlays.is_empty(),
        "reports must not expose earlier settings feedback"
    );
    let view = settings.report_view.as_ref().unwrap();
    let source = view.test_snapshot().clone();
    let page = (view.page_index(), view.page_count());
    let close = view.test_button_rect(None).unwrap();
    let buttons = [Some(false), Some(true), None].map(|key| view.test_button_rect(key));
    assert!(settings.report_view.as_mut().unwrap().press(
        [point(close).0, point(close).1],
        report_view::PointerSource::Mouse,
    ));

    let raw = "Font: Settings";
    app.user_event(events, PanelUserEvent::InputMethodError(raw.into()));
    let settings = app.settings.as_mut().unwrap();
    let (_, overlays) = settings.current_frame_with_snapshot(&snapshot);
    assert_eq!(
        overlays.len(),
        1,
        "a focused report must visibly show a newly delivered input error"
    );
    let expected = settings
        .chrome
        .ui_language
        .tr("Input method error: {}")
        .replace("{}", raw);
    assert_eq!(
        overlays[0]
            .atlas_glyphs
            .iter()
            .map(|glyph| glyph.ch)
            .collect::<String>(),
        expected
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>(),
        "a report must localize the wrapper and preserve opaque error parameters",
    );
    let bounds = overlays[0].quads[0].rect;
    assert!(
        bounds[1] + bounds[3] <= close[1],
        "error feedback must not cover report controls"
    );
    settings
        .render()
        .expect("present the report and its new error notice");
    assert!(
        settings.needs_periodic_frame(),
        "the report error must have a bounded visible lifetime"
    );
    assert_eq!(settings.last_commit_feedback.as_deref(), Some(legacy));
    assert_eq!(
        settings.commit_feedback_ticks, 37,
        "the report must not advance old settings feedback"
    );
    let view = settings.report_view.as_ref().unwrap();
    assert_eq!(view.test_snapshot(), &source);
    assert_eq!((view.page_index(), view.page_count()), page);
    assert_eq!(
        [Some(false), Some(true), None].map(|key| view.test_button_rect(key)),
        buttons
    );
    for _ in 0..180 {
        settings.advance_periodic_frame();
    }
    assert!(!settings.needs_periodic_frame());
    let (_, overlays) = settings.current_frame_with_snapshot(&snapshot);
    assert!(
        overlays.is_empty(),
        "an expired notice must neither remain nor reveal legacy feedback"
    );
    settings.render().unwrap();
    assert!(
        settings.report_view.as_mut().unwrap().release(
            [point(close).0, point(close).1],
            report_view::PointerSource::Mouse,
        ),
        "a report error must preserve the previously presented button identity"
    );
    assert_eq!(EditingState::from(&*settings), before);
    assert!(settings.is_focused);
    assert!(!settings.text_input.composing());
    assert_eq!(settings.last_commit_feedback.as_deref(), Some(legacy));
    assert_eq!(settings.commit_feedback_ticks, 37);
    settings.last_commit_feedback = previous_feedback;
    settings.commit_feedback_ticks = previous_ticks;
    assert_eq!(app.panel.as_ref().unwrap().is_focused, main_focused);
    assert_eq!(app.native_auto_shown, native_auto_shown);
    assert_main_untouched(app);
}

fn assert_small_report_input_error(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    requested: PhysicalSize<u32>,
    limited: bool,
    match_report_font: bool,
) {
    app.show_report_details(
        events,
        ReportSnapshot::InputMethodError(if limited {
            "日".repeat(700_000)
        } else {
            "short report".into()
        }),
    );
    let settings = app.settings.as_mut().unwrap();
    settings.chrome.ui_language = suzaku_map::ui::UiLanguage::English;
    settings.chrome.text_scale = DisplayTextScale::Large;
    let _ = settings.window.request_inner_size(requested);
    assert_eq!(settings.window.inner_size(), requested);
    settings.resize(requested.width, requested.height);
    settings.render().unwrap();
    let window = settings.window.id();
    app.window_event(events, window, WindowEvent::Focused(true));
    app.settings.as_mut().unwrap().render().unwrap();

    let settings = app.settings.as_mut().unwrap();
    let before = EditingState::from(&*settings);
    let legacy = (
        settings.last_commit_feedback.clone(),
        settings.commit_feedback_ticks,
    );
    let view = settings.report_view.as_ref().unwrap();
    let source = view.test_snapshot().clone();
    let page = (view.page_index(), view.page_count());
    let buttons = [Some(false), Some(true), None].map(|key| view.test_button_rect(key));
    let close = buttons[2].expect("the small report must have a presented close button");
    let warning = "Report exceeds the 2 MiB display limit; only a bounded preview is available.";
    let scene = settings.last_scene.as_ref().unwrap();
    assert_eq!(
        scene
            .text_sections
            .iter()
            .flat_map(|section| &section.layouts)
            .any(|layout| { !layout.truncated && layout.lines.join(" ") == warning }),
        limited,
        "the actual small report must retain its complete mandatory warning",
    );
    assert!(settings.report_view.as_mut().unwrap().press(
        [point(close).0, point(close).1],
        report_view::PointerSource::Mouse,
    ));

    app.user_event(events, PanelUserEvent::InputMethodError("IO".into()));
    let settings = app.settings.as_mut().unwrap();
    let snapshot = settings.view_snapshot();
    let (scene, overlays) = settings.current_frame_with_snapshot(&snapshot);
    assert_eq!(
        overlays.len(),
        1,
        "a new error must be visible at {requested:?}, warning={limited}",
    );
    let visible = overlays[0]
        .atlas_glyphs
        .iter()
        .map(|glyph| glyph.ch)
        .collect::<String>();
    assert!(
        visible.contains("IO"),
        "the small-report error payload must have actual visible glyphs: {visible:?}",
    );
    let (_, body) = settings
        .report_view
        .as_ref()
        .unwrap()
        .input_method_error_feedback(settings.chrome.ui_language)
        .unwrap();
    assert!(!overlays[0].atlas_glyphs.is_empty());
    for glyph in &overlays[0].atlas_glyphs {
        assert!(glyph.rect[0] >= body[0] - 0.01);
        assert!(glyph.rect[1] >= body[1] - 0.01);
        assert!(glyph.rect[0] + glyph.rect[2] <= body[0] + body[2] + 0.01);
        assert!(glyph.rect[1] + glyph.rect[3] <= body[1] + body[3] + 0.01);
    }
    if match_report_font {
        let reference = scene
            .atlas_glyphs
            .iter()
            .find(|glyph| {
                glyph.ch == 'I'
                    && glyph.rect[0] >= body[0] - 0.01
                    && glyph.rect[1] >= body[1] - 0.01
                    && glyph.rect[1] + glyph.rect[3] <= body[1] + body[3] + 0.01
            })
            .expect("the actual report body must expose its selected-size reference glyph");
        let notice = overlays[0]
            .atlas_glyphs
            .iter()
            .find(|glyph| glyph.ch == reference.ch)
            .expect("the notice wrapper must have the same reference character");
        assert!(settings.font_atlas.uses_runtime_font);
        let reference_ink = settings.font_atlas.quad_for(reference).0;
        let notice_ink = settings.font_atlas.quad_for(notice).0;
        for dimension in [2, 3] {
            assert!(reference.rect[dimension] > 0.0);
            assert!(reference_ink[dimension] > 0.0);
            assert!(
                (notice.rect[dimension] - reference.rect[dimension]).abs() < 0.01,
                "the notice must retain the actual report glyph size at {requested:?}: notice={notice:?}, reference={reference:?}",
            );
            assert!(
                (notice_ink[dimension] - reference_ink[dimension]).abs() < 0.01,
                "the notice must retain the actual rasterized report glyph size at {requested:?}: notice={notice_ink:?}, reference={reference_ink:?}",
            );
        }
    }
    settings
        .render()
        .expect("present the small report and visible error");
    let view = settings.report_view.as_ref().unwrap();
    assert_eq!(view.test_snapshot(), &source);
    assert_eq!((view.page_index(), view.page_count()), page);
    assert_eq!(
        [Some(false), Some(true), None].map(|key| view.test_button_rect(key)),
        buttons
    );
    assert_eq!(EditingState::from(&*settings), before);
    assert_eq!(
        (
            settings.last_commit_feedback.clone(),
            settings.commit_feedback_ticks
        ),
        legacy
    );
    for _ in 0..180 {
        settings.advance_periodic_frame();
    }
    assert!(!settings.needs_periodic_frame());
    let (_, overlays) = settings.current_frame_with_snapshot(&snapshot);
    assert!(overlays.is_empty());
    settings.render().unwrap();
    assert!(
        settings.report_view.as_mut().unwrap().release(
            [point(close).0, point(close).1],
            report_view::PointerSource::Mouse,
        ),
        "a small-report error must preserve the acquired close-button identity"
    );
    assert_eq!(EditingState::from(&*settings), before);
    assert_eq!(
        (
            settings.last_commit_feedback.clone(),
            settings.commit_feedback_ticks
        ),
        legacy
    );
    app.close_report_details();
    assert_main_untouched(app);
}

fn assert_model_setup_entry(app: &mut PanelApp, events: &ActiveEventLoop) {
    let main_before = app.panel.as_ref().unwrap().chrome.clone();
    let settings_before = app.settings.as_ref().unwrap().chrome.clone();
    let focused_before = app.settings.as_ref().unwrap().is_focused;
    let panel = app.panel.as_mut().unwrap();
    panel.activate_interaction(InteractionKind::OpenModelSetup, false);
    assert!(panel.report_view.is_none(), "the guide belongs to settings");
    assert_eq!(panel.chrome, main_before);
    panel.chrome.compact_mode = false;
    panel.chrome.settings_open = true;

    let settings = app.settings.as_mut().unwrap();
    settings.chrome.settings_open = true;
    settings.chrome.settings_category = SettingsCategory::Model;
    settings.chrome.settings_search_query = "Configuration path and guide".into();
    settings.chrome.settings_search_focused = false;
    settings.chrome.settings_keyboard_focus = Some(InteractionKind::OpenModelSetup);
    settings.chrome.settings_scroll_offset = 0.0;
    settings.is_focused = true;
    settings.last_scene = None;
    settings.render().unwrap();
    let before_press = EditingState::from(&*settings);
    let rect = settings
        .last_scene
        .as_ref()
        .unwrap()
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::OpenModelSetup)
        .expect("the settings guide must expose a real presented target")
        .rect;
    mouse(app, events, point(rect), ElementState::Pressed);
    assert_eq!(
        EditingState::from(app.settings.as_ref().unwrap()),
        before_press
    );
    // Pointer activation has always left keyboard navigation before dispatching
    // the clicked action. Freeze every other field before the release opens it.
    let mut before = EditingState::from(app.settings.as_ref().unwrap());
    before.keyboard_focus = None;
    mouse(app, events, point(rect), ElementState::Released);
    let settings = app.settings.as_ref().unwrap();
    assert!(settings.report_view.is_some());
    assert!(settings.report_return_to_settings);
    assert_eq!(EditingState::from(settings), before);
    let window = settings.window.id();
    assert!(!app.settings.as_mut().unwrap().sync_text_input_permission());
    app.window_event(
        events,
        window,
        WindowEvent::Ime(Ime::Commit("must not edit a model setting or draft".into())),
    );
    assert_eq!(EditingState::from(app.settings.as_ref().unwrap()), before);
    app.close_report_details();
    assert_eq!(EditingState::from(app.settings.as_ref().unwrap()), before);

    // This follows the same key activation helper as the real settings event.
    // Its post-activation focus reveal must leave the return navigation intact.
    let settings = app.settings.as_mut().unwrap();
    settings.chrome.settings_keyboard_focus = Some(InteractionKind::OpenModelSetup);
    let keyboard_before = EditingState::from(&*settings);
    settings.handle_settings_key(
        &winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter),
        None,
        false,
    );
    assert!(settings.report_view.is_some());
    assert_eq!(EditingState::from(&*settings), keyboard_before);
    // PanelApp normally mirrors the finished keyboard event through this helper.
    let chrome = settings.chrome.clone();
    assert!(!app.panel.as_mut().unwrap().adopt_settings_from(&chrome));
    app.close_report_details();
    assert_eq!(
        EditingState::from(app.settings.as_ref().unwrap()),
        keyboard_before
    );
    assert!(!app.panel.as_ref().unwrap().chrome.llm_enabled);
    assert!(!app.panel_visible);

    app.panel.as_mut().unwrap().chrome = main_before;
    let settings = app.settings.as_mut().unwrap();
    settings.chrome = settings_before;
    settings.is_focused = focused_before;
    settings.invalidate_pointer_scene();
    settings.sync_text_input_state();
}

impl ReportHeldProbe {
    fn new(mut app: PanelApp, events: &ActiveEventLoop) -> Self {
        let started = Instant::now();
        let panel = app.panel.as_mut().unwrap();
        // The old synchronous checks are complete. Build an internally
        // consistent settings fixture without showing/unfolding the main window.
        panel.chrome.compact_mode = false;
        panel.chrome.settings_open = true;
        let chrome = panel.chrome.clone();
        let window = Arc::new(
            events
                .create_window(settings_window_attributes(&chrome))
                .unwrap(),
        );
        let mut settings = pollster::block_on(PanelState::new_settings(window, chrome)).unwrap();
        settings.engine.configure_prediction(None);
        finish_settings_window_creation(&settings.window, &panel.window, events);
        let window = settings.window.id();
        app.settings = Some(settings);
        let (connection, _) = x11rb::connect(None).unwrap();
        Self {
            app,
            connection,
            window,
            started,
            stage: 0,
            case: 0,
            keys: Vec::new(),
            before: None,
            device: None,
            received_keys: 0,
            received_repeats: 0,
        }
    }

    fn xid(&self) -> u32 {
        match self
            .app
            .settings
            .as_ref()
            .unwrap()
            .window
            .window_handle()
            .unwrap()
            .as_raw()
        {
            RawWindowHandle::Xlib(handle) => handle.window as u32,
            _ => panic!("private report hold fixture requires its own X11 window"),
        }
    }

    fn viewable(&self) -> bool {
        self.connection
            .get_window_attributes(self.xid())
            .unwrap()
            .reply()
            .unwrap()
            .map_state
            == MapState::VIEWABLE
    }

    fn focused(&self) -> bool {
        self.connection
            .get_input_focus()
            .unwrap()
            .reply()
            .unwrap()
            .focus
            == self.xid()
    }

    fn send_transition(&self, keysym: u32, response_type: u8) {
        let setup = self.connection.setup();
        let mapping = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap();
        let width = usize::from(mapping.keysyms_per_keycode);
        let code = mapping
            .keysyms
            .chunks(width)
            .position(|row| row[0] == keysym)
            .expect("private Xvfb key map contains the requested key") as u8
            + setup.min_keycode;
        let window = self.xid();
        let mask = if response_type == KEY_PRESS_EVENT {
            EventMask::KEY_PRESS
        } else {
            EventMask::KEY_RELEASE
        };
        self.connection
            .send_event(
                false,
                window,
                mask,
                KeyPressEvent {
                    response_type,
                    detail: code,
                    sequence: 0,
                    time: x11rb::CURRENT_TIME,
                    root: setup.roots[0].root,
                    event: window,
                    child: 0,
                    root_x: 0,
                    root_y: 0,
                    event_x: 0,
                    event_y: 0,
                    state: KeyButMask::default(),
                    same_screen: true,
                },
            )
            .unwrap()
            .check()
            .unwrap();
        self.connection.flush().unwrap();
    }

    fn key(&self) -> PhysicalKey {
        PhysicalKey::Code(if self.case == 0 {
            KeyCode::Backspace
        } else {
            KeyCode::KeyA
        })
    }

    fn keysym(&self) -> u32 {
        if self.case == 0 {
            0xff08
        } else {
            u32::from('a')
        }
    }

    fn open_report(&mut self, events: &ActiveEventLoop) {
        self.before = Some(EditingState::from(self.app.settings.as_ref().unwrap()));
        let raw = (0..160)
            .map(|line| format!("private held-key report line {line:04}\n"))
            .collect::<String>();
        self.app
            .show_report_details(events, ReportSnapshot::InputMethodError(raw));
        let settings = self.app.settings.as_mut().unwrap();
        assert_eq!(settings.window.id(), self.window);
        assert!(settings.report_return_to_settings);
        present_report(settings);
        assert!(settings.report_view.as_ref().unwrap().page_count() > 3);
        assert_eq!(
            EditingState::from(&*settings),
            *self.before.as_ref().unwrap()
        );
    }

    fn prepare_case(&mut self, events: &ActiveEventLoop) {
        self.keys.clear();
        let settings = self.app.settings.as_mut().unwrap();
        assert!(settings.report_view.is_none());
        settings.chrome.settings_search_query = "retained query".into();
        settings.chrome.settings_search_focused = true;
        settings.chrome.settings_keyboard_focus = Some(InteractionKind::SettingsSearchInput);
        settings.chrome.settings_scroll_offset = 0.0;
        settings.sync_text_input_state();
        if self.case == 2 {
            // A real rejected recording press owns A before the report exists.
            settings
                .chrome
                .toggle_shortcut_recording(suzaku_map::ime::shortcuts::ShortcutAction::Previous);
            settings.sync_text_input_state();
        } else {
            self.open_report(events);
        }
        self.send_transition(self.keysym(), KEY_PRESS_EVENT);
        self.stage = 2;
    }

    fn close_presented_report(&mut self, events: &ActiveEventLoop) {
        let settings = self.app.settings.as_mut().unwrap();
        present_report(settings);
        let close = point(
            settings
                .report_view
                .as_ref()
                .unwrap()
                .test_button_rect(None)
                .unwrap(),
        );
        mouse(&mut self.app, events, close, ElementState::Pressed);
        assert!(self.app.settings.as_ref().unwrap().report_view.is_some());
        mouse(&mut self.app, events, close, ElementState::Released);
        let settings = self.app.settings.as_ref().unwrap();
        assert_eq!(settings.window.id(), self.window);
        assert!(settings.report_view.is_none());
        assert_eq!(EditingState::from(settings), *self.before.as_ref().unwrap());
    }

    fn assert_restored_query(&self) {
        let settings = self.app.settings.as_ref().unwrap();
        assert_eq!(settings.window.id(), self.window);
        assert!(settings.report_view.is_none());
        assert_eq!(EditingState::from(settings), *self.before.as_ref().unwrap());
        assert!(settings.chrome.settings_search_focused);
    }

    fn fresh_edit(&mut self) {
        self.send_transition(self.keysym(), KEY_PRESS_EVENT);
        self.send_transition(self.keysym(), KEY_PRESS_EVENT);
        self.send_transition(self.keysym(), KEY_RELEASE_EVENT);
        self.stage = 6;
    }

    fn advance(&mut self, events: &ActiveEventLoop) -> bool {
        assert!(
            self.started.elapsed() < Duration::from_secs(20),
            "report hold fixture stuck at case {} stage {} with {:?}",
            self.case,
            self.stage,
            self.keys
        );
        events.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(15),
        ));
        match self.stage {
            0 if self.viewable() => {
                self.connection
                    .set_input_focus(InputFocus::PARENT, self.xid(), x11rb::CURRENT_TIME)
                    .unwrap()
                    .check()
                    .unwrap();
                self.connection.flush().unwrap();
                self.stage = 1;
            }
            1 if self.focused() && self.app.settings.as_ref().unwrap().is_focused => {
                self.prepare_case(events)
            }
            2 if self.keys.len() == 1 => {
                assert_eq!(self.keys[0], (self.key(), ElementState::Pressed, false));
                assert!(self.focused());
                if self.case == 2 {
                    let settings = self.app.settings.as_mut().unwrap();
                    assert!(settings.chrome.shortcut_recording.is_some());
                    assert!(!settings.chrome.shortcut_recording_error.is_empty());
                    assert_eq!(settings.chrome.settings_search_query, "retained query");
                    settings.chrome.settings_search_focused = true;
                    settings.chrome.settings_keyboard_focus =
                        Some(InteractionKind::SettingsSearchInput);
                    self.open_report(events);
                    // Neither entering nor replacing details may release the
                    // recording-owned key. No intervening repeat can re-own it.
                    self.app.show_report_details(
                        events,
                        ReportSnapshot::InputMethodError("replacement report".repeat(80)),
                    );
                    present_report(self.app.settings.as_mut().unwrap());
                }
                if self.case == 3 {
                    self.send_transition(self.keysym(), KEY_RELEASE_EVENT);
                    self.stage = 3;
                } else {
                    self.close_presented_report(events);
                    self.send_transition(self.keysym(), KEY_PRESS_EVENT);
                    self.stage = 4;
                }
            }
            3 if self.keys.len() == 2 => {
                assert_eq!(self.keys[1], (self.key(), ElementState::Released, false));
                self.close_presented_report(events);
                self.fresh_edit();
            }
            4 if self.keys.len() == 2 => {
                assert_eq!(
                    self.keys[1],
                    (self.key(), ElementState::Pressed, true),
                    "post-close second press must be a real unreleased winit repeat"
                );
                self.assert_restored_query();
                self.send_transition(self.keysym(), KEY_RELEASE_EVENT);
                self.stage = 5;
            }
            5 if self.keys.len() == 3 => {
                assert_eq!(self.keys[2], (self.key(), ElementState::Released, false));
                self.assert_restored_query();
                self.fresh_edit();
            }
            6 if self.keys.len() == if self.case == 3 { 5 } else { 6 } => {
                let start = if self.case == 3 { 2 } else { 3 };
                assert_eq!(self.keys[start], (self.key(), ElementState::Pressed, false));
                assert_eq!(
                    self.keys[start + 1],
                    (self.key(), ElementState::Pressed, true)
                );
                assert_eq!(
                    self.keys[start + 2],
                    (self.key(), ElementState::Released, false)
                );
                let settings = self.app.settings.as_ref().unwrap();
                let mut expected = self.before.as_ref().unwrap().search.clone();
                if self.case == 0 {
                    expected.pop();
                    expected.pop();
                } else {
                    expected.push_str("aa");
                }
                assert_eq!(
                    settings.chrome.settings_search_query, expected,
                    "fresh ordinary editing and its repeat must remain enabled"
                );
                assert_eq!(
                    settings.chrome.seed_text,
                    self.before.as_ref().unwrap().seed
                );
                self.case += 1;
                if self.case < 4 {
                    self.prepare_case(events);
                } else {
                    self.keys.clear();
                    self.open_report(events);
                    self.send_transition(0xff56, KEY_PRESS_EVENT); // PageDown
                    self.stage = 7;
                }
            }
            7 if self.keys.len() == 1 => {
                assert_eq!(
                    self.keys[0],
                    (
                        PhysicalKey::Code(KeyCode::PageDown),
                        ElementState::Pressed,
                        false
                    )
                );
                assert_eq!(
                    self.app
                        .settings
                        .as_ref()
                        .unwrap()
                        .report_view
                        .as_ref()
                        .unwrap()
                        .page_index(),
                    1
                );
                self.send_transition(0xff56, KEY_PRESS_EVENT);
                self.stage = 8;
            }
            8 if self.keys.len() == 2 => {
                assert_eq!(
                    self.keys[1],
                    (
                        PhysicalKey::Code(KeyCode::PageDown),
                        ElementState::Pressed,
                        true
                    )
                );
                assert_eq!(
                    self.app
                        .settings
                        .as_ref()
                        .unwrap()
                        .report_view
                        .as_ref()
                        .unwrap()
                        .page_index(),
                    2,
                    "report paging repeat must not be consumed as an editor hold"
                );
                self.send_transition(0xff56, KEY_RELEASE_EVENT);
                self.stage = 9;
            }
            9 if self.keys.len() == 3 => {
                assert_eq!(
                    self.keys[2],
                    (
                        PhysicalKey::Code(KeyCode::PageDown),
                        ElementState::Released,
                        false
                    )
                );
                self.send_transition(0xff1b, KEY_PRESS_EVENT);
                self.stage = 10;
            }
            10 if self.keys.len() == 4 => {
                assert_eq!(
                    self.keys[3],
                    (
                        PhysicalKey::Code(KeyCode::Escape),
                        ElementState::Pressed,
                        false
                    )
                );
                assert!(self.app.settings.as_ref().unwrap().report_view.is_some());
                self.send_transition(0xff1b, KEY_PRESS_EVENT);
                self.stage = 11;
            }
            11 if self.keys.len() == 5 => {
                assert_eq!(
                    self.keys[4],
                    (
                        PhysicalKey::Code(KeyCode::Escape),
                        ElementState::Pressed,
                        true
                    )
                );
                assert!(self.app.settings.as_ref().unwrap().report_view.is_some());
                self.send_transition(0xff1b, KEY_RELEASE_EVENT);
                self.stage = 12;
            }
            12 if self.keys.len() == 6 => {
                assert_eq!(
                    self.keys[5],
                    (
                        PhysicalKey::Code(KeyCode::Escape),
                        ElementState::Released,
                        false
                    )
                );
                self.assert_restored_query();
                assert_eq!(
                    self.received_keys, 29,
                    "all appended hold and paging vectors must receive their actual X11 key transitions"
                );
                assert_eq!(
                    self.received_repeats, 9,
                    "held and fresh editor repeats must be identified by winit, not fabricated KeyEvents"
                );
                assert_report_touch_device_boundaries(
                    &mut self.app,
                    events,
                    self.device.expect("actual X11 keyboard device identity"),
                );
                assert!(
                    self.started.elapsed() < Duration::from_secs(20),
                    "report hold/touch phases must finish within their original new-phase deadline"
                );
                println!(
                    "PASS: real X11 report holds stay read-only across presented mouse close until release; fresh editor repeats, recording ownership, in-report release, PageDown repeat and Escape release remain usable; controlled touch identities cannot cross devices"
                );
                return true;
            }
            _ => {}
        }
        false
    }
}

fn assert_report_touch_device_boundaries(
    app: &mut PanelApp,
    events: &ActiveEventLoop,
    actual: DeviceId,
) {
    use winit::event::{Touch, TouchPhase};
    let other = DeviceId::dummy();
    assert_ne!(
        actual, other,
        "controlled touch matrix needs two distinct legal opaque device identities, not fabricated IDs"
    );
    let window = app.settings.as_ref().unwrap().window.id();
    app.show_report_details(
        events,
        ReportSnapshot::InputMethodError("controlled touch identity report line\n".repeat(160)),
    );
    present_report(app.settings.as_mut().unwrap());
    let route = |app: &mut PanelApp, device_id, phase, id, at: (f32, f32)| {
        app.window_event(
            events,
            window,
            WindowEvent::Touch(Touch {
                device_id,
                phase,
                location: PhysicalPosition::new(f64::from(at.0), f64::from(at.1)),
                force: None,
                id,
            }),
        );
    };
    let next = point(
        app.settings
            .as_ref()
            .unwrap()
            .report_view
            .as_ref()
            .unwrap()
            .test_button_rect(Some(true))
            .unwrap(),
    );
    route(app, actual, TouchPhase::Started, 77, next);
    route(app, other, TouchPhase::Ended, 77, next);
    assert_eq!(
        app.settings
            .as_ref()
            .unwrap()
            .report_view
            .as_ref()
            .unwrap()
            .page_index(),
        0,
        "a different device must not finish the report Next gesture"
    );
    route(app, actual, TouchPhase::Ended, 77, next);
    assert_eq!(
        app.settings
            .as_ref()
            .unwrap()
            .report_view
            .as_ref()
            .unwrap()
            .page_index(),
        0,
        "a mismatched release must also cancel the old Next press"
    );
    route(app, actual, TouchPhase::Started, 78, next);
    route(app, actual, TouchPhase::Ended, 78, next);
    assert_eq!(
        app.settings
            .as_ref()
            .unwrap()
            .report_view
            .as_ref()
            .unwrap()
            .page_index(),
        1
    );
    present_report(app.settings.as_mut().unwrap());
    let close = point(
        app.settings
            .as_ref()
            .unwrap()
            .report_view
            .as_ref()
            .unwrap()
            .test_button_rect(None)
            .unwrap(),
    );
    route(app, actual, TouchPhase::Started, 77, close);
    route(app, other, TouchPhase::Ended, 77, close);
    assert!(
        app.settings.as_ref().unwrap().report_view.is_some(),
        "a different device must not finish report Close"
    );
    route(app, actual, TouchPhase::Ended, 77, close);
    assert!(
        app.settings.as_ref().unwrap().report_view.is_some(),
        "a mismatched release must also cancel the old Close press"
    );
    route(app, actual, TouchPhase::Started, 78, close);
    route(app, actual, TouchPhase::Ended, 78, close);
    assert_eq!(app.settings.as_ref().unwrap().window.id(), window);
    assert!(app.settings.as_ref().unwrap().report_view.is_none());
}

fn assert_report_occlusion_boundaries(app: &mut PanelApp, events: &ActiveEventLoop) {
    use winit::event::{MouseScrollDelta, Touch, TouchPhase};

    assert!(
        app.settings.is_none(),
        "this fixture owns only a temporary report"
    );
    let native_auto_shown = app.native_auto_shown;
    let config_before = [
        suzaku_map::platform::settings_host::display_settings_path(),
        std::path::PathBuf::from(std::env::var_os("SUZAKU_IME_CONFIG").unwrap()),
    ]
    .map(|path| {
        let bytes = config_bytes(&path);
        (path, bytes)
    });
    app.show_report_details(
        events,
        ReportSnapshot::InputMethodError(
            "occlusion report immutable source line 日 {}\n".repeat(160),
        ),
    );
    let window = app.settings.as_ref().unwrap().window.id();
    // A controlled lifecycle callee, not a claim of hardware focus acceptance.
    app.window_event(events, window, WindowEvent::Focused(true));
    present_report(app.settings.as_mut().unwrap());
    let settings = app.settings.as_ref().unwrap();
    assert!(!settings.report_return_to_settings);
    let before = EditingState::from(settings);
    let source = settings
        .report_view
        .as_ref()
        .unwrap()
        .test_snapshot()
        .clone();
    assert!(settings.report_view.as_ref().unwrap().page_count() > 2);

    let check = |app: &PanelApp, page: usize, phase: &str| {
        let settings = app.settings.as_ref().expect(phase);
        assert_eq!(settings.window.id(), window, "{phase}");
        let view = settings.report_view.as_ref().expect(phase);
        assert_eq!(view.page_index(), page, "{phase}");
        assert_eq!(view.test_snapshot(), &source, "immutable source: {phase}");
        assert_eq!(
            EditingState::from(settings),
            before,
            "read-only state: {phase}"
        );
        assert_main_untouched(app);
    };
    let route = |app: &mut PanelApp, touch: bool, at: (f32, f32), state: ElementState| {
        if touch {
            app.window_event(
                events,
                window,
                WindowEvent::Touch(Touch {
                    device_id: DeviceId::dummy(),
                    phase: if state == ElementState::Pressed {
                        TouchPhase::Started
                    } else {
                        TouchPhase::Ended
                    },
                    location: PhysicalPosition::new(f64::from(at.0), f64::from(at.1)),
                    force: None,
                    id: 907,
                }),
            );
        } else {
            mouse(app, events, at, state);
        }
    };
    let target = |app: &PanelApp, next: bool| {
        point(
            app.settings
                .as_ref()
                .unwrap()
                .report_view
                .as_ref()
                .unwrap()
                .test_button_rect(next.then_some(true))
                .unwrap(),
        )
    };

    // A valid press cannot survive a completely hidden interval, even when the
    // same immutable report is successfully presented before its tail release.
    for touch in [false, true] {
        for next in [true, false] {
            present_report(app.settings.as_mut().unwrap());
            let at = target(app, next);
            route(app, touch, at, ElementState::Pressed);
            app.window_event(events, window, WindowEvent::Occluded(true));
            app.window_event(events, window, WindowEvent::Occluded(false));
            present_report(app.settings.as_mut().unwrap());
            route(app, touch, at, ElementState::Released);
            check(app, 0, "a pre-occlusion report press must not revive");
        }
    }

    app.window_event(events, window, WindowEvent::Occluded(true));
    for touch in [false, true] {
        for next in [true, false] {
            // Deliberately rebuild a genuine surface while hidden: invalidating
            // old presses alone does not reject a new ghost press on this frame.
            present_report(app.settings.as_mut().unwrap());
            let at = target(app, next);
            route(app, touch, at, ElementState::Pressed);
            route(app, touch, at, ElementState::Released);
            check(
                app,
                0,
                "new mouse/touch report gestures while occluded are inert",
            );
        }
    }
    app.window_event(
        events,
        window,
        WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(0.0, -1.0),
            phase: TouchPhase::Moved,
        },
    );
    check(app, 0, "a report wheel while occluded must not page");

    let next_at = target(app, true);
    let close_at = target(app, false);
    app.window_event(events, window, WindowEvent::Occluded(false));
    assert!(
        app.settings
            .as_ref()
            .unwrap()
            .report_view
            .as_ref()
            .unwrap()
            .test_button_rect(None)
            .is_none(),
        "returning visibility is not a successful report presentation"
    );
    for touch in [false, true] {
        for at in [next_at, close_at] {
            route(app, touch, at, ElementState::Pressed);
            route(app, touch, at, ElementState::Released);
            check(
                app,
                0,
                "fresh report gestures must wait for the returned frame",
            );
        }
    }

    for (touch, expected_page) in [(false, 1), (true, 2)] {
        present_report(app.settings.as_mut().unwrap());
        let at = target(app, true);
        route(app, touch, at, ElementState::Pressed);
        route(app, touch, at, ElementState::Released);
        check(
            app,
            expected_page,
            "fresh presented report Next remains usable",
        );
    }
    present_report(app.settings.as_mut().unwrap());
    let at = target(app, false);
    route(app, false, at, ElementState::Pressed);
    check(app, 2, "fresh presented report Close waits for release");
    route(app, false, at, ElementState::Released);
    assert!(
        app.settings.is_none(),
        "fresh presented Close removes only this report"
    );
    assert_eq!(app.native_auto_shown, native_auto_shown);
    assert_main_untouched(app);
    for (path, bytes) in config_before {
        assert_eq!(
            config_bytes(&path),
            bytes,
            "reports must not persist settings: {path:?}"
        );
    }
    println!(
        "PASS: controlled report occlusion retires old gestures and rejects new mouse/touch/wheel actions despite hidden redraws; returned pointers wait for presentation, fresh Next/Close work, and source/settings/main remain unchanged"
    );
}

impl ApplicationHandler<PanelUserEvent> for ReportProbe {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        let window = Arc::new(events.create_window(panel_window_attributes()).unwrap());
        let mut panel = pollster::block_on(PanelState::new(window)).unwrap();
        panel.chrome.llm_enabled = false;
        panel.engine.configure_prediction(None);
        panel
            .chrome
            .set_seed_text("report fixture draft 日 {}".into());
        panel.chrome.compact_mode = true;
        panel.chrome.settings_open = false;
        panel.window.set_visible(false);
        let chrome = panel.chrome.clone();
        let mut app = PanelApp::new(self.proxy.clone(), None);
        // Do not call resumed/open_settings: neither tray/service ownership nor
        // a request to expand and show the independent main panel is in scope.
        app.panel = Some(panel);
        app.panel_visible = false;
        let window = Arc::new(
            events
                .create_window(settings_window_attributes(&chrome))
                .unwrap(),
        );
        let mut settings = pollster::block_on(PanelState::new_settings(window, chrome)).unwrap();
        settings.chrome.settings_search_query = "retained settings query".into();
        settings.chrome.settings_category = SettingsCategory::Appearance;
        settings.chrome.settings_scroll_offset = 137.0;
        settings.chrome.settings_search_focused = true;
        settings.chrome.settings_keyboard_focus = Some(InteractionKind::SettingsSearchClear);
        let before = EditingState::from(&settings);
        let original_window = settings.window.id();
        app.settings = Some(settings);
        let config_before = [
            suzaku_map::platform::settings_host::display_settings_path(),
            std::path::PathBuf::from(std::env::var_os("SUZAKU_IME_CONFIG").unwrap()),
        ]
        .map(|path| {
            let bytes = config_bytes(&path);
            (path, bytes)
        });
        assert_model_setup_entry(&mut app, events);

        let raw = (0..160)
            .map(|line| format!("/private/Settings_日/{line:04}/{{}}\n"))
            .collect::<String>();
        app.show_report_details(
            events,
            ReportSnapshot::Data(tray_data::DataReport::BackupSaved(raw)),
        );
        assert_main_untouched(&app);
        assert!(app.settings.as_ref().unwrap().report_return_to_settings);
        assert_eq!(app.settings.as_ref().unwrap().window.id(), original_window);
        let main_window = app.panel.as_ref().unwrap().window.id();
        app.window_event(events, main_window, WindowEvent::RedrawRequested);
        assert!(
            app.settings.as_ref().unwrap().report_view.is_some(),
            "a compact main-panel redraw must not dismiss the independent report"
        );
        assert_eq!(EditingState::from(app.settings.as_ref().unwrap()), before);
        assert_read_only_inputs(&mut app, events, &before);
        assert_report_input_error(&mut app, events, "Sent to active app: private earlier text");

        let settings = app.settings.as_mut().unwrap();
        present_report(settings);
        let view = settings.report_view.as_ref().unwrap();
        assert_eq!(view.page_index(), 0);
        assert!(
            view.page_count() > 2,
            "the complete synthetic report must be paginated"
        );
        let next = point(view.test_button_rect(Some(true)).expect("next page button"));
        settings.report_view.as_mut().unwrap().invalidate();
        let snapshot = settings.engine.snapshot();
        let _ = settings.current_frame_with_snapshot(&snapshot);
        assert!(
            settings
                .report_view
                .as_ref()
                .unwrap()
                .test_button_rect(None)
                .is_none()
        );
        mouse(&mut app, events, next, ElementState::Pressed);
        mouse(&mut app, events, next, ElementState::Released);
        assert_eq!(
            app.settings
                .as_ref()
                .unwrap()
                .report_view
                .as_ref()
                .unwrap()
                .page_index(),
            0
        );

        present_report(app.settings.as_mut().unwrap());
        mouse(&mut app, events, next, ElementState::Pressed);
        assert_eq!(
            app.settings
                .as_ref()
                .unwrap()
                .report_view
                .as_ref()
                .unwrap()
                .page_index(),
            0
        );
        // A redraw between press/release must neither execute nor forget it.
        app.settings.as_mut().unwrap().render().unwrap();
        mouse(&mut app, events, next, ElementState::Released);
        assert_eq!(
            app.settings
                .as_ref()
                .unwrap()
                .report_view
                .as_ref()
                .unwrap()
                .page_index(),
            1
        );

        let settings = app.settings.as_mut().unwrap();
        present_report(settings);
        let previous = point(
            settings
                .report_view
                .as_ref()
                .unwrap()
                .test_button_rect(Some(false))
                .unwrap(),
        );
        mouse(&mut app, events, previous, ElementState::Pressed);
        mouse(&mut app, events, (0.0, 0.0), ElementState::Released);
        assert_eq!(
            app.settings
                .as_ref()
                .unwrap()
                .report_view
                .as_ref()
                .unwrap()
                .page_index(),
            1
        );

        let settings = app.settings.as_mut().unwrap();
        assert!(!settings.handle_report_key(
            PhysicalKey::Code(KeyCode::PageDown),
            ElementState::Pressed,
            false
        ));
        assert_eq!(settings.report_view.as_ref().unwrap().page_index(), 2);
        assert!(!settings.handle_report_key(
            PhysicalKey::Code(KeyCode::PageDown),
            ElementState::Released,
            false
        ));
        assert_eq!(settings.report_view.as_ref().unwrap().page_index(), 2);
        assert!(!settings.handle_report_key(
            PhysicalKey::Code(KeyCode::Home),
            ElementState::Pressed,
            false
        ));
        assert_eq!(settings.report_view.as_ref().unwrap().page_index(), 0);
        assert_eq!(EditingState::from(&*settings), before);
        let escape = PhysicalKey::Code(KeyCode::Escape);
        assert!(!settings.handle_report_key(escape, ElementState::Pressed, true));
        assert!(
            settings.report_close_key.is_none(),
            "an Escape already held in another window must not arm report close"
        );
        assert!(!settings.handle_report_key(escape, ElementState::Released, false));
        assert!(!settings.handle_report_key(escape, ElementState::Pressed, false));
        let report_window = settings.window.id();
        app.window_event(events, report_window, WindowEvent::Focused(false));
        app.window_event(events, report_window, WindowEvent::Focused(true));
        let settings = app.settings.as_mut().unwrap();
        assert!(settings.report_close_key.is_none());
        assert!(settings.report_view.is_some());
        assert!(!settings.handle_report_key(escape, ElementState::Released, false));
        assert_eq!(EditingState::from(&*settings), before);
        assert!(!settings.handle_report_key(escape, ElementState::Pressed, false));
        assert!(!settings.handle_report_key(escape, ElementState::Pressed, true));
        assert!(!settings.handle_report_key(
            PhysicalKey::Code(KeyCode::KeyA),
            ElementState::Released,
            false
        ));
        assert!(
            settings.report_view.is_some(),
            "Escape press must not leak into restored settings"
        );
        assert!(settings.handle_report_key(escape, ElementState::Released, false));
        app.close_report_details();
        let settings = app.settings.as_ref().unwrap();
        assert_eq!(settings.window.id(), original_window);
        assert!(settings.report_view.is_none());
        assert_eq!(EditingState::from(settings), before);
        assert_main_untouched(&app);

        // Also freeze a real, valid nonzero scroll position. Keep the synthetic
        // zero-results/137 assertion above; this independently covers the next
        // ordinary GPU frame after returning, not just the close transition.
        let settings = app.settings.as_mut().unwrap();
        settings.chrome.settings_search_query.clear();
        settings.chrome.settings_keyboard_focus = None;
        let requested = PhysicalSize::new(400, 270);
        let _ = settings.window.request_inner_size(requested);
        assert_eq!(settings.window.inner_size(), requested);
        settings.resize(requested.width, requested.height);
        settings.render().unwrap();
        let maximum = settings
            .last_scene
            .as_ref()
            .unwrap()
            .settings_scroll_metadata
            .as_ref()
            .unwrap()
            .max_scroll_offset;
        assert!(
            maximum > 0.0,
            "the ordinary settings fixture must actually scroll"
        );
        settings.chrome.settings_scroll_offset = 137.0_f32.min(maximum);
        settings.render().unwrap();
        let visible_before = EditingState::from(&*settings);
        assert!(visible_before.scroll > 0.0);
        app.show_report_details(
            events,
            ReportSnapshot::InputMethodError("scroll return fixture".into()),
        );
        app.settings.as_mut().unwrap().render().unwrap();
        app.close_report_details();
        let settings = app.settings.as_mut().unwrap();
        assert_eq!(EditingState::from(&*settings), visible_before);
        settings.render().unwrap();
        assert_eq!(EditingState::from(&*settings), visible_before);
        assert_main_untouched(&app);

        // The mandatory preview warning must fit actual learned font metrics,
        // not just the deterministic built-in metrics used by pure layouts.
        for language in [
            suzaku_map::ui::UiLanguage::English,
            suzaku_map::ui::UiLanguage::French,
        ] {
            app.show_report_details(
                events,
                ReportSnapshot::InputMethodError("日".repeat(700_000)),
            );
            let settings = app.settings.as_mut().unwrap();
            settings.chrome.ui_language = language;
            settings.chrome.text_scale = DisplayTextScale::Large;
            let requested = PhysicalSize::new(400, 220);
            let _ = settings.window.request_inner_size(requested);
            assert_eq!(settings.window.inner_size(), requested);
            settings.resize(requested.width, requested.height);
            settings.render().unwrap();
            assert!(settings.font_atlas.uses_runtime_font);
            let scene = settings.last_scene.as_ref().unwrap();
            let notice = language
                .tr("Report exceeds the 2 MiB display limit; only a bounded preview is available.")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let layout = scene
                .text_sections
                .iter()
                .flat_map(|section| &section.layouts)
                .find(|layout| layout.lines.join(" ") == notice)
                .expect("the real minimum window must retain the complete preview warning");
            assert!(!layout.truncated);
            for glyph in &layout.atlas_glyphs {
                let clip = glyph.clip_rect.unwrap();
                assert!(glyph.rect[1] >= clip[1] - 0.01);
                assert!(glyph.rect[1] + glyph.rect[3] <= clip[1] + clip[3] + 0.01);
            }
            assert!(settings.report_view.as_ref().unwrap().page_count() > 1);
            assert!(scene.atlas_glyphs.iter().any(|glyph| glyph.ch == '日'));
            app.close_report_details();
            assert_main_untouched(&app);
        }

        assert_small_report_input_error(
            &mut app,
            events,
            PhysicalSize::new(640, 310),
            false,
            false,
        );
        assert_small_report_input_error(&mut app, events, PhysicalSize::new(640, 320), false, true);
        assert_small_report_input_error(&mut app, events, PhysicalSize::new(400, 220), true, false);

        // Without an existing settings window, closing details removes only its
        // temporary settings-kind surface, not the hidden/compact main panel.
        app.settings = None;
        app.show_report_details(
            events,
            ReportSnapshot::InputMethodError(
                "opaque diagnostic /private/Settings_日/{}".repeat(80),
            ),
        );
        assert!(!app.settings.as_ref().unwrap().report_return_to_settings);
        assert_main_untouched(&app);
        let settings = app.settings.as_mut().unwrap();
        let temporary_before = EditingState::from(&*settings);
        present_report(settings);
        assert_read_only_inputs(&mut app, events, &temporary_before);
        assert_report_input_error(
            &mut app,
            events,
            "Input method error: an older settings failure",
        );
        present_report(app.settings.as_mut().unwrap());
        let close = point(
            app.settings
                .as_ref()
                .unwrap()
                .report_view
                .as_ref()
                .unwrap()
                .test_button_rect(None)
                .unwrap(),
        );
        mouse(&mut app, events, close, ElementState::Pressed);
        assert!(app.settings.is_some());
        mouse(&mut app, events, close, ElementState::Released);
        assert!(app.settings.is_none());
        assert_main_untouched(&app);

        app.native_auto_shown = true;
        app.show_report_details(
            events,
            ReportSnapshot::InputMethodError("synthetic visibility-boundary report".into()),
        );
        assert!(
            app.native_auto_shown,
            "opening details must not change input visibility ownership"
        );
        let report_window = app.settings.as_ref().unwrap().window.id();
        app.apply_panel_visibility(false);
        assert_eq!(app.settings.as_ref().unwrap().window.id(), report_window);
        assert!(
            app.settings.as_ref().unwrap().report_view.is_some(),
            "automatic candidate hiding must not close a read-only report"
        );
        assert_main_untouched(&app);
        app.set_panel_visible(false);
        assert!(
            app.settings.is_none(),
            "an explicit user Hide must still close auxiliary report windows"
        );
        assert_main_untouched(&app);
        for (path, bytes) in config_before {
            assert_eq!(
                config_bytes(&path),
                bytes,
                "reading and dismissing reports must not persist settings: {path:?}"
            );
        }
        println!(
            "PASS: actual report surfaces paginate only presented gestures, require Escape release, consume IME/editing identities read-only, preserve settings and never show/unfold the main panel"
        );
        assert_report_occlusion_boundaries(&mut app, events);
        self.held = Some(ReportHeldProbe::new(app, events));
    }

    fn window_event(&mut self, events: &ActiveEventLoop, window: WindowId, event: WindowEvent) {
        let Some(held) = self.held.as_mut() else {
            return;
        };
        if window != held.window {
            return;
        }
        if let WindowEvent::KeyboardInput {
            device_id,
            event,
            is_synthetic: false,
            ..
        } = &event
        {
            held.keys
                .push((event.physical_key, event.state, event.repeat));
            held.received_keys += 1;
            held.received_repeats += usize::from(event.repeat);
            held.device.get_or_insert(*device_id);
        }
        held.app.window_event(events, window, event);
    }

    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        if let Some(held) = self.held.as_mut()
            && held.advance(events)
        {
            self.completed = true;
            events.exit();
        }
    }
}
