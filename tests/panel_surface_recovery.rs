//! CPU regression of the production surface-error dispatch and recovery methods.
//! Fake window/surface operations do not exercise real GPU loss or presentation.
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

#[test]
fn lost_or_outdated_surface_recovers_without_a_size_change() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let controller = fs::read_to_string(root.join("src/bin/panel/controller.rs")).unwrap();
    let input = fs::read_to_string(root.join("src/bin/panel/input.rs")).unwrap();
    let resize = braced_item(&controller, "pub(super) fn resize(").unwrap();
    // Keeping this optional lets this same test expose the original no-op path.
    let recovery = braced_item(&controller, "pub(super) fn recover_surface(").unwrap_or_default();
    let dispatch = braced_item(&input, "match err {").unwrap();
    let source = HARNESS
        .replace("// PRODUCTION_METHODS", &format!("{resize}\n{recovery}"))
        .replace("// PRODUCTION_DISPATCH", &dispatch);
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "suzaku-surface-regression-{}-{unique}",
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
    let harness = directory.join("surface.rs");
    let binary = directory.join(format!("surface{}", std::env::consts::EXE_SUFFIX));
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
        "production surface recovery regression:\n{}\n{}",
        String::from_utf8_lossy(&tested.stdout),
        String::from_utf8_lossy(&tested.stderr)
    );
}

const HARNESS: &str = r#"
#![allow(dead_code)]
mod panel {
    use std::{cell::RefCell, rc::Rc};
    mod winit { pub mod dpi {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct PhysicalSize<T> { pub width: T, pub height: T }
        impl PhysicalSize<u32> {
            pub fn new(width: u32, height: u32) -> Self { Self { width, height } }
            pub fn to_logical<T>(self, _: f64) -> Self { self }
        }
    }}
    mod suzaku_map { pub mod ime { pub mod gpu {
        pub struct WgpuCandidateRenderer;
        impl WgpuCandidateRenderer { pub fn new(_: f32, _: f32) -> Self { Self } }
    }}}
    use winit::dpi::PhysicalSize;
    type Events = Rc<RefCell<Vec<&'static str>>>;
    struct Surface { events: Events, configured: RefCell<Vec<(u32, u32)>> }
    impl Surface {
        fn configure(&self, _: &(), config: &PhysicalSize<u32>) {
            assert!(config.width > 0 && config.height > 0);
            self.configured.borrow_mut().push((config.width, config.height));
            self.events.borrow_mut().push("configure");
        }
    }
    struct Window { events: Events }
    impl Window {
        fn scale_factor(&self) -> f64 { 1.0 }
        fn request_redraw(&self) { self.events.borrow_mut().push("redraw"); }
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum PanelWindowKind { Main, Settings }
    struct Chrome { compact_mode: bool }
    struct Native { presented: Option<()> }
    struct PanelState {
        size: PhysicalSize<u32>, config: PhysicalSize<u32>, surface: Surface,
        device: (), renderer: suzaku_map::ime::gpu::WgpuCandidateRenderer,
        window: Window, kind: PanelWindowKind, chrome: Chrome, native: Native,
        last_scene: Option<()>, presented_pointer_scene: Option<()>, report_view: Option<()>,
    }
    impl PanelState {
        fn new() -> Self {
            let events = Events::default();
            Self {
                size: PhysicalSize::new(640, 360), config: PhysicalSize::new(640, 360),
                surface: Surface { events: events.clone(), configured: RefCell::default() },
                device: (), renderer: suzaku_map::ime::gpu::WgpuCandidateRenderer,
                window: Window { events }, kind: PanelWindowKind::Settings,
                chrome: Chrome { compact_mode: false }, native: Native { presented: Some(()) },
                last_scene: Some(()), presented_pointer_scene: Some(()), report_view: None,
            }
        }
        fn invalidate_pointer_scene(&mut self) {
            self.last_scene = None;
            self.presented_pointer_scene = None;
            self.window.events.borrow_mut().push("invalidate");
        }
        fn is_external_window_resize(&mut self, _: PhysicalSize<u32>) -> bool { true }
        fn record_scaled_expanded_size(&mut self, _: PhysicalSize<u32>) { panic!("recovery changed the scale base"); }
        fn note_expanded_window_position(&mut self) { panic!("recovery changed window placement"); }
        fn constrain_expanded_window_position(&mut self) { panic!("recovery constrained window placement"); }
        fn reveal_settings_focus(&mut self) { panic!("recovery changed settings focus"); }
        // PRODUCTION_METHODS
    }
    #[derive(Clone, Copy, Debug)]
    enum SurfaceError { Lost, Outdated, Timeout, Other, OutOfMemory }
    #[derive(Default)]
    struct EventLoop { exited: RefCell<bool> }
    impl EventLoop { fn exit(&self) { *self.exited.borrow_mut() = true; } }
    fn dispatch(state: &mut PanelState, err: SurfaceError, event_loop: &EventLoop, allow_exit: bool) {
        // PRODUCTION_DISPATCH
    }
    fn assert_recovery(error: SurfaceError) {
        let mut state = PanelState::new();
        let size = state.size;
        dispatch(&mut state, error, &EventLoop::default(), false);
        assert_eq!(state.surface.configured.borrow().as_slice(), &[(640, 360)], "{error:?} must recreate the surface at unchanged dimensions");
        assert_eq!(state.window.events.borrow().as_slice(), &["invalidate", "configure", "redraw"], "{error:?} must schedule a frame after configuration");
        assert_eq!(state.size, size);
        assert_eq!(state.config, size);
        assert!(state.native.presented.is_none());
        assert!(state.last_scene.is_none());
        assert!(state.presented_pointer_scene.is_none());
    }
    #[test]
    fn lost_surface_reconfigures_and_redraws_at_the_same_size() { assert_recovery(SurfaceError::Lost); }
    #[test]
    fn outdated_surface_reconfigures_and_redraws_at_the_same_size() { assert_recovery(SurfaceError::Outdated); }
    #[test]
    fn an_ordinary_same_size_resize_remains_a_no_op() {
        let mut state = PanelState::new();
        state.resize(640, 360);
        assert!(state.surface.configured.borrow().is_empty());
        assert!(state.window.events.borrow().is_empty());
        assert!(state.last_scene.is_some());
        assert!(state.presented_pointer_scene.is_some());
    }
    #[test]
    fn unrelated_errors_do_not_reconfigure_the_surface_or_change_exit_policy() {
        for error in [SurfaceError::Timeout, SurfaceError::Other, SurfaceError::OutOfMemory] {
            for allow_exit in [false, true] {
                let mut state = PanelState::new();
                let event_loop = EventLoop::default();
                dispatch(&mut state, error, &event_loop, allow_exit);
                assert!(state.surface.configured.borrow().is_empty());
                assert!(state.window.events.borrow().is_empty());
                assert_eq!(*event_loop.exited.borrow(), matches!(error, SurfaceError::OutOfMemory) && allow_exit);
            }
        }
    }
}
"#;
