//! Execute production auxiliary-window routing with deterministic CPU failures.
//! Platform, GPU and editing facilities are stubs; no desktop session is used.
use std::{fs, path::Path, process::Command, time::SystemTime};

fn braced_item(source: &str, marker: &str) -> Option<String> {
    let start = source.find(marker)?;
    assert_eq!(source.matches(marker).count(), 1, "ambiguous {marker}");
    let opening = start + source[start..].find('{').unwrap();
    let mut depth = 0;
    for (offset, byte) in source.as_bytes()[opening..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(source[start..=opening + offset].to_owned());
                }
            }
            _ => {}
        }
    }
    panic!("unclosed {marker}");
}

fn block_body(source: &str, marker: &str) -> String {
    let item = braced_item(source, marker).unwrap();
    item[item.find('{').unwrap() + 1..item.len() - 1].to_owned()
}

#[test]
fn auxiliary_window_failures_preserve_input_and_allow_explicit_retry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let panel = fs::read_to_string(root.join("src/bin/panel.rs")).unwrap();
    let controller = fs::read_to_string(root.join("src/bin/panel/controller.rs")).unwrap();
    let input = fs::read_to_string(root.join("src/bin/panel/input.rs")).unwrap();
    let keyboard = fs::read_to_string(root.join("src/bin/panel/keyboard.rs")).unwrap();
    // Optional helpers allow this fixture to execute the original failing code,
    // while required entry points always come from the current implementation.
    let helpers = braced_item(&panel, "fn create_auxiliary_window(").unwrap_or_default();
    let methods = [
        "fn open_settings(",
        "fn show_report_details(",
        "fn window_event(",
    ]
    .into_iter()
    .map(|marker| braced_item(&panel, marker).unwrap())
    .chain(
        [
            "fn sync_settings_window(",
            "fn show_auxiliary_window_error(",
        ]
        .into_iter()
        .filter_map(|marker| braced_item(&panel, marker)),
    )
    .collect::<Vec<_>>()
    .join("\n");
    let source = HARNESS
        .replace(
            "// PRODUCTION_DISPATCH_SYNC",
            &braced_item(&keyboard, "fn sync_text_input_after_dispatch(").unwrap_or_else(|| {
                "fn sync_text_input_after_dispatch(&mut self) { self.sync_text_input_state(); }"
                    .into()
            }),
        )
        .replace("// PRODUCTION_HELPERS", &helpers)
        .replace("// PRODUCTION_METHODS", &methods)
        .replace(
            "// PRODUCTION_GEAR_REQUEST",
            &block_body(&controller, "InteractionKind::SettingsToggle => {"),
        )
        .replace(
            "// PRODUCTION_SHORTCUT_REQUEST",
            &block_body(
                &input,
                "if let Some(show_reference) = super::keyboard::settings_shortcut(",
            ),
        );
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "suzaku-auxiliary-regression-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let harness = directory.join("auxiliary.rs");
    let binary = directory.join(format!("auxiliary{}", std::env::consts::EXE_SUFFIX));
    fs::write(&harness, source).unwrap();
    let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2024", "--test"])
        .arg(&harness)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "CPU harness compilation failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let tested = Command::new(&binary).arg("--nocapture").output().unwrap();
    assert!(
        tested.status.success(),
        "production auxiliary-window regressions:\n{}\n{}",
        String::from_utf8_lossy(&tested.stdout),
        String::from_utf8_lossy(&tested.stderr)
    );
    print!("{}", String::from_utf8_lossy(&tested.stdout));
}

const HARNESS: &str = r#"
#![allow(dead_code)]
use std::{cell::{Cell, RefCell}, error::Error, rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure { None, Create, Initialize }
struct Fixture {
    failure: Failure, creates: usize, initializes: usize, finishes: usize,
    dropped_auxiliary_windows: usize,
}
type Shared = Rc<RefCell<Fixture>>;
type WindowId = usize;
mod winit { pub mod event {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ElementState { Pressed, Released }
}}
struct Window { id: WindowId, visible: Cell<bool>, focus_requests: Cell<usize>, fixture: Shared }
impl Window {
    fn id(&self) -> WindowId { self.id }
    fn set_visible(&self, visible: bool) { self.visible.set(visible); }
    fn focus_window(&self) { self.focus_requests.set(self.focus_requests.get() + 1); }
    fn request_redraw(&self) {}
}
impl Drop for Window {
    fn drop(&mut self) {
        if self.id != 0 { self.fixture.borrow_mut().dropped_auxiliary_windows += 1; }
    }
}
struct ActiveEventLoop { fixture: Shared }
impl ActiveEventLoop {
    fn create_window(&self, _: ()) -> Result<Window, std::io::Error> {
        let mut fixture = self.fixture.borrow_mut();
        fixture.creates += 1;
        if fixture.failure == Failure::Create {
            return Err(std::io::Error::other("synthetic create failure"));
        }
        Ok(Window { id: fixture.creates, visible: Cell::new(false), focus_requests: Cell::new(0), fixture: self.fixture.clone() })
    }
}
mod pollster { pub fn block_on<T>(value: T) -> T { value } }
mod suzaku_map { pub mod ime { pub mod gpu {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SettingsCategory { Appearance, Shortcuts }
}}}
use suzaku_map::ime::gpu::SettingsCategory;
mod report_view {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ReportSnapshot(pub String);
}
#[derive(Clone, Debug, PartialEq)]
struct PanelChromeState {
    settings_open: bool, compact_mode: bool, seed_text: String, input_focused: bool,
    settings_scroll_offset: f32, settings_keyboard_focus: Option<()>,
    settings_search_focused: bool, settings_category: SettingsCategory,
    settings_search_query: String, settings_collapsed_sections: Vec<usize>,
}
impl Default for PanelChromeState {
    fn default() -> Self {
        Self {
            settings_open: false, compact_mode: false, seed_text: "retained draft 日".into(),
            input_focused: true, settings_scroll_offset: 9.0, settings_keyboard_focus: None,
            settings_search_focused: false, settings_category: SettingsCategory::Appearance,
            settings_search_query: String::new(), settings_collapsed_sections: Vec::new(),
        }
    }
}
struct Frame { host: String, context: u64 }
impl Frame { fn visible(&self) -> bool { true } }
struct Native {
    keyboard_blocked: bool, typing_blocked: bool, presented: Option<()>, frame: Option<Frame>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalKey { F1, Comma }
#[derive(Clone, Copy)]
struct KeyEvent { physical_key: PhysicalKey, state: winit::event::ElementState }
struct TextInput { allowed: bool, preedit: String, revocations: usize, command_holds: Vec<PhysicalKey> }
impl TextInput {
    fn hold_panel_command_key(&mut self, key: PhysicalKey) {
        if !self.command_holds.contains(&key) { self.command_holds.push(key); }
    }
    fn clear_shortcut_hold(&mut self) { self.command_holds.clear(); }
    fn inherit_owned_holds(&mut self, other: &Self) {
        for &key in &other.command_holds { self.hold_panel_command_key(key); }
    }
    fn consume_recorded_hold(&mut self, key: PhysicalKey, pressed: bool) -> bool {
        if pressed { return self.command_holds.contains(&key); }
        if let Some(index) = self.command_holds.iter().position(|held| *held == key) {
            self.command_holds.remove(index);
            return true;
        }
        false
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelWindowKind { Main, Settings }
struct PanelState {
    kind: PanelWindowKind, settings_window_request: Option<bool>, input_dispatch_guard: bool,
    window: Arc<Window>, chrome: PanelChromeState, native: Native,
    report_view: Option<report_view::ReportSnapshot>, report_return_to_settings: bool,
    last_commit_feedback: Option<String>, commit_feedback_ticks: u8, is_focused: bool,
    last_compact_toggle: Option<()>,
    text_input: TextInput, last_scene: Option<()>, close_requested: bool,
    quit_requested: bool, settings_save_requested: bool,
}
impl PanelState {
    fn new_settings(window: Arc<Window>, chrome: PanelChromeState) -> Result<Self, Box<dyn Error>> {
        let failed = {
            let mut fixture = window.fixture.borrow_mut();
            fixture.initializes += 1;
            fixture.failure == Failure::Initialize
        };
        if failed { return Err(std::io::Error::other("synthetic initialize failure").into()); }
        let mut state = Self::new(window, chrome);
        state.kind = PanelWindowKind::Settings;
        Ok(state)
    }
    fn new(window: Arc<Window>, chrome: PanelChromeState) -> Self {
        Self {
            kind: PanelWindowKind::Main, settings_window_request: None, input_dispatch_guard: false,
            window, chrome, native: Native { keyboard_blocked: false, typing_blocked: false,
                presented: Some(()), frame: None }, report_view: None,
            report_return_to_settings: false, last_commit_feedback: None,
            commit_feedback_ticks: 0, is_focused: true,
            text_input: TextInput { allowed: true, preedit: String::new(), revocations: 0, command_holds: Vec::new() },
            last_compact_toggle: None,
            last_scene: Some(()), close_requested: false, quit_requested: false,
            settings_save_requested: false,
        }
    }
    fn suspend_native_keyboard(&mut self) {
        self.native.keyboard_blocked = true;
        self.native.typing_blocked = true;
    }
    fn apply_compact_mode(&mut self, compact: bool) { self.chrome.compact_mode = compact; }
    fn end_report_details(&mut self) { self.report_view = None; self.report_return_to_settings = false; }
    fn begin_report_details(&mut self, snapshot: report_view::ReportSnapshot, return_to_settings: bool) {
        self.report_view = Some(snapshot); self.report_return_to_settings = return_to_settings;
    }
    fn apply_window_decorations(&mut self) {}
    fn sync_text_input_permission(&mut self) -> bool {
        let allowed = self.is_focused && self.chrome.input_focused
            && !self.chrome.compact_mode && !self.chrome.settings_open;
        if self.text_input.allowed && !allowed {
            self.text_input.revocations += 1;
            self.text_input.preedit.clear();
        }
        self.text_input.allowed = allowed;
        self.text_input.allowed
    }
    fn sync_text_input_state(&mut self) { self.sync_text_input_permission(); }
    // PRODUCTION_DISPATCH_SYNC
    fn handle_report_window_event(&mut self, _: &ActiveEventLoop, _: WindowEvent) -> bool { false }
    fn adopt_settings_from(&mut self, _: &PanelChromeState) -> bool { false }
    fn persist_display_settings(&mut self) { panic!("unexpected settings save"); }
    fn has_local_panel_interaction(&self) -> bool { self.is_focused || self.chrome.settings_open }
    fn native_composition_visible(&self) -> bool { self.native.frame.is_some() }
    fn cancel_translation(&mut self) { panic!("unexpected translation cancellation"); }
    fn finish_text_editing(&mut self) { panic!("unexpected input ownership change"); }
    fn clear_pointer_hover(&mut self) { panic!("unexpected hover reset"); }
    fn request_settings_by_gear(&mut self) {
        // PRODUCTION_GEAR_REQUEST
    }
    fn request_settings_by_shortcut(&mut self, show_reference: bool) {
        let state = self;
        let event = KeyEvent { physical_key: if show_reference { PhysicalKey::F1 } else { PhysicalKey::Comma }, state: winit::event::ElementState::Pressed };
        // PRODUCTION_SHORTCUT_REQUEST
    }
}
fn settings_window_attributes(_: &PanelChromeState) {}
fn finish_settings_window_creation(window: &Arc<Window>, _: &Arc<Window>, _: &ActiveEventLoop) {
    window.fixture.borrow_mut().finishes += 1;
    window.set_visible(true);
}
fn mirror_settings_chrome(settings: &mut PanelChromeState, main: &PanelChromeState) -> bool {
    let changed = settings != main; *settings = main.clone(); changed
}
// PRODUCTION_HELPERS
struct SystemTray;
impl SystemTray {
    fn has_icon(&self) -> bool { false }
    fn set_panel_visible(&self, _: bool) {}
}
#[derive(Clone, Copy)]
enum WindowEvent {
    CloseRequested, RedrawRequested, Gear, Shortcut, Preferences, Other,
    KeyboardInput { event: KeyEvent, is_synthetic: bool },
}
fn handle_panel_window_event(state: &mut PanelState, _: &ActiveEventLoop, event: WindowEvent, _: bool) -> bool {
    if state.input_dispatch_guard { return false; }
    state.settings_window_request = None;
    match event {
        WindowEvent::Gear => state.request_settings_by_gear(),
        WindowEvent::Shortcut => state.request_settings_by_shortcut(true),
        WindowEvent::Preferences => state.request_settings_by_shortcut(false),
        // Own-window release is an existing input-dispatch responsibility. The
        // cross-window retirement under test stays in the extracted PanelApp.
        WindowEvent::KeyboardInput { event, is_synthetic: false }
            if event.state == winit::event::ElementState::Released => {
                state.text_input.consume_recorded_hold(event.physical_key, false);
            }
        _ => {}
    }
    state.sync_text_input_after_dispatch();
    true
}
struct PanelApp {
    panel: Option<PanelState>, settings: Option<PanelState>, native_auto_shown: bool,
    native_hidden_context: Option<(String, u64)>, panel_visible: bool,
    next_frame_at: Option<()>, tray: Option<SystemTray>,
}
impl PanelApp {
    fn set_panel_visible(&mut self, visible: bool) {
        self.panel_visible = visible;
        self.native_auto_shown = false;
        self.native_hidden_context = None;
        self.panel.as_ref().unwrap().window.set_visible(visible);
    }
    fn request_quit(&self, _: &ActiveEventLoop) { panic!("unexpected event-loop exit"); }
    fn close_report_details(&mut self) { self.settings = None; }
    // PRODUCTION_METHODS
}
fn fixture(failure: Failure) -> (PanelApp, ActiveEventLoop) {
    let shared = Rc::new(RefCell::new(Fixture { failure, creates: 0, initializes: 0,
        finishes: 0, dropped_auxiliary_windows: 0 }));
    let panel = PanelState::new(Arc::new(Window { id: 0, visible: Cell::new(true), focus_requests: Cell::new(0),
        fixture: shared.clone() }), PanelChromeState::default());
    (PanelApp { panel: Some(panel), settings: None, native_auto_shown: true,
        native_hidden_context: Some(("host-owned".into(), 71)), panel_visible: true,
        next_frame_at: None, tray: None }, ActiveEventLoop { fixture: shared })
}
fn assert_failure(app: &PanelApp, events: &ActiveEventLoop, stage: Failure, attempts: usize, blocked: bool) {
    let panel = app.panel.as_ref().unwrap();
    assert!(app.settings.is_none(), "a failed constructor cannot publish an auxiliary state");
    assert!(!panel.chrome.settings_open, "failed settings must not remain pending");
    assert!(panel.text_input.allowed, "failed settings must restore main input permission");
    assert_eq!(panel.native.keyboard_blocked, blocked, "failed settings suspended the keyboard");
    assert_eq!(panel.native.typing_blocked, blocked, "failed settings suspended native typing");
    let feedback = panel.last_commit_feedback.as_deref().expect("visible error feedback");
    assert!(feedback.starts_with("Input method error: "), "feedback must use the visible error class: {feedback}");
    assert!(feedback.contains(match stage { Failure::Create => "create", _ => "initialize" }), "missing failure-stage context: {feedback}");
    assert!(panel.commit_feedback_ticks > 0);
    let fixture = events.fixture.borrow();
    assert_eq!(fixture.creates, attempts);
    assert_eq!(fixture.initializes, if stage == Failure::Initialize { attempts } else { 0 });
    assert_eq!(fixture.finishes, 0, "failed windows cannot be shown or focused");
    assert_eq!(fixture.dropped_auxiliary_windows, fixture.initializes, "a failed initialization must release its hidden window");
}
fn assert_settings_success(app: &PanelApp, events: &ActiveEventLoop, attempts: usize) {
    let panel = app.panel.as_ref().unwrap();
    let settings = app.settings.as_ref().expect("explicit retry can open settings");
    assert!(panel.chrome.settings_open);
    assert!(settings.chrome.settings_open, "the new settings state must inherit the successful request");
    assert!(settings.window.visible.get());
    assert!(panel.native.keyboard_blocked && panel.native.typing_blocked);
    assert!(!panel.text_input.allowed);
    assert_eq!(events.fixture.borrow().creates, attempts);
    assert_eq!(events.fixture.borrow().finishes, 1);
}
fn check_tray_settings_failures(stage: Failure) {
        for blocked in [false, true] {
            let (mut app, events) = fixture(stage);
            app.panel.as_mut().unwrap().native.keyboard_blocked = blocked;
            app.panel.as_mut().unwrap().native.typing_blocked = blocked;
            for attempts in 1..=3 {
                app.open_settings(&events);
                assert_failure(&app, &events, stage, attempts, blocked);
                app.window_event(&events, 0, WindowEvent::RedrawRequested);
                assert_failure(&app, &events, stage, attempts, blocked);
            }
            events.fixture.borrow_mut().failure = Failure::None;
            app.open_settings(&events);
            assert_settings_success(&app, &events, 4);
            let id = app.settings.as_ref().unwrap().window.id();
            app.window_event(&events, id, WindowEvent::Other);
            assert!(app.settings.is_some(), "the first settings event must not close the new window");
        }
}
#[test]
fn tray_settings_creation_failures_roll_back_and_allow_explicit_retry() {
    check_tray_settings_failures(Failure::Create);
}
#[test]
fn tray_settings_initialization_failures_roll_back_and_allow_explicit_retry() {
    check_tray_settings_failures(Failure::Initialize);
}
fn check_main_settings_failures(stage: Failure) {
    for trigger in [WindowEvent::Gear, WindowEvent::Shortcut] {
            for blocked in [false, true] {
                let (mut app, events) = fixture(stage);
                app.panel.as_mut().unwrap().native.keyboard_blocked = blocked;
                app.panel.as_mut().unwrap().native.typing_blocked = blocked;
                for attempts in 1..=3 {
                    app.window_event(&events, 0, trigger);
                    assert_failure(&app, &events, stage, attempts, blocked);
                    app.window_event(&events, 0, WindowEvent::RedrawRequested);
                    assert_failure(&app, &events, stage, attempts, blocked);
                }
                events.fixture.borrow_mut().failure = Failure::None;
                app.window_event(&events, 0, trigger);
                assert_settings_success(&app, &events, 4);
            }
    }
}
#[test]
fn main_gear_and_shortcut_creation_failures_restore_permission_without_automatic_retry() {
    check_main_settings_failures(Failure::Create);
}
#[test]
fn main_gear_and_shortcut_initialization_failures_restore_permission_without_automatic_retry() {
    check_main_settings_failures(Failure::Initialize);
}
#[derive(Debug, PartialEq)]
struct InputOwnership {
    chrome: PanelChromeState, focused: bool, allowed: bool, visible: bool,
    panel_visible: bool, auto_shown: bool, hidden_context: Option<(String, u64)>,
    keyboard_blocked: bool, typing_blocked: bool, presented: Option<()>,
}
fn ownership(app: &PanelApp) -> InputOwnership {
    let panel = app.panel.as_ref().unwrap();
    InputOwnership { chrome: panel.chrome.clone(), focused: panel.is_focused,
        allowed: panel.text_input.allowed, visible: panel.window.visible.get(),
        panel_visible: app.panel_visible, auto_shown: app.native_auto_shown,
        hidden_context: app.native_hidden_context.clone(),
        keyboard_blocked: panel.native.keyboard_blocked, typing_blocked: panel.native.typing_blocked,
        presented: panel.native.presented }
}
fn check_report_failures(stage: Failure) {
        for blocked in [false, true] {
            let (mut app, events) = fixture(stage);
            app.panel_visible = false;
            let panel = app.panel.as_mut().unwrap();
            panel.window.set_visible(false);
            panel.chrome.compact_mode = true;
            panel.is_focused = false;
            panel.text_input.allowed = false;
            panel.native.keyboard_blocked = blocked;
            panel.native.typing_blocked = blocked;
            let before = ownership(&app);
            for attempts in 1..=3 {
                app.show_report_details(&events, report_view::ReportSnapshot("retained report 日".into()));
                assert_eq!(ownership(&app), before, "report failure must not acquire input ownership");
                assert!(app.settings.is_none());
                let feedback = app.panel.as_ref().unwrap().last_commit_feedback.as_deref().unwrap();
                assert!(feedback.starts_with("Input method error: "));
                assert!(feedback.contains("report"));
                assert!(feedback.contains(match stage { Failure::Create => "create", _ => "initialize" }));
                let fixture = events.fixture.borrow();
                assert_eq!(fixture.creates, attempts);
                assert_eq!(fixture.finishes, 0);
                assert_eq!(fixture.dropped_auxiliary_windows, fixture.initializes);
                drop(fixture);
                app.window_event(&events, 0, WindowEvent::RedrawRequested);
                assert_eq!(ownership(&app), before);
                assert_eq!(events.fixture.borrow().creates, attempts);
            }
            events.fixture.borrow_mut().failure = Failure::None;
            app.show_report_details(&events, report_view::ReportSnapshot("retained report 日".into()));
            assert_eq!(ownership(&app), before);
            assert_eq!(app.settings.as_ref().unwrap().report_view, Some(report_view::ReportSnapshot("retained report 日".into())));
            assert_eq!(events.fixture.borrow().finishes, 1);
            app.window_event(&events, 0, WindowEvent::RedrawRequested);
            assert!(app.settings.as_ref().unwrap().report_view.is_some(), "main redraw cannot dismiss the independent report");
            assert_eq!(ownership(&app), before);
        }
}
#[test]
fn report_creation_failures_leave_hidden_compact_input_ownership_unchanged() {
    check_report_failures(Failure::Create);
}
#[test]
fn report_initialization_failures_leave_hidden_compact_input_ownership_unchanged() {
    check_report_failures(Failure::Initialize);
}
#[test]
fn existing_auxiliary_window_is_reused_during_constructor_failure() {
    let (mut app, events) = fixture(Failure::None);
    app.open_settings(&events);
    assert_settings_success(&app, &events, 1);
    let id = app.settings.as_ref().unwrap().window.id();
    events.fixture.borrow_mut().failure = Failure::Create;
    app.show_report_details(&events, report_view::ReportSnapshot("existing report".into()));
    assert_eq!(app.settings.as_ref().unwrap().window.id(), id);
    assert!(app.settings.as_ref().unwrap().report_return_to_settings);
    app.open_settings(&events);
    assert_eq!(app.settings.as_ref().unwrap().window.id(), id);
    assert!(app.settings.as_ref().unwrap().report_view.is_none());
    assert_settings_success(&app, &events, 1);
    assert_eq!(app.settings.as_ref().unwrap().window.focus_requests.get(), 1);
}

#[test]
fn main_settings_requests_replace_reports_but_ordinary_redraws_preserve_them() {
    for return_to_settings in [false, true] {
        for trigger in [WindowEvent::Gear, WindowEvent::Shortcut, WindowEvent::Preferences] {
            let (mut app, events) = fixture(Failure::None);
            if return_to_settings { app.open_settings(&events); }
            app.show_report_details(&events, report_view::ReportSnapshot("frozen report".into()));
            let report_id = app.settings.as_ref().unwrap().window.id();
            app.native_auto_shown = false;
            let before = ownership(&app);
            app.window_event(&events, 0, WindowEvent::RedrawRequested);
            assert_eq!(ownership(&app), before, "ordinary redraw changed report ownership");
            assert!(app.settings.as_ref().unwrap().report_view.is_some());

            // Main gear is a toggle; its first click closes a report inherited
            // from ordinary settings, while the other commands always open.
            app.window_event(&events, 0, trigger);
            if return_to_settings && matches!(trigger, WindowEvent::Gear) {
                assert!(app.settings.is_none(), "an explicit close must also close a settings report");
                assert!(!app.panel.as_ref().unwrap().chrome.settings_open);
                assert!(app.panel.as_ref().unwrap().text_input.allowed);
            } else {
                let settings = app.settings.as_ref().expect("explicit open reuses the report window");
                assert_eq!(settings.window.id(), report_id);
                assert!(settings.report_view.is_none(), "explicit main settings request left a read-only report");
                assert_eq!(settings.window.focus_requests.get(), 1, "explicit reuse must request focus from the window manager");
                assert!(settings.chrome.settings_open);
                assert_eq!(settings.chrome.settings_category, if matches!(trigger, WindowEvent::Shortcut) {
                    SettingsCategory::Shortcuts
                } else { SettingsCategory::Appearance });
                assert!(!app.panel.as_ref().unwrap().text_input.allowed);
                app.window_event(&events, 0, WindowEvent::Gear);
                assert!(app.settings.is_none(), "explicit close left an ordinary settings window");
                assert!(app.panel.as_ref().unwrap().text_input.allowed);
            }
            assert!(app.panel.as_ref().unwrap().settings_window_request.is_none());
            assert_eq!(events.fixture.borrow().creates, 1, "reuse cannot create another window");
        }
    }
}

#[test]
fn main_settings_failure_preserves_preedit_and_success_releases_it_once() {
    for stage in [Failure::Create, Failure::Initialize] {
        // Real composing input rejects F1/Ctrl+, before their request branch.
        // The pointer gear can request settings while preedit is still active.
        let (mut app, events) = fixture(stage);
        app.panel.as_mut().unwrap().text_input.preedit = "unfinished 日本".into();
        app.window_event(&events, 0, WindowEvent::Gear);
        assert_failure(&app, &events, stage, 1, false);
        let panel = app.panel.as_ref().unwrap();
        assert_eq!(panel.text_input.preedit, "unfinished 日本", "failed construction erased preedit before rollback");
        assert_eq!(panel.text_input.revocations, 0, "failure revoked the original input context");
        assert!(panel.settings_window_request.is_none());
        events.fixture.borrow_mut().failure = Failure::None;
        app.window_event(&events, 0, WindowEvent::Gear);
        assert_settings_success(&app, &events, 2);
        let panel = app.panel.as_ref().unwrap();
        assert!(panel.text_input.preedit.is_empty());
        assert_eq!(panel.text_input.revocations, 1, "success releases the old input context once");
        assert!(panel.settings_window_request.is_none());
    }
}

#[test]
fn rejected_reentrant_dispatch_cannot_consume_an_outer_settings_request() {
    let (mut app, events) = fixture(Failure::None);
    app.native_auto_shown = false;
    let panel = app.panel.as_mut().unwrap();
    panel.settings_window_request = Some(true);
    panel.input_dispatch_guard = true;
    panel.text_input.preedit = "outer unfinished 日本".into();
    let before = ownership(&app);
    app.window_event(&events, 0, WindowEvent::Gear);
    assert_eq!(ownership(&app), before);
    assert_eq!(app.panel.as_ref().unwrap().settings_window_request, Some(true));
    assert_eq!(app.panel.as_ref().unwrap().text_input.preedit, "outer unfinished 日本");
    assert!(app.settings.is_none());
    assert_eq!(events.fixture.borrow().creates, 0);
}

#[test]
fn shortcut_request_context_records_keys_without_changing_preedit() {
    let (mut app, _) = fixture(Failure::None);
    let panel = app.panel.as_mut().unwrap();
    panel.text_input.preedit = "unfinished 日本".into();
    panel.request_settings_by_gear();
    assert!(panel.text_input.command_holds.is_empty(), "pointer gear is not a key command");
    panel.request_settings_by_shortcut(false);
    panel.request_settings_by_shortcut(false);
    assert_eq!(panel.text_input.command_holds, [PhysicalKey::Comma]);
    panel.request_settings_by_shortcut(true);
    assert_eq!(panel.text_input.command_holds, [PhysicalKey::Comma, PhysicalKey::F1]);
    assert_eq!(panel.text_input.preedit, "unfinished 日本");
    assert_eq!(panel.text_input.revocations, 0);
    panel.request_settings_by_gear();
    assert!(panel.text_input.command_holds.is_empty(), "closing clears owned commands");
    assert_eq!(panel.text_input.preedit, "unfinished 日本");
}

#[test]
fn successful_settings_creation_and_reuse_merge_existing_owned_commands() {
    let (mut app, events) = fixture(Failure::None);
    app.panel.as_mut().unwrap().text_input.hold_panel_command_key(PhysicalKey::Comma);
    app.open_settings(&events);
    assert_settings_success(&app, &events, 1);
    assert_eq!(app.settings.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
    assert_eq!(app.panel.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
    let target = app.settings.as_mut().unwrap();
    let id = target.window.id();
    target.text_input.hold_panel_command_key(PhysicalKey::F1);
    target.text_input.preedit = "retained target preedit".into();
    app.open_settings(&events);
    let target = app.settings.as_ref().unwrap();
    assert_eq!(target.window.id(), id, "reuse must keep the same managed target");
    assert_eq!(target.text_input.command_holds, [PhysicalKey::Comma, PhysicalKey::F1], "merge must retain target ownership without duplicating source ownership");
    assert_eq!(target.text_input.preedit, "retained target preedit", "handoff must not copy or clear the target's unrelated text input");
    assert_eq!(app.panel.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
    assert_eq!(events.fixture.borrow().creates, 1);
    assert_eq!(target.window.focus_requests.get(), 1);
}

#[test]
fn failed_settings_creation_keeps_source_commands_until_explicit_retry() {
    for failure in [Failure::Create, Failure::Initialize] {
        let (mut app, events) = fixture(failure);
        let source = app.panel.as_mut().unwrap();
        source.text_input.hold_panel_command_key(PhysicalKey::Comma);
        source.text_input.preedit = "retained failed request".into();
        app.open_settings(&events);
        assert_failure(&app, &events, failure, 1, false);
        assert_eq!(app.panel.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
        assert_eq!(app.panel.as_ref().unwrap().text_input.preedit, "retained failed request");
        events.fixture.borrow_mut().failure = Failure::None;
        app.open_settings(&events);
        assert_settings_success(&app, &events, 2);
        assert_eq!(app.settings.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
        assert_eq!(app.panel.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
    }
}

#[test]
fn only_real_managed_window_releases_retire_the_other_windows_owned_command() {
    use winit::event::ElementState;
    for release_from_settings in [false, true] {
        let (mut app, events) = fixture(Failure::None);
        app.panel.as_mut().unwrap().text_input.hold_panel_command_key(PhysicalKey::Comma);
        app.open_settings(&events);
        let target = app.settings.as_ref().unwrap().window.id();
        let sender = if release_from_settings { target } else { 0 };
        let key_event = |state, is_synthetic| WindowEvent::KeyboardInput {
            event: KeyEvent { physical_key: PhysicalKey::Comma, state }, is_synthetic,
        };
        let both_owned = |app: &PanelApp| {
            assert_eq!(app.panel.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
            assert_eq!(app.settings.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
        };
        both_owned(&app);
        app.window_event(&events, sender, key_event(ElementState::Pressed, false));
        both_owned(&app);
        app.window_event(&events, sender, key_event(ElementState::Released, true));
        both_owned(&app);
        app.window_event(&events, usize::MAX, key_event(ElementState::Released, false));
        both_owned(&app);
        if release_from_settings {
            app.settings.as_mut().unwrap().input_dispatch_guard = true;
        } else {
            app.panel.as_mut().unwrap().input_dispatch_guard = true;
        }
        app.window_event(&events, sender, key_event(ElementState::Released, false));
        both_owned(&app);
        if release_from_settings {
            app.settings.as_mut().unwrap().input_dispatch_guard = false;
        } else {
            app.panel.as_mut().unwrap().input_dispatch_guard = false;
        }
        app.window_event(&events, sender, key_event(ElementState::Released, false));
        assert!(app.panel.as_ref().unwrap().text_input.command_holds.is_empty());
        assert!(app.settings.as_ref().unwrap().text_input.command_holds.is_empty());
        assert!(app.panel.as_ref().unwrap().settings_window_request.is_none());
        assert!(app.settings.as_ref().unwrap().settings_window_request.is_none());
        assert_eq!(events.fixture.borrow().creates, 1);
    }
}

#[test]
fn a_new_read_only_report_does_not_inherit_main_owned_commands() {
    let (mut app, events) = fixture(Failure::None);
    app.panel.as_mut().unwrap().text_input.hold_panel_command_key(PhysicalKey::Comma);
    app.show_report_details(&events, report_view::ReportSnapshot("read-only handoff boundary".into()));
    assert!(app.settings.as_ref().unwrap().report_view.is_some());
    assert!(app.settings.as_ref().unwrap().text_input.command_holds.is_empty());
    assert_eq!(app.panel.as_ref().unwrap().text_input.command_holds, [PhysicalKey::Comma]);
    assert_eq!(events.fixture.borrow().creates, 1);
}
"#;
