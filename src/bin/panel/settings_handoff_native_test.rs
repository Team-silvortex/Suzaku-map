//! Cross-window command ownership on private X11 windows, through PanelApp.
use super::*;
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
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
    let RawWindowHandle::Xlib(handle) = window.window_handle().unwrap().as_raw() else {
        panic!("private X11 fixture required");
    };
    handle.window as u32
}

pub(super) struct SettingsHandoffProbe {
    connection: RustConnection,
    main: WindowId,
    settings: Option<WindowId>,
    case: usize,
    stage: usize,
    started: Instant,
    events: Vec<(WindowId, KeyEvent, ModifiersState)>,
    source: PanelChromeState,
    engine: suzaku_map::ime::Snapshot,
    configs: Vec<(std::path::PathBuf, Option<Vec<u8>>)>,
}

impl SettingsHandoffProbe {
    pub(super) fn new(app: &mut PanelApp, event_loop: &ActiveEventLoop) -> Self {
        let old = app.settings.as_ref().unwrap().window.id();
        app.window_event(event_loop, old, WindowEvent::CloseRequested);
        assert!(app.settings.is_none());
        let panel = app.panel.as_mut().unwrap();
        assert!(!panel.chrome.llm_enabled);
        assert!(!panel.chrome.compact_mode);
        assert!(!panel.text_input.composing());
        panel.chrome.settings_search_query = "Text".into();
        panel.chrome.settings_search_focused = false;
        panel.chrome.settings_keyboard_focus = None;
        panel.window.set_visible(true);
        let configs = [
            suzaku_map::platform::settings_host::display_settings_path(),
            std::path::PathBuf::from(std::env::var_os("SUZAKU_IME_CONFIG").unwrap()),
        ]
        .into_iter()
        .map(|path| {
            let bytes = config_bytes(&path);
            (path, bytes)
        })
        .collect();
        let (connection, _) = x11rb::connect(None).unwrap();
        let probe = Self {
            connection,
            main: panel.window.id(),
            settings: None,
            case: 0,
            stage: 0,
            started: Instant::now(),
            events: Vec::new(),
            source: panel.chrome.clone(),
            engine: panel.engine.snapshot(),
            configs,
        };
        probe.focus(&panel.window);
        probe
    }

    pub(super) fn observe(&mut self, app: &PanelApp, id: WindowId, event: &WindowEvent) {
        let state = if id == self.main {
            app.panel.as_ref()
        } else {
            app.settings
                .as_ref()
                .filter(|state| state.window.id() == id)
        };
        if let Some(state) = state
            && let WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } = event
        {
            self.events.push((id, event.clone(), state.modifiers));
        }
    }

    fn focus(&self, window: &Window) {
        self.connection
            .set_input_focus(InputFocus::PARENT, xid(window), x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        self.connection.flush().unwrap();
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

    fn send(&self, window: &Window, control: bool, modifiers: KeyButMask, response_type: u8) {
        let setup = self.connection.setup();
        let mapping = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap();
        let width = usize::from(mapping.keysyms_per_keycode);
        let keysym = if control { 0xffe3 } else { u32::from(',') };
        let code = mapping
            .keysyms
            .chunks(width)
            .position(|row| row[0] == keysym)
            .expect("private keyboard mapping") as u8
            + setup.min_keycode;
        let window = xid(window);
        self.connection
            .send_event(
                false,
                window,
                if response_type == KEY_PRESS_EVENT {
                    EventMask::KEY_PRESS
                } else {
                    EventMask::KEY_RELEASE
                },
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

    fn assert_key(
        &self,
        index: usize,
        window: WindowId,
        control: bool,
        state: ElementState,
        repeat: Option<bool>,
    ) {
        let (actual_window, event, _) = &self.events[index];
        assert_eq!(*actual_window, window, "case {} event {index}", self.case);
        assert_eq!(
            event.physical_key,
            PhysicalKey::Code(if control {
                KeyCode::ControlLeft
            } else {
                KeyCode::Comma
            })
        );
        assert_eq!(event.state, state);
        if let Some(repeat) = repeat {
            assert_eq!(event.repeat, repeat, "case {} event {index}", self.case);
        }
    }

    fn assert_source(&self, app: &PanelApp, query: &str) {
        let panel = app.panel.as_ref().unwrap();
        let settings = app.settings.as_ref().unwrap();
        assert_eq!(panel.window.id(), self.main);
        assert_eq!(Some(settings.window.id()), self.settings);
        assert!(settings.is_focused);
        assert!(!panel.is_focused);
        assert!(panel.chrome.settings_open);
        assert!(settings.chrome.settings_open);
        assert!(settings.report_view.is_none());
        assert_eq!(
            settings.chrome.settings_search_query, query,
            "cross-window command case {} must not leak its unreleased key into search",
            self.case
        );
        for state in [panel, settings] {
            assert_eq!(state.chrome.seed_text, self.source.seed_text);
            assert_eq!(state.chrome.translation, self.source.translation);
            assert_eq!(
                state.chrome.shortcut_bindings,
                self.source.shortcut_bindings
            );
            assert_eq!(
                state.chrome.shortcut_edit_generation,
                self.source.shortcut_edit_generation
            );
            assert_eq!(
                PersistedDisplaySettings::from(&state.chrome),
                PersistedDisplaySettings::from(&self.source)
            );
        }
        assert_eq!(panel.engine.snapshot(), self.engine);
        for (path, bytes) in &self.configs {
            assert_eq!(
                config_bytes(path),
                *bytes,
                "private configuration changed: {path:?}"
            );
        }
    }

    pub(super) fn advance(&mut self, app: &mut PanelApp) -> bool {
        assert!(
            self.started.elapsed() < Duration::from_secs(5),
            "cross-window case {} stage {} events {:?}",
            self.case,
            self.stage,
            self.events
        );
        let panel = app.panel.as_ref().unwrap();
        match self.stage {
            0 if panel.is_focused && self.viewable(&panel.window) => {
                assert!(panel.modifiers.is_empty());
                self.send(&panel.window, true, KeyButMask::default(), KEY_PRESS_EVENT);
                self.stage = 1;
            }
            1 if self.events.len() == 1 => {
                self.assert_key(0, self.main, true, ElementState::Pressed, Some(false));
                self.send(&panel.window, false, KeyButMask::CONTROL, KEY_PRESS_EVENT);
                if self.case == 2 {
                    // Queue the source release before the app can request the
                    // target focus. X11 routes keys through its active window.
                    self.send(&panel.window, false, KeyButMask::CONTROL, KEY_RELEASE_EVENT);
                }
                self.stage = 2;
            }
            2 if self.events.len() == (if self.case == 2 { 3 } else { 2 }) => {
                self.assert_key(1, self.main, false, ElementState::Pressed, Some(false));
                assert_eq!(self.events[1].2, ModifiersState::CONTROL);
                if self.case == 2 {
                    self.assert_key(2, self.main, false, ElementState::Released, Some(false));
                }
                let settings = app
                    .settings
                    .as_ref()
                    .expect("actual Main command must open Settings");
                if self.case == 0 {
                    self.settings = Some(settings.window.id());
                } else {
                    assert_eq!(
                        self.settings,
                        Some(settings.window.id()),
                        "reuse must keep the same auxiliary window"
                    );
                }
                if self.viewable(&settings.window) {
                    // Supply a real X11 focus acknowledgement on Xvfb, which has
                    // no desktop window manager. No Focused event is fabricated.
                    self.focus(&settings.window);
                    self.stage = 3;
                }
            }
            3 if app.settings.as_ref().unwrap().is_focused && !panel.is_focused => {
                self.assert_source(app, "Text");
                let settings = app.settings.as_ref().unwrap();
                if self.case == 2 {
                    self.send(
                        &settings.window,
                        true,
                        KeyButMask::CONTROL,
                        KEY_RELEASE_EVENT,
                    );
                    self.stage = 7;
                } else {
                    self.send(
                        &settings.window,
                        true,
                        KeyButMask::CONTROL,
                        KEY_RELEASE_EVENT,
                    );
                    self.send(
                        &settings.window,
                        false,
                        KeyButMask::default(),
                        KEY_PRESS_EVENT,
                    );
                    self.stage = 4;
                }
            }
            4 if self.events.len() == 4 => {
                let settings = app.settings.as_ref().unwrap();
                self.assert_key(
                    2,
                    settings.window.id(),
                    true,
                    ElementState::Released,
                    Some(false),
                );
                // Focus transitions can reset the backend repeat bit. Physical
                // ownership, not that bit, must cover this unreleased key.
                self.assert_key(3, settings.window.id(), false, ElementState::Pressed, None);
                assert!(self.events[3].2.is_empty());
                self.assert_source(app, "Text");
                self.send(
                    &settings.window,
                    false,
                    KeyButMask::default(),
                    KEY_RELEASE_EVENT,
                );
                self.stage = 5;
            }
            5 if self.events.len() == 5 => {
                let settings = app.settings.as_ref().unwrap();
                let release = self.events.len() - 1;
                self.assert_key(
                    release,
                    settings.window.id(),
                    false,
                    ElementState::Released,
                    Some(false),
                );
                self.assert_source(app, "Text");
                self.send_plain_pair(&settings.window);
                self.stage = 6;
            }
            7 if self.events.len() == 4 => {
                let settings = app.settings.as_ref().unwrap();
                self.assert_key(
                    3,
                    settings.window.id(),
                    true,
                    ElementState::Released,
                    Some(false),
                );
                self.assert_source(app, "Text");
                self.send_plain_pair(&settings.window);
                self.stage = 6;
            }
            6 if self.events.len() == (if self.case == 2 { 7 } else { 8 }) => {
                let window = app.settings.as_ref().unwrap().window.id();
                let start = self.events.len() - 3;
                for (offset, state, repeat) in [
                    (0, ElementState::Pressed, false),
                    (1, ElementState::Pressed, true),
                    (2, ElementState::Released, false),
                ] {
                    self.assert_key(start + offset, window, false, state, Some(repeat));
                }
                self.assert_source(app, "Text,,");
                if self.case == 2 {
                    println!(
                        "PASS: real X11 Main-to-Settings creation/reuse retains command-key ownership across focus, consumes a source-window release, and permits fresh plain presses/repeats without changing source/preferences/configuration"
                    );
                    return true;
                }
                // Reuse, rather than destroy, the same auxiliary on the next
                // command. Only navigation state is reset; no preference is saved.
                for state in [app.panel.as_mut().unwrap(), app.settings.as_mut().unwrap()] {
                    state.chrome.settings_search_query = "Text".into();
                    state.chrome.settings_search_focused = false;
                    state.chrome.settings_keyboard_focus = None;
                    state.sync_text_input_state();
                }
                self.case += 1;
                self.stage = 0;
                self.events.clear();
                self.focus(&app.panel.as_ref().unwrap().window);
            }
            _ => {}
        }
        false
    }

    fn send_plain_pair(&self, window: &Window) {
        self.send(window, false, KeyButMask::default(), KEY_PRESS_EVENT);
        self.send(window, false, KeyButMask::default(), KEY_PRESS_EVENT);
        self.send(window, false, KeyButMask::default(), KEY_RELEASE_EVENT);
    }
}

fn config_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("cannot read private fixture configuration: {error}"),
    }
}
