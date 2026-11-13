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
        candidate_edit: None,
        candidate_edit_completed: false,
        mode_boundary: None,
        mode_boundary_completed: false,
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
    candidate_edit: Option<CandidateEditProbe>,
    candidate_edit_completed: bool,
    mode_boundary: Option<ModeBoundaryProbe>,
    mode_boundary_completed: bool,
}

struct CandidateEditProbe {
    chrome: PanelChromeState,
    engine: XRTabletImeEngine,
    history: suzaku_map::panel_support::CompletionHistory,
    expected_events: usize,
}

struct ModeBoundaryProbe {
    chrome: PanelChromeState,
    engine: XRTabletImeEngine,
    history: suzaku_map::panel_support::CompletionHistory,
    runs_without_window_focus: bool,
    start_key_events: usize,
    command: usize,
    events: Vec<(
        winit::keyboard::PhysicalKey,
        winit::event::ElementState,
        bool,
    )>,
    compact_snapshot: Option<suzaku_map::ime::Snapshot>,
    compact_history: Option<(u64, Vec<String>)>,
}

impl ModeBoundaryProbe {
    fn keysym(&self) -> u32 {
        if self.command == 0 {
            u32::from('i')
        } else {
            u32::from('1')
        }
    }

    fn key(&self) -> winit::keyboard::PhysicalKey {
        use winit::keyboard::{KeyCode, PhysicalKey};
        PhysicalKey::Code(if self.command == 0 {
            KeyCode::KeyI
        } else {
            KeyCode::Digit1
        })
    }

    fn assert_compact_draft(&self, state: &PanelState) {
        assert!(state.chrome.compact_mode);
        assert_eq!(state.chrome.seed_text, "hello");
        assert_eq!(state.chrome.caret_index, 5);
        assert_eq!(
            &state.engine.snapshot(),
            self.compact_snapshot.as_ref().unwrap()
        );
        assert_eq!(
            (
                state.completion_history.revision(),
                state.completion_history.labels()
            ),
            *self.compact_history.as_ref().unwrap(),
            "compact keyboard events must preserve adopted completion undo",
        );
    }
}

fn prepare_mode_command(state: &mut PanelState, command: usize) {
    state.chrome.set_seed_text(if command == 0 {
        String::new()
    } else {
        "abc".into()
    });
    state.chrome.move_caret_to_end();
    state.chrome.active_input_mode = if command == 0 {
        InputMode::Dictation
    } else {
        InputMode::Handwriting
    };
    state.chrome.input_modes_expanded = true;
    state.chrome.voice_state = VoiceCaptureState::Idle;
    state.chrome.voice_transcript = if command == 0 {
        "hello".into()
    } else {
        String::new()
    };
    state.chrome.blur_input();
    state.refresh_seed();
    state.sync_text_input_state();
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
        let panel = xid(&self.state.as_ref().unwrap().window);
        let mask = if response_type == KEY_PRESS_EVENT {
            EventMask::KEY_PRESS
        } else {
            EventMask::KEY_RELEASE
        };
        self.connection
            .send_event(
                false,
                panel,
                mask,
                KeyPressEvent {
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
                    state: KeyButMask::default(),
                    same_screen: true,
                },
            )
            .unwrap()
            .check()
            .unwrap();
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
        if let Some(boundary) = self.mode_boundary.as_mut()
            && let WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } = &event
        {
            boundary
                .events
                .push((event.physical_key, event.state, event.repeat));
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
            10 if !self.state.as_ref().unwrap().chrome.settings_open
                && !self.candidate_edit_completed =>
            {
                assert_eq!(self.focused(), panel);
                let state = self.state.as_mut().unwrap();
                assert!(state.is_focused);
                assert!(!state.native.showing);
                let mut fixture = XRTabletImeEngine::new(EngineConfig::default());
                fixture.set_preferences_enabled(false);
                self.candidate_edit = Some(CandidateEditProbe {
                    chrome: state.chrome.clone(),
                    engine: std::mem::replace(&mut state.engine, fixture),
                    history: std::mem::take(&mut state.completion_history),
                    expected_events: self.key_events + 1,
                });
                state.chrome.active_input_mode = InputMode::VirtualKeyboard;
                state.chrome.input_modes_expanded = false;
                state.chrome.set_seed_text("hel".into());
                state.chrome.move_caret_to_end();
                state.chrome.blur_input();
                state.engine.seed("hel");
                state.refresh_composition_candidates();
                state.sync_text_input_state();
                native_sync::present_test_frame(state);
                assert!(!state.chrome.input_focused);
                assert_eq!(state.engine.snapshot().seed_text, "hel");
                self.send_keys(&[u32::from(' ')]);
                self.step = 13;
            }
            13 if self.key_events == self.candidate_edit.as_ref().unwrap().expected_events => {
                assert_eq!(self.focused(), panel);
                let state = self.state.as_ref().unwrap();
                assert!(state.is_focused);
                assert!(!state.chrome.input_focused);
                assert_eq!(state.chrome.seed_text, "hel ");
                assert_eq!(state.engine.snapshot().seed_text, "hel ");
                assert!(state.engine.snapshot().committed_text.is_empty());
                self.candidate_edit.as_mut().unwrap().expected_events = self.key_events + 1;
                self.send_keys(&[0xff08]); // Backspace must also reach candidate mode.
                self.step = 14;
            }
            14 if self.key_events == self.candidate_edit.as_ref().unwrap().expected_events => {
                assert_eq!(self.focused(), panel);
                let state = self.state.as_ref().unwrap();
                assert!(state.is_focused);
                assert!(!state.chrome.input_focused);
                assert_eq!(
                    state.chrome.seed_text, "hel",
                    "a real candidate-mode Backspace must remove the preceding Space",
                );
                assert_eq!(state.engine.snapshot().seed_text, "hel");
                assert_eq!(state.chrome.caret_index, 3);
                assert!(state.engine.snapshot().committed_text.is_empty());
                self.candidate_edit.as_mut().unwrap().expected_events = self.key_events + 1;
                self.send_keys(&[0xff08]);
                self.step = 15;
            }
            15 if self.key_events == self.candidate_edit.as_ref().unwrap().expected_events => {
                let state = self.state.as_ref().unwrap();
                assert!(!state.chrome.input_focused);
                assert_eq!(state.chrome.seed_text, "he");
                assert_eq!(state.engine.snapshot().seed_text, "he");
                assert_eq!(state.chrome.caret_index, 2);
                self.candidate_edit.as_mut().unwrap().expected_events = self.key_events + 3;
                // Two deletions and a separate no-op on the empty draft; these
                // remain real winit events, not calls to the editing helper.
                self.send_keys(&[0xff08, 0xff08, 0xff08]);
                self.step = 16;
            }
            16 if self.key_events == self.candidate_edit.as_ref().unwrap().expected_events => {
                assert_eq!(self.focused(), panel);
                let state = self.state.as_mut().unwrap();
                assert!(state.is_focused);
                assert!(!state.chrome.input_focused);
                assert!(state.chrome.seed_text.is_empty());
                assert_eq!(state.engine.snapshot().seed_text, "");
                assert!(state.engine.snapshot().committed_text.is_empty());
                assert_eq!(state.chrome.caret_index, 0);
                assert!(state.completion_history.labels().is_empty());
                let original = self.candidate_edit.take().unwrap();
                state.chrome = original.chrome;
                state.engine = original.engine;
                state.completion_history = original.history;
                state.refresh_composition_candidates();
                state.sync_text_input_state();
                self.candidate_edit_completed = true;
                self.step = 10;
                println!(
                    "PASS: real candidate-mode Space/Backspace preserve window focus, delete the explicit space and text, and leave an empty draft unchanged"
                );
            }
            10 if !self.state.as_ref().unwrap().chrome.settings_open
                && !self.mode_boundary_completed =>
            {
                assert_eq!(self.focused(), panel);
                let state = self.state.as_mut().unwrap();
                assert!(state.is_focused);
                assert!(!state.native.showing);
                let mut fixture = XRTabletImeEngine::new(EngineConfig::default());
                fixture.set_preferences_enabled(false);
                self.mode_boundary = Some(ModeBoundaryProbe {
                    chrome: state.chrome.clone(),
                    engine: std::mem::replace(&mut state.engine, fixture),
                    history: std::mem::take(&mut state.completion_history),
                    runs_without_window_focus: state.runs_without_window_focus,
                    start_key_events: self.key_events,
                    command: 0,
                    events: Vec::new(),
                    compact_snapshot: None,
                    compact_history: None,
                });
                prepare_mode_command(state, 0);
                self.send_transition(u32::from('i'), KEY_PRESS_EVENT);
                self.step = 17;
            }
            17 if self.mode_boundary.as_ref().unwrap().events.len() == 1 => {
                use winit::event::ElementState;
                let boundary = self.mode_boundary.as_ref().unwrap();
                assert_eq!(
                    boundary.events[0],
                    (boundary.key(), ElementState::Pressed, false)
                );
                let state = self.state.as_ref().unwrap();
                assert_eq!(self.focused(), panel);
                assert!(state.chrome.input_focused);
                assert_eq!(state.chrome.active_input_mode, InputMode::VirtualKeyboard);
                let expected = if boundary.command == 0 {
                    "hello"
                } else {
                    "abc"
                };
                assert_eq!(state.chrome.seed_text, expected);
                assert_eq!(state.engine.snapshot().seed_text, expected);
                self.send_transition(boundary.keysym(), KEY_PRESS_EVENT);
                self.step = 18;
            }
            18 if self.mode_boundary.as_ref().unwrap().events.len() == 2 => {
                use winit::event::ElementState;
                let boundary = self.mode_boundary.as_ref().unwrap();
                assert_eq!(
                    boundary.events[1],
                    (boundary.key(), ElementState::Pressed, true),
                    "the second real X11 press must be observed by winit as an unreleased repeat",
                );
                let expected = if boundary.command == 0 {
                    "hello"
                } else {
                    "abc"
                };
                let state = self.state.as_ref().unwrap();
                assert_eq!(
                    state.chrome.seed_text, expected,
                    "a held tool command must not become text after changing mode"
                );
                assert_eq!(state.engine.snapshot().seed_text, expected);
                self.send_transition(boundary.keysym(), KEY_RELEASE_EVENT);
                self.step = 19;
            }
            19 if self.mode_boundary.as_ref().unwrap().events.len() == 3 => {
                use winit::event::ElementState;
                let boundary = self.mode_boundary.as_ref().unwrap();
                assert_eq!(
                    boundary.events[2],
                    (boundary.key(), ElementState::Released, false)
                );
                // A distinct press after release is ordinary editing, and its
                // own repeat remains ordinary editing too. Do not disable i/1.
                self.send_transition(boundary.keysym(), KEY_PRESS_EVENT);
                self.send_transition(boundary.keysym(), KEY_PRESS_EVENT);
                self.send_transition(boundary.keysym(), KEY_RELEASE_EVENT);
                self.step = 20;
            }
            20 if self.mode_boundary.as_ref().unwrap().events.len() == 6 => {
                use winit::event::ElementState;
                let boundary = self.mode_boundary.as_ref().unwrap();
                assert_eq!(
                    boundary.events[3],
                    (boundary.key(), ElementState::Pressed, false)
                );
                assert_eq!(
                    boundary.events[4],
                    (boundary.key(), ElementState::Pressed, true)
                );
                assert_eq!(
                    boundary.events[5],
                    (boundary.key(), ElementState::Released, false)
                );
                let expected = if boundary.command == 0 {
                    "helloii"
                } else {
                    "abc11"
                };
                let state = self.state.as_ref().unwrap();
                assert_eq!(state.chrome.seed_text, expected);
                assert_eq!(state.engine.snapshot().seed_text, expected);
                if boundary.command == 0 {
                    let boundary = self.mode_boundary.as_mut().unwrap();
                    boundary.command = 1;
                    boundary.events.clear();
                    prepare_mode_command(self.state.as_mut().unwrap(), 1);
                    self.send_transition(u32::from('1'), KEY_PRESS_EVENT);
                    self.step = 17;
                } else {
                    self.mode_boundary.as_mut().unwrap().events.clear();
                    let state = self.state.as_mut().unwrap();
                    // Exercise the supported ordinary focusing-window path,
                    // not the no-focus companion's borrowed focus lease.
                    state.finish_text_editing();
                    state.runs_without_window_focus = false;
                    self.focus(panel);
                    self.step = 21;
                }
            }
            21 if self.state.as_ref().unwrap().is_focused && self.focused() == panel => {
                let state = self.state.as_mut().unwrap();
                state.chrome.set_seed_text("hel".into());
                state.chrome.move_caret_to_end();
                state.chrome.focus_input();
                state.refresh_seed();
                let completion = state
                    .next_token_completions
                    .iter()
                    .position(|edit| edit.seed_after == "hello")
                    .expect("local hello completion");
                state.select_next_token(completion);
                assert_eq!(state.chrome.seed_text, "hello");
                assert!(!state.completion_history.labels().is_empty());
                let boundary = self.mode_boundary.as_mut().unwrap();
                boundary.compact_snapshot = Some(state.engine.snapshot());
                boundary.compact_history = Some((
                    state.completion_history.revision(),
                    state.completion_history.labels(),
                ));
                state.last_compact_toggle = None;
                state.apply_compact_mode(true);
                state.sync_text_input_state();
                boundary.assert_compact_draft(state);
                assert!(!state.chrome.input_focused);
                assert_eq!(
                    self.focused(),
                    panel,
                    "compact fixture must retain actual focus on its own window"
                );
                self.send_keys(&[0xff08, u32::from(' ')]);
                self.step = 22;
            }
            22 if self.mode_boundary.as_ref().unwrap().events.len() == 4 => {
                assert_eq!(self.focused(), panel);
                let state = self.state.as_mut().unwrap();
                self.mode_boundary
                    .as_ref()
                    .unwrap()
                    .assert_compact_draft(state);
                // A later production Focused(true) notification must not turn
                // the compact orb into an invisible text editor.
                handle_panel_window_event(state, event_loop, WindowEvent::Focused(true), false);
                assert!(!state.chrome.input_focused);
                self.send_keys(&[u32::from('x')]);
                self.step = 23;
            }
            23 if self.mode_boundary.as_ref().unwrap().events.len() == 6 => {
                assert_eq!(self.focused(), panel);
                self.mode_boundary
                    .as_ref()
                    .unwrap()
                    .assert_compact_draft(self.state.as_ref().unwrap());
                self.send_keys(&[0xffbe]); // F1 remains available from the orb.
                self.step = 24;
            }
            24 if self.mode_boundary.as_ref().unwrap().events.len() == 8 => {
                let state = self.state.as_ref().unwrap();
                assert!(state.chrome.settings_open);
                assert_eq!(
                    state.chrome.settings_category,
                    suzaku_map::ime::gpu::SettingsCategory::Shortcuts
                );
                self.mode_boundary
                    .as_ref()
                    .unwrap()
                    .assert_compact_draft(state);
                self.send_keys(&[0xff1b]);
                self.step = 25;
            }
            25 if self.mode_boundary.as_ref().unwrap().events.len() == 10 => {
                let state = self.state.as_mut().unwrap();
                assert!(!state.chrome.settings_open);
                self.mode_boundary
                    .as_ref()
                    .unwrap()
                    .assert_compact_draft(state);
                let original = self.mode_boundary.take().unwrap();
                assert_eq!(
                    self.key_events,
                    original.start_key_events + 13,
                    "all mode/compact checks must traverse real X11/winit pressed events"
                );
                state.last_compact_toggle = None;
                state.apply_compact_mode(false);
                state.chrome = original.chrome;
                state.engine = original.engine;
                state.completion_history = original.history;
                state.runs_without_window_focus = original.runs_without_window_focus;
                state.refresh_composition_candidates();
                state.sync_text_input_state();
                self.mode_boundary_completed = true;
                self.step = 10;
                println!(
                    "PASS: real X11 I/1 command holds stay owned through mode changes until release, fresh editing repeats work, and a focused compact orb preserves hidden draft/engine/undo while F1 remains available"
                );
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
        settings_holds: None,
    };
    event_loop.run_app(&mut probe).unwrap();
    assert!(probe.completed);
    assert_eq!(
        probe.key_events, 6,
        "all six real winit key events must reach the dispatcher"
    );
    assert!(
        probe
            .settings_holds
            .as_ref()
            .is_some_and(|holds| holds.completed),
        "the same-window settings command hold phases must complete"
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
    settings_holds: Option<SettingsCommandHoldProbe>,
}

struct SettingsCommandHoldProbe {
    window: WindowId,
    chrome: PanelChromeState,
    engine: suzaku_map::ime::Snapshot,
    configs: Vec<(std::path::PathBuf, Option<Vec<u8>>)>,
    case: usize,
    stage: usize,
    events: Vec<(
        winit::keyboard::PhysicalKey,
        winit::event::ElementState,
        bool,
        winit::keyboard::ModifiersState,
    )>,
    completed: bool,
}

fn settings_command_config_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot inspect private command-hold configuration: {error}"),
    }
}

impl SettingsCommandHoldProbe {
    fn new(state: &PanelState) -> Self {
        let configs = [
            suzaku_map::platform::settings_host::display_settings_path(),
            std::path::PathBuf::from(std::env::var_os("SUZAKU_IME_CONFIG").unwrap()),
        ]
        .into_iter()
        .map(|path| {
            let bytes = settings_command_config_bytes(&path);
            (path, bytes)
        })
        .collect();
        Self {
            window: state.window.id(),
            chrome: state.chrome.clone(),
            engine: state.engine.snapshot(),
            configs,
            case: 0,
            stage: 0,
            events: Vec::new(),
            completed: false,
        }
    }

    fn keysym(&self) -> u32 {
        u32::from(if self.case == 0 { ',' } else { 'f' })
    }

    fn key(&self) -> winit::keyboard::PhysicalKey {
        use winit::keyboard::{KeyCode, PhysicalKey};
        PhysicalKey::Code(if self.case == 0 {
            KeyCode::Comma
        } else {
            KeyCode::KeyF
        })
    }

    fn assert_source(&self, state: &PanelState, query: &str) {
        use crate::app_state::PersistedDisplaySettings;
        assert_eq!(state.window.id(), self.window);
        assert!(state.is_focused);
        assert!(!state.runs_without_window_focus);
        assert!(state.chrome.settings_open);
        assert!(state.report_view.is_none());
        assert!(state.chrome.shortcut_recording.is_none());
        assert_eq!(
            state.chrome.settings_search_query, query,
            "command case {}",
            self.case
        );
        assert_eq!(state.chrome.seed_text, self.chrome.seed_text);
        assert_eq!(state.chrome.translation, self.chrome.translation);
        assert_eq!(state.engine.snapshot(), self.engine);
        assert_eq!(state.chrome.shortcut_profile, self.chrome.shortcut_profile);
        assert_eq!(
            state.chrome.shortcut_bindings,
            self.chrome.shortcut_bindings
        );
        assert_eq!(
            state.chrome.shortcut_edit_generation,
            self.chrome.shortcut_edit_generation
        );
        assert_eq!(
            PersistedDisplaySettings::from(&state.chrome),
            PersistedDisplaySettings::from(&self.chrome)
        );
        for (path, bytes) in &self.configs {
            assert_eq!(
                settings_command_config_bytes(path),
                *bytes,
                "settings commands must not persist configuration: {path:?}"
            );
        }
    }

    fn assert_key(
        &self,
        index: usize,
        key: winit::keyboard::PhysicalKey,
        state: winit::event::ElementState,
        repeat: bool,
        modifiers: Option<winit::keyboard::ModifiersState>,
    ) {
        let event = &self.events[index];
        assert_eq!(
            (event.0, event.1, event.2),
            (key, state, repeat),
            "case {} event {index}",
            self.case
        );
        if let Some(modifiers) = modifiers {
            assert_eq!(
                event.3, modifiers,
                "case {} event {index} modifiers",
                self.case
            );
        }
    }
}

impl ShortcutRecordingProbe {
    fn begin_settings_command_holds(&mut self) {
        assert_eq!(
            self.key_events, 6,
            "finish the original recording matrix first"
        );
        self.settings_holds = Some(SettingsCommandHoldProbe::new(self.state.as_ref().unwrap()));
        self.connection
            .set_input_focus(
                InputFocus::PARENT,
                xid(&self.state.as_ref().unwrap().window),
                x11rb::CURRENT_TIME,
            )
            .unwrap()
            .check()
            .unwrap();
        self.connection.flush().unwrap();
    }

    fn settings_command_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: WindowId,
        event: WindowEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        if window != state.window.id() {
            return;
        }
        if let WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } = &event
        {
            assert!(
                !is_synthetic,
                "the new phases require actual X11/winit keys"
            );
            self.settings_holds.as_mut().unwrap().events.push((
                event.physical_key,
                event.state,
                event.repeat,
                state.modifiers,
            ));
        }
        handle_panel_window_event(state, event_loop, event, false);
    }

    fn advance_settings_command_holds(&mut self) -> bool {
        if self.settings_holds.as_ref().unwrap().case == 2 {
            return self.advance_settings_activation_hold();
        }
        use winit::{
            event::ElementState,
            keyboard::{KeyCode, ModifiersState, PhysicalKey},
        };
        let focused = self
            .connection
            .get_input_focus()
            .unwrap()
            .reply()
            .unwrap()
            .focus;
        let own = xid(&self.state.as_ref().unwrap().window);
        let phase = self.settings_holds.as_ref().unwrap();
        assert!(
            phase.events.len() <= 8,
            "unexpected extra command events: {:?}",
            phase.events
        );
        match phase.stage {
            0 if focused == own && self.state.as_ref().unwrap().is_focused => {
                let state = self.state.as_mut().unwrap();
                assert!(!state.runs_without_window_focus);
                assert!(state.modifiers.is_empty());
                state.chrome.settings_search_query = "Text".into();
                state.chrome.settings_search_focused = false;
                state.chrome.settings_keyboard_focus = Some(InteractionKind::SettingsSearchClear);
                state.chrome.settings_scroll_offset = 0.0;
                state.sync_text_input_state();
                native_sync::present_test_frame(state);
                assert!(
                    state
                        .last_scene
                        .as_ref()
                        .unwrap()
                        .settings_focus_targets
                        .iter()
                        .any(|target| target.kind == InteractionKind::SettingsSearchClear)
                );
                self.settings_holds
                    .as_ref()
                    .unwrap()
                    .assert_source(state, "Text");
                self.send_transition(0xffe3, KeyButMask::default(), KEY_PRESS_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 1;
            }
            1 if phase.events.len() == 1 => {
                phase.assert_key(
                    0,
                    PhysicalKey::Code(KeyCode::ControlLeft),
                    ElementState::Pressed,
                    false,
                    None,
                );
                self.send_transition(phase.keysym(), KeyButMask::CONTROL, KEY_PRESS_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 2;
            }
            2 if phase.events.len() == 2 => {
                phase.assert_key(
                    1,
                    phase.key(),
                    ElementState::Pressed,
                    false,
                    Some(ModifiersState::CONTROL),
                );
                let state = self.state.as_ref().unwrap();
                phase.assert_source(state, "Text");
                if phase.case == 0 {
                    assert!(!state.chrome.settings_search_focused);
                    assert_eq!(
                        state.chrome.settings_keyboard_focus, None,
                        "Ctrl+, must traverse the actual settings command"
                    );
                } else {
                    assert!(
                        state.chrome.settings_search_focused,
                        "Ctrl+F must focus search from an initially unfocused field"
                    );
                    assert_eq!(
                        state.chrome.settings_keyboard_focus,
                        Some(InteractionKind::SettingsSearchInput)
                    );
                }
                self.send_transition(0xffe3, KeyButMask::CONTROL, KEY_RELEASE_EVENT);
                self.send_transition(phase.keysym(), KeyButMask::default(), KEY_PRESS_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 3;
            }
            3 if phase.events.len() == 4 => {
                phase.assert_key(
                    2,
                    PhysicalKey::Code(KeyCode::ControlLeft),
                    ElementState::Released,
                    false,
                    None,
                );
                phase.assert_key(
                    3,
                    phase.key(),
                    ElementState::Pressed,
                    true,
                    Some(ModifiersState::empty()),
                );
                phase.assert_source(self.state.as_ref().unwrap(), "Text");
                self.send_transition(phase.keysym(), KeyButMask::default(), KEY_RELEASE_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 4;
            }
            4 if phase.events.len() == 5 => {
                phase.assert_key(
                    4,
                    phase.key(),
                    ElementState::Released,
                    false,
                    Some(ModifiersState::empty()),
                );
                phase.assert_source(self.state.as_ref().unwrap(), "Text");
                self.send_transition(phase.keysym(), KeyButMask::default(), KEY_PRESS_EVENT);
                self.send_transition(phase.keysym(), KeyButMask::default(), KEY_PRESS_EVENT);
                self.send_transition(phase.keysym(), KeyButMask::default(), KEY_RELEASE_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 5;
            }
            5 if phase.events.len() == 8 => {
                for (index, state, repeat) in [
                    (5, ElementState::Pressed, false),
                    (6, ElementState::Pressed, true),
                    (7, ElementState::Released, false),
                ] {
                    phase.assert_key(
                        index,
                        phase.key(),
                        state,
                        repeat,
                        Some(ModifiersState::empty()),
                    );
                }
                let expected = if phase.case == 0 { "Text,," } else { "Textff" };
                phase.assert_source(self.state.as_ref().unwrap(), expected);
                assert!(self.state.as_ref().unwrap().chrome.settings_search_focused);
                if phase.case == 0 {
                    let phase = self.settings_holds.as_mut().unwrap();
                    phase.case = 1;
                    phase.stage = 0;
                    phase.events.clear();
                } else {
                    self.settings_holds.as_mut().unwrap().completed = true;
                    println!(
                        "PASS: real X11 same-window Ctrl+, and Ctrl+F keep unreleased command keys out of search after Ctrl release; fresh plain presses and repeats still type, with source/preferences/configuration unchanged"
                    );
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn begin_settings_activation_hold(&mut self) {
        let phase = self.settings_holds.as_mut().unwrap();
        assert!(
            phase.completed,
            "the original two command phases must finish"
        );
        assert_eq!(phase.case, 1);
        phase.case = 2;
        phase.stage = 0;
        phase.events.clear();
        phase.completed = false;
    }

    fn advance_settings_activation_hold(&mut self) -> bool {
        use winit::{
            event::ElementState,
            keyboard::{KeyCode, ModifiersState, PhysicalKey},
        };
        let key = PhysicalKey::Code(KeyCode::Space);
        let phase = self.settings_holds.as_ref().unwrap();
        assert!(phase.events.len() <= 6);
        match phase.stage {
            0 => {
                let state = self.state.as_mut().unwrap();
                assert!(state.is_focused);
                assert!(state.modifiers.is_empty());
                assert!(!state.text_input.composing());
                state.chrome.settings_search_query = "Text".into();
                state.chrome.settings_search_focused = false;
                state.chrome.settings_keyboard_focus = Some(InteractionKind::SettingsSearchClear);
                state.chrome.settings_scroll_offset = 0.0;
                state.sync_text_input_state();
                native_sync::present_test_frame(state);
                assert!(
                    state
                        .last_scene
                        .as_ref()
                        .unwrap()
                        .settings_focus_targets
                        .iter()
                        .any(|target| target.kind == InteractionKind::SettingsSearchClear)
                );
                phase.assert_source(state, "Text");
                self.send_transition(u32::from(' '), KeyButMask::default(), KEY_PRESS_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 1;
            }
            1 if phase.events.len() == 1 => {
                phase.assert_key(
                    0,
                    key,
                    ElementState::Pressed,
                    false,
                    Some(ModifiersState::empty()),
                );
                let state = self.state.as_ref().unwrap();
                phase.assert_source(state, "");
                assert!(state.chrome.settings_search_focused);
                assert_eq!(
                    state.chrome.settings_keyboard_focus,
                    Some(InteractionKind::SettingsSearchInput)
                );
                self.send_transition(u32::from(' '), KeyButMask::default(), KEY_PRESS_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 2;
            }
            2 if phase.events.len() == 2 => {
                phase.assert_key(
                    1,
                    key,
                    ElementState::Pressed,
                    true,
                    Some(ModifiersState::empty()),
                );
                phase.assert_source(self.state.as_ref().unwrap(), "");
                self.send_transition(u32::from(' '), KeyButMask::default(), KEY_RELEASE_EVENT);
                self.settings_holds.as_mut().unwrap().stage = 3;
            }
            3 if phase.events.len() == 3 => {
                phase.assert_key(
                    2,
                    key,
                    ElementState::Released,
                    false,
                    Some(ModifiersState::empty()),
                );
                phase.assert_source(self.state.as_ref().unwrap(), "");
                for transition in [KEY_PRESS_EVENT, KEY_PRESS_EVENT, KEY_RELEASE_EVENT] {
                    self.send_transition(u32::from(' '), KeyButMask::default(), transition);
                }
                self.settings_holds.as_mut().unwrap().stage = 4;
            }
            4 if phase.events.len() == 6 => {
                for (index, state, repeat) in [
                    (3, ElementState::Pressed, false),
                    (4, ElementState::Pressed, true),
                    (5, ElementState::Released, false),
                ] {
                    phase.assert_key(index, key, state, repeat, Some(ModifiersState::empty()));
                }
                phase.assert_source(self.state.as_ref().unwrap(), "  ");
                assert!(self.state.as_ref().unwrap().chrome.settings_search_focused);
                self.settings_holds.as_mut().unwrap().completed = true;
                println!(
                    "PASS: real X11 Space activation clears the presented search control without leaking its held repeat into the newly focused editor; after release, fresh Space and repeat still type with draft/preferences/configuration unchanged"
                );
                return true;
            }
            _ => {}
        }
        false
    }

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
        if self.settings_holds.is_some() {
            self.settings_command_window_event(event_loop, window, event);
            return;
        }
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
        if self.settings_holds.is_some() {
            if self.advance_settings_command_holds() {
                if self.settings_holds.as_ref().unwrap().case == 1 {
                    self.begin_settings_activation_hold();
                } else {
                    event_loop.exit();
                }
            }
            return;
        }
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
                if self.settings_holds.is_none() {
                    self.begin_settings_command_holds();
                    return;
                }
                event_loop.exit();
            }
            _ => {}
        }
    }
}
