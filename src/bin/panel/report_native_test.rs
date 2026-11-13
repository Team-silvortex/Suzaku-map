//! Real report surfaces in the private Xvfb fixture, never the user's desktop.
//! Keyboard identities exercise the report's press/release helper directly;
//! pointer, IME and window routing traverse PanelApp's production handlers.
use super::*;
use crate::report_view::ReportSnapshot;
use suzaku_map::ime::gpu::SettingsCategory;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceId, ElementState, Ime, MouseButton},
    keyboard::{KeyCode, PhysicalKey},
    platform::x11::EventLoopBuilderExtX11,
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
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct ReportProbe {
    proxy: EventLoopProxy<PanelUserEvent>,
    completed: bool,
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

impl ApplicationHandler<PanelUserEvent> for ReportProbe {
    fn resumed(&mut self, events: &ActiveEventLoop) {
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
        self.completed = true;
        events.exit();
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
