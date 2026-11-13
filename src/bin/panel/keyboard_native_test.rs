//! Sends X11 key events only to this test's own windows on an isolated Xvfb.
//! No desktop capture, global hooks, IME switch or input injection into other apps.
use super::*;
use winit::{
    event::Ime,
    platform::x11::{EventLoopBuilderExtX11, WindowAttributesExtX11},
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

fn xid(window: &Window) -> u32 {
    match window.window_handle().unwrap().as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window as u32,
        _ => panic!("X11 test window required"),
    }
}

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_keyboard_editing_and_focus_return() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let (connection, _) = x11rb::connect(None).unwrap();
    let mut probe = KeyboardProbe {
        state: None,
        target: None,
        other: None,
        connection,
        step: 0,
        started: Instant::now(),
        key_events: 0,
        preedit_shortcut_events: None,
    };
    event_loop.run_app(&mut probe).unwrap();
    assert_eq!(probe.step, 11);
    assert!(
        probe.key_events >= 28,
        "test must traverse real winit key events"
    );
}

struct KeyboardProbe {
    state: Option<PanelState>,
    target: Option<Arc<Window>>,
    other: Option<Arc<Window>>,
    connection: RustConnection,
    step: usize,
    started: Instant,
    key_events: usize,
    preedit_shortcut_events: Option<usize>,
}

fn click_input(state: &mut PanelState) {
    crate::native_sync::present_test_frame(state);
    let scene = state
        .last_scene
        .as_ref()
        .expect("presented seed input fixture");
    let rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::SeedInput)
        .unwrap()
        .rect;
    state.cursor_position = Some((rect[0] + 8.0, rect[1] + rect[3] / 2.0));
    state.select_at_cursor();
}

fn assert_settings_pointer_waits_for_preedit_decision(
    state: &mut PanelState,
    events: &ActiveEventLoop,
) {
    use winit::event::{DeviceId, ElementState, MouseButton};

    let chrome = state.chrome.clone();
    let focused = state.is_focused;
    for constructor_succeeds in [false, true] {
        state.chrome = chrome.clone();
        state.chrome.compact_mode = false;
        state.chrome.settings_open = false;
        state.chrome.focus_input();
        state.is_focused = true;
        state.sync_text_input_state();
        handle_panel_window_event(
            state,
            events,
            WindowEvent::Ime(Ime::Preedit("unfinished 日本".into(), Some((11, 11)))),
            false,
        );
        assert!(state.text_input.composing());
        native_sync::present_test_frame(state);
        let rect = state
            .last_scene
            .as_ref()
            .unwrap()
            .interactive_targets
            .iter()
            .find(|target| target.kind == InteractionKind::SettingsToggle)
            .expect("the actual main frame must present its settings target")
            .rect;
        state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
        for transition in [ElementState::Pressed, ElementState::Released] {
            assert!(handle_panel_window_event(
                state,
                events,
                WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    state: transition,
                    button: MouseButton::Left,
                },
                false,
            ));
        }
        assert_eq!(state.settings_window_request, Some(true));
        assert!(
            state.text_input.composing(),
            "the real pointer dispatch must retain preedit until the constructor decision"
        );
        assert_eq!(state.chrome.seed_text, chrome.seed_text);

        // Constructor failure/success is synthetic here. The CPU auxiliary
        // regression separately executes the production PanelApp Err/Ok paths.
        let result = if constructor_succeeds {
            Ok(())
        } else {
            Err("synthetic auxiliary constructor failure")
        };
        assert_eq!(state.settings_window_request.take(), Some(true));
        if result.is_err() {
            state.chrome.settings_open = false;
        }
        assert_eq!(state.sync_text_input_permission(), !constructor_succeeds);
        assert_eq!(
            state.text_input.composing(),
            !constructor_succeeds,
            "failed settings must preserve preedit; only committed settings revoke it"
        );
    }

    state.chrome.settings_open = false;
    state.sync_text_input_state();
    state.handle_ime_event(Ime::Preedit("outer 日本".into(), Some((5, 5))));
    state.chrome.settings_open = true;
    state.input_dispatch_guard = true;
    assert!(!handle_panel_window_event(
        state,
        events,
        WindowEvent::RedrawRequested,
        false,
    ));
    assert!(
        state.text_input.composing(),
        "rejected reentrant dispatch must not synchronize an outer input context"
    );
    state.settings_window_request = Some(true);
    assert!(!handle_panel_window_event(
        state,
        events,
        WindowEvent::RedrawRequested,
        false,
    ));
    assert_eq!(state.settings_window_request, Some(true));
    state.input_dispatch_guard = false;
    assert!(handle_panel_window_event(
        state,
        events,
        WindowEvent::RedrawRequested,
        false,
    ));
    assert!(state.settings_window_request.is_none());
    assert!(
        !state.text_input.composing(),
        "a standalone PanelState must not defer permission across later ordinary events"
    );

    state.chrome = chrome;
    state.is_focused = focused;
    state.invalidate_pointer_scene();
    state.sync_text_input_state();
    println!(
        "PASS: real presented settings pointer dispatch retains preedit until a synthetic constructor decision, and reentrant dispatch cannot clear or consume an outer request"
    );
}

fn assert_local_backspace_boundary_preserves_work(state: &mut PanelState) {
    use std::sync::{Mutex, mpsc};
    use suzaku_map::ime::PredictionStatus;
    use suzaku_map::ime::gpu::TranslationPhase;
    use suzaku_map::prediction::{
        PredictionCancellation, PredictionError, PredictionProvider, PredictionRequest,
        PredictionResponse,
    };
    use winit::keyboard::{Key, ModifiersState, NamedKey};

    struct HeldPrediction {
        started: mpsc::Sender<PredictionCancellation>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl PredictionProvider for HeldPrediction {
        fn predict(
            &self,
            _: &PredictionRequest,
            cancellation: &PredictionCancellation,
        ) -> Result<PredictionResponse, PredictionError> {
            let _ = self.started.send(cancellation.clone());
            let _ = self
                .release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(2));
            cancellation.check()?;
            Err(PredictionError::NoCandidates)
        }
    }

    let original_chrome = state.chrome.clone();
    state.chrome.set_seed_text("hel".into());
    state.chrome.focus_input();
    state.refresh_seed();
    let completion = state
        .next_token_completions
        .iter()
        .position(|edit| edit.seed_after == "hello")
        .expect("local hello completion");
    state.select_next_token(completion);
    assert_eq!(state.chrome.seed_text, "hello");
    let history = state.completion_history.labels();
    assert!(!history.is_empty(), "adoption must establish undo history");

    let (started, received) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    state
        .engine
        .configure_prediction_provider(Some(Arc::new(HeldPrediction {
            started,
            release: Mutex::new(blocked),
        })));
    let cancellation = received
        .recv_timeout(Duration::from_secs(2))
        .expect("owned in-memory prediction started");
    assert_eq!(state.engine.prediction_status(), PredictionStatus::Pending);
    state.chrome.translation.phase = TranslationPhase::Ready;
    state.chrome.translation.text = "こんにちは".into();
    state.chrome.translation.message = "synthetic ready translation".into();
    let translation = state.chrome.translation.clone();
    let candidates = state.engine.candidates().to_vec();
    let snapshot = state.engine.snapshot();
    state.modifiers = ModifiersState::empty();
    state.handle_editing_key(&Key::Named(NamedKey::Home), None, false);
    assert_eq!(state.chrome.caret_index, 0);
    for repeat in [false, true] {
        state.handle_editing_key(&Key::Named(NamedKey::Backspace), None, repeat);
        assert_eq!(state.chrome.seed_text, "hello");
        assert_eq!(state.chrome.caret_index, 0);
        assert_eq!(
            state.completion_history.labels(),
            history,
            "a no-op Backspace must retain completion undo"
        );
        assert_eq!(state.engine.snapshot(), snapshot);
        assert_eq!(state.engine.candidates(), candidates);
        assert_eq!(state.engine.prediction_status(), PredictionStatus::Pending);
        assert!(
            !cancellation.is_cancelled(),
            "a no-op must not restart prediction"
        );
        assert_eq!(state.chrome.translation, translation);
    }

    // A real deletion still invalidates derived work and refreshes the engine.
    state.handle_editing_key(&Key::Named(NamedKey::End), None, false);
    state.handle_editing_key(&Key::Named(NamedKey::Backspace), None, false);
    assert_eq!(state.chrome.seed_text, "hell");
    assert_eq!(state.engine.snapshot().seed_text, "hell");
    assert!(state.completion_history.labels().is_empty());
    assert!(cancellation.is_cancelled());
    assert_eq!(state.chrome.translation.phase, TranslationPhase::Idle);
    state.engine.configure_prediction(None);
    let _ = release.send(());

    state.chrome.set_seed_text(String::new());
    state.refresh_seed();
    state.chrome.translation = translation.clone();
    state.backspace_seed();
    assert!(state.chrome.seed_text.is_empty());
    assert_eq!(state.chrome.translation, translation);
    assert!(state.engine.candidates().is_empty());
    assert_eq!(state.engine.prediction_status(), PredictionStatus::Disabled);

    state.cancel_translation();
    state.chrome = original_chrome;
    state.refresh_seed();
    println!(
        "PASS: local Home/empty-draft Backspace keeps completion undo, candidates, in-flight prediction and translation; real deletion still refreshes"
    );
}

impl KeyboardProbe {
    fn focus(&self, window: u32) {
        self.connection
            .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
    }

    fn focused(&self) -> u32 {
        self.connection
            .get_input_focus()
            .unwrap()
            .reply()
            .unwrap()
            .focus
    }

    fn send_keys(&self, keysyms: &[u32]) {
        self.send_keys_with_modifiers(keysyms, KeyButMask::default());
    }

    fn send_keys_with_modifiers(&self, keysyms: &[u32], modifiers: KeyButMask) {
        let setup = self.connection.setup();
        let mapping = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap();
        let width = usize::from(mapping.keysyms_per_keycode);
        let panel = xid(&self.state.as_ref().unwrap().window);
        for keysym in keysyms {
            let code = mapping
                .keysyms
                .chunks(width)
                .position(|row| row[0] == *keysym)
                .unwrap() as u8
                + setup.min_keycode;
            for (response_type, mask) in [
                (KEY_PRESS_EVENT, EventMask::KEY_PRESS),
                (KEY_RELEASE_EVENT, EventMask::KEY_RELEASE),
            ] {
                let event = KeyPressEvent {
                    response_type,
                    detail: code,
                    sequence: 0,
                    time: x11rb::CURRENT_TIME,
                    root: setup.roots[0].root,
                    event: panel,
                    child: 0,
                    root_x: 0,
                    root_y: 0,
                    event_x: 0,
                    event_y: 0,
                    state: modifiers,
                    same_screen: true,
                };
                self.connection
                    .send_event(false, panel, mask, event)
                    .unwrap()
                    .check()
                    .unwrap();
            }
        }
        self.connection.flush().unwrap();
    }
}

impl ApplicationHandler for KeyboardProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let target = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Synthetic target"))
                .unwrap(),
        );
        let other = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Synthetic other app"))
                .unwrap(),
        );
        self.focus(xid(&target));
        let window = Arc::new(
            event_loop
                .create_window(
                    panel_window_attributes()
                        .with_override_redirect(true)
                        .with_visible(true)
                        .with_active(false),
                )
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        state.runs_without_window_focus = true;
        state.chrome.llm_enabled = false;
        state.engine.configure_prediction(None);
        assert_settings_pointer_waits_for_preedit_decision(&mut state, event_loop);
        assert_local_backspace_boundary_preserves_work(&mut state);
        self.state = Some(state);
        self.target = Some(target);
        self.other = Some(other);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if window != state.window.id() {
            return;
        }
        let composing_shortcut = state.text_input.composing()
            && matches!(&event, WindowEvent::KeyboardInput { event, .. }
                if event.state == winit::event::ElementState::Pressed
                    && (event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::F1)
                        || matches!(&event.logical_key, winit::keyboard::Key::Character(text) if text == ",")));
        let before = composing_shortcut.then(|| state.chrome.clone());
        if matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.state == winit::event::ElementState::Pressed)
        {
            self.key_events += 1;
        }
        handle_panel_window_event(state, event_loop, event, false);
        if let Some(before) = before {
            assert!(state.settings_window_request.is_none());
            assert_eq!(state.chrome, before);
            assert!(state.text_input.composing());
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        assert!(
            self.started.elapsed() < Duration::from_secs(20),
            "keyboard probe stuck at step {}, seed={:?}",
            self.step,
            self.state.as_ref().map(|s| &s.chrome.seed_text)
        );
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(15),
        ));
        if self.started.elapsed() < Duration::from_millis(250) {
            return;
        }
        let target = xid(self.target.as_ref().unwrap());
        let panel = xid(&self.state.as_ref().unwrap().window);
        match self.step {
            0 => {
                assert_eq!(
                    self.focused(),
                    target,
                    "showing a companion must not take focus"
                );
                let state = self.state.as_mut().unwrap();
                native_sync::assert_native_view(state);
                state.chrome.active_input_mode = InputMode::Handwriting;
                click_input(state);
                self.step = 1;
            }
            1 | 3 | 5 if self.state.as_ref().unwrap().is_focused => {
                assert_eq!(self.focused(), panel);
                self.send_keys(&"v123 n i ".chars().map(u32::from).collect::<Vec<_>>());
                self.step += 1;
            }
            2 if self.preedit_shortcut_events.is_none()
                && self.state.as_ref().unwrap().chrome.seed_text == "v123 n i " =>
            {
                let state = self.state.as_mut().unwrap();
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Preedit("日本".into(), Some((3, 3)))),
                    false,
                );
                assert!(state.text_input.composing());
                self.preedit_shortcut_events = Some(self.key_events + 2);
                self.send_keys(&[0xffbe]);
                self.send_keys_with_modifiers(&[u32::from(',')], KeyButMask::CONTROL);
                self.step = 12;
            }
            12 if self.preedit_shortcut_events == Some(self.key_events) => {
                let state = self.state.as_ref().unwrap();
                assert!(state.text_input.composing());
                assert!(!state.chrome.settings_open);
                assert!(state.settings_window_request.is_none());
                assert_eq!(state.chrome.seed_text, "v123 n i ");
                self.step = 2;
            }
            2 | 4 | 6 if self.state.as_ref().unwrap().chrome.seed_text == "v123 n i " => {
                let step = self.step;
                let state = self.state.as_mut().unwrap();
                assert_eq!(
                    state.chrome.active_input_mode,
                    match step {
                        2 => InputMode::Handwriting,
                        4 => InputMode::Dictation,
                        _ => InputMode::VirtualKeyboard,
                    }
                );
                assert!(state.chrome.voice_state != VoiceCaptureState::Listening);
                assert!(!state.engine.candidates().is_empty());
                // Preedit is visible but never enters the candidate engine until committed.
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Preedit("日本".into(), Some((3, 3)))),
                    false,
                );
                assert_eq!(state.chrome.seed_text, "v123 n i ");
                assert!(state.text_input.composing());
                let scene = state.current_scene();
                let input = scene
                    .text_sections
                    .iter()
                    .flat_map(|section| &section.layouts)
                    .find(|layout| layout.role == suzaku_map::ime::gpu::TextRole::InputValue)
                    .unwrap();
                assert_eq!(
                    input.lines,
                    ["v123 n i 日本"],
                    "preedit is shown without losing draft spaces"
                );
                assert!(
                    input
                        .atlas_glyphs
                        .iter()
                        .all(|glyph| glyph.clip_rect.is_some())
                );
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Preedit("".into(), None)),
                    false,
                );
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Commit("日本語中文é".into())),
                    false,
                );
                assert_eq!(state.chrome.seed_text, "v123 n i 日本語中文é");
                assert_eq!(state.engine.snapshot().seed_text, state.chrome.seed_text);
                if step < 6 {
                    state.chrome.active_input_mode = if step == 2 {
                        InputMode::Dictation
                    } else {
                        InputMode::VirtualKeyboard
                    };
                    state.chrome.set_seed_text(String::new());
                    state.refresh_seed();
                } else {
                    self.send_keys(&[0xff0d]); // Enter finishes direct editing.
                }
                self.step += 1;
            }
            7 if !self.state.as_ref().unwrap().is_focused => {
                assert_eq!(
                    self.focused(),
                    target,
                    "Enter must return the borrowed focus"
                );
                let state = self.state.as_mut().unwrap();
                assert!(!state.chrome.input_focused);
                let before = state.chrome.seed_text.clone();
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Commit("late commit".into())),
                    false,
                );
                assert_eq!(state.chrome.seed_text, before);
                click_input(state);
                self.step = 8;
            }
            8 if self.state.as_ref().unwrap().is_focused => {
                let state = self.state.as_mut().unwrap();
                // Sending into the panel itself is forbidden, without touching native IPC.
                assert!(
                    !state.commit_selected_candidate_to_host(suzaku_map::ime::CommitOptions {
                        force: true
                    })
                );
                assert!(
                    state
                        .last_commit_feedback
                        .as_ref()
                        .unwrap()
                        .contains("target app")
                );
                click_input(state);
                self.send_keys(&[0xffbe]); // Focus-local F1 opens the shortcut reference.
                self.step = 9;
            }
            9 if self.state.as_ref().unwrap().chrome.settings_open => {
                let state = self.state.as_ref().unwrap();
                assert_eq!(
                    state.chrome.settings_category,
                    suzaku_map::ime::gpu::SettingsCategory::Shortcuts
                );
                assert_eq!(state.chrome.seed_text, "v123 n i 日本語中文é");
                self.send_keys(&[0xff1b]); // Esc closes the reference, not the draft.
                self.step = 10;
            }
            10 if !self.state.as_ref().unwrap().chrome.settings_open => {
                assert_eq!(
                    self.state.as_ref().unwrap().chrome.seed_text,
                    "v123 n i 日本語中文é"
                );
                let other = xid(self.other.as_ref().unwrap());
                self.focus(other);
                self.state.as_mut().unwrap().finish_text_editing();
                assert_eq!(
                    self.focused(),
                    other,
                    "a later user app switch wins over restoration"
                );
                self.step = 11;
                event_loop.exit();
            }
            _ => {}
        }
    }
}

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_custom_shortcut_recording_owns_window_events() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let config = std::env::var_os("XDG_CONFIG_HOME").expect("isolated config");
    assert!(std::path::Path::new(&config).starts_with(std::env::temp_dir()));
    assert!(std::env::var("DISPLAY").unwrap().split('.').next().unwrap() != ":0");
    let mut builder = EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let (connection, _) = x11rb::connect(None).unwrap();
    let mut probe = ShortcutRecordingProbe {
        state: None,
        other: None,
        connection,
        step: 0,
        started: Instant::now(),
        key_events: 0,
        completed: false,
    };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
    assert_eq!(
        probe.key_events, 6,
        "all six real winit key events must reach the dispatcher"
    );
}

struct ShortcutRecordingProbe {
    state: Option<PanelState>,
    other: Option<Arc<Window>>,
    connection: RustConnection,
    step: usize,
    started: Instant,
    key_events: usize,
    completed: bool,
}

impl ShortcutRecordingProbe {
    fn viewable(&self, window: &Window) -> bool {
        self.connection
            .get_window_attributes(xid(window))
            .unwrap()
            .reply()
            .unwrap()
            .map_state
            == MapState::VIEWABLE
    }

    fn send_key(&self, keysym: u32, modifiers: KeyButMask) {
        self.send_transition(keysym, modifiers, KEY_PRESS_EVENT);
        self.send_transition(keysym, modifiers, KEY_RELEASE_EVENT);
    }

    fn send_transition(&self, keysym: u32, modifiers: KeyButMask, response_type: u8) {
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
        let window = xid(&self.state.as_ref().unwrap().window);
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
                    state: modifiers,
                    same_screen: true,
                },
            )
            .unwrap()
            .check()
            .unwrap();
        self.connection.flush().unwrap();
    }

    fn click_recording(state: &mut PanelState, action: suzaku_map::ime::shortcuts::ShortcutAction) {
        state.chrome.settings_search_query = action.label().into();
        state.chrome.settings_scroll_offset = 0.0;
        state.last_scene = None;
        native_sync::present_test_frame(state);
        let scene = state.last_scene.as_ref().unwrap();
        let kind = InteractionKind::RecordShortcut(action);
        let rect = scene
            .interactive_targets
            .iter()
            .find(|target| target.kind == kind)
            .unwrap()
            .rect;
        state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
        state.begin_primary_press(false);
        state.complete_primary_release(false);
        assert_eq!(state.chrome.shortcut_recording, Some(action));
        state.sync_text_input_state();
    }

    fn assert_recording_untouched(&self) {
        use suzaku_map::ime::shortcuts::{ShortcutAction, ShortcutBindings};
        let state = self.state.as_ref().unwrap();
        assert_eq!(
            state.chrome.shortcut_recording,
            Some(ShortcutAction::Previous)
        );
        assert_eq!(state.chrome.shortcut_bindings, ShortcutBindings::default());
        assert_eq!(state.chrome.shortcut_edit_generation, 0);
        assert_eq!(state.chrome.seed_text, "private draft café 😀");
        assert_eq!(state.chrome.settings_search_query, "Previous");
        assert!(state.chrome.settings_open);
        assert_eq!(state.window_scale, 1.0);
        assert!(!state.chrome.shortcut_recording_error.is_empty());
    }
}

impl ApplicationHandler for ShortcutRecordingProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let other = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Private focus target"))
                .unwrap(),
        );
        let chrome = PanelChromeState {
            settings_open: true,
            settings_category: suzaku_map::ime::gpu::SettingsCategory::Shortcuts,
            seed_text: "private draft café 😀".into(),
            llm_enabled: false,
            ..Default::default()
        };
        let window = Arc::new(
            event_loop
                .create_window(settings_window_attributes(&chrome))
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new_settings(window, chrome)).unwrap();
        state.engine.configure_prediction(None);
        // Production settings attributes intentionally start hidden. Complete the
        // normal show/map lifecycle before requesting focus on this private window.
        finish_settings_window_creation(&state.window, &other, event_loop);
        state.window.request_redraw();
        self.state = Some(state);
        self.other = Some(other);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if window != state.window.id() {
            return;
        }
        if matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.state == winit::event::ElementState::Pressed)
        {
            self.key_events += 1;
        }
        handle_panel_window_event(state, event_loop, event, false);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        use suzaku_map::ime::shortcuts::ShortcutAction;
        assert!(
            self.started.elapsed() < Duration::from_secs(20),
            "shortcut recording probe stuck at step {} with {} keys",
            self.step,
            self.key_events
        );
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(15),
        ));
        match self.step {
            0 if self.viewable(&self.state.as_ref().unwrap().window) => {
                self.connection
                    .set_input_focus(
                        InputFocus::PARENT,
                        xid(&self.state.as_ref().unwrap().window),
                        x11rb::CURRENT_TIME,
                    )
                    .unwrap()
                    .check()
                    .unwrap();
                self.step = 1;
            }
            1 if self.state.as_ref().unwrap().is_focused => {
                Self::click_recording(self.state.as_mut().unwrap(), ShortcutAction::Previous);
                self.send_key(0xffbe, KeyButMask::default()); // F1 must not open help/reset search.
                self.step = 2;
            }
            2 if self.key_events == 1 => {
                self.assert_recording_untouched();
                self.send_key(u32::from(','), KeyButMask::CONTROL); // Settings command is captured.
                self.step = 3;
            }
            3 if self.key_events == 2 => {
                self.assert_recording_untouched();
                self.send_key(u32::from('='), KeyButMask::CONTROL | KeyButMask::SHIFT);
                self.step = 4;
            }
            4 if self.key_events == 3 => {
                self.assert_recording_untouched();
                self.send_transition(u32::from('g'), KeyButMask::CONTROL, KEY_PRESS_EVENT);
                self.step = 5;
            }
            5 if self.key_events == 4 => {
                let state = self.state.as_mut().unwrap();
                assert_eq!(
                    state
                        .chrome
                        .shortcut_binding_label(ShortcutAction::Previous),
                    "Ctrl+G"
                );
                assert_eq!(state.chrome.shortcut_edit_generation, 1);
                assert_eq!(state.chrome.shortcut_recording, None);
                assert_eq!(state.chrome.seed_text, "private draft café 😀");
                assert_eq!(state.chrome.settings_search_query, "Previous");
                // Release Ctrl before the still-held G repeats: neither the unmodified
                // repeat nor a settings shortcut may enter an editor after recording.
                self.send_transition(0xffe3, KeyButMask::CONTROL, KEY_RELEASE_EVENT);
                self.send_transition(u32::from('g'), KeyButMask::default(), KEY_PRESS_EVENT);
                self.step = 6;
            }
            6 if self.key_events == 5 => {
                let state = self.state.as_ref().unwrap();
                assert_eq!(state.chrome.shortcut_edit_generation, 1);
                assert_eq!(state.chrome.settings_search_query, "Previous");
                assert_eq!(state.chrome.seed_text, "private draft café 😀");
                self.send_transition(u32::from('g'), KeyButMask::default(), KEY_RELEASE_EVENT);
                Self::click_recording(self.state.as_mut().unwrap(), ShortcutAction::Next);
                self.send_key(0xff1b, KeyButMask::default());
                self.step = 7;
            }
            7 if self.key_events == 6 && self.viewable(self.other.as_ref().unwrap()) => {
                let state = self.state.as_mut().unwrap();
                assert_eq!(state.chrome.shortcut_recording, None);
                assert!(
                    state.chrome.settings_open,
                    "recording Escape must not close settings"
                );
                assert_eq!(state.chrome.shortcut_edit_generation, 1);
                Self::click_recording(state, ShortcutAction::Next);
                handle_panel_window_event(
                    state,
                    event_loop,
                    WindowEvent::Ime(Ime::Preedit("日本".into(), Some((3, 3)))),
                    false,
                );
                assert_eq!(state.chrome.shortcut_recording, None);
                assert!(!state.text_input.composing());
                assert_eq!(state.chrome.settings_search_query, "Next choice");
                Self::click_recording(state, ShortcutAction::Next);
                self.connection
                    .set_input_focus(
                        InputFocus::PARENT,
                        xid(self.other.as_ref().unwrap()),
                        x11rb::CURRENT_TIME,
                    )
                    .unwrap()
                    .check()
                    .unwrap();
                self.step = 8;
            }
            8 if !self.state.as_ref().unwrap().is_focused => {
                let state = self.state.as_ref().unwrap();
                assert_eq!(state.chrome.shortcut_recording, None);
                assert_eq!(state.chrome.shortcut_edit_generation, 1);
                assert_eq!(
                    state.chrome.shortcut_bindings.get(ShortcutAction::Next),
                    None
                );
                assert_eq!(state.chrome.seed_text, "private draft café 😀");
                self.completed = true;
                println!(
                    "PASS: real X11/winit recording captures commands, holds a recorded key through modifier release/repeat, consumes Esc, and cancels on preedit/focus loss without text leakage"
                );
                event_loop.exit();
            }
            _ => {}
        }
    }
}
