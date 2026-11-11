//! Real GPU surfaces in the opt-in private Xvfb fixture; no keyboard or model I/O.
use super::*;
use std::sync::{Mutex, mpsc};
use suzaku_map::{
    ime::{
        PredictionStatus,
        companion::NativeComposition,
        gpu::{PanelLayoutMode, RenderScene, TextLayout, TextRole, ThemePreset},
    },
    prediction::{
        PredictionCancellation, PredictionError, PredictionProvider, PredictionRequest,
        PredictionResponse,
    },
    ui::{PredictionNotice, UiLanguage},
};
use winit::{dpi::PhysicalSize, platform::x11::EventLoopBuilderExtX11};

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn native_prediction_feedback_is_render_local_and_preserves_draft_and_delivery_feedback() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let temporary = std::env::temp_dir();
    for name in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "SUZAKU_IME_CONFIG",
    ] {
        let path = std::path::PathBuf::from(std::env::var_os(name).expect("private test path"));
        assert!(path.is_absolute() && path.starts_with(&temporary), "{name}");
        assert_ne!(path, temporary, "a dedicated fixture path is required");
        assert!(
            !path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir)),
            "private paths cannot escape the fixture"
        );
        for ancestor in path
            .ancestors()
            .take_while(|ancestor| *ancestor != temporary.as_path())
        {
            match std::fs::symlink_metadata(ancestor) {
                Ok(metadata) => assert!(!metadata.file_type().is_symlink(), "{name}"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("cannot inspect the private fixture path: {error}"),
            }
        }
    }
    let display = std::env::var("DISPLAY").expect("isolated X11 display");
    let number = display.split('.').next().unwrap();
    assert!(
        number.starts_with(':'),
        "a local private Xvfb display is required"
    );
    assert_ne!(number, ":0", "the normal desktop is never in scope");
    let mut builder = EventLoop::<PanelUserEvent>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    let mut probe = PredictionFeedbackProbe {
        proxy: events.create_proxy(),
        completed: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

struct PredictionFeedbackProbe {
    proxy: EventLoopProxy<PanelUserEvent>,
    completed: bool,
}

struct ControlledProvider {
    entered: mpsc::Sender<()>,
    reply: Mutex<mpsc::Receiver<PredictionError>>,
}

impl PredictionProvider for ControlledProvider {
    fn predict(
        &self,
        _: &PredictionRequest,
        _: &PredictionCancellation,
    ) -> Result<PredictionResponse, PredictionError> {
        self.entered.send(()).unwrap();
        Err(self
            .reply
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap())
    }
}

fn input_layouts(scene: &RenderScene) -> &[TextLayout] {
    &scene
        .text_sections
        .iter()
        .find(|section| section.role == TextRole::InputLabel)
        .expect("the expanded input panel has its original two-layout header")
        .layouts
}

fn frame(state: &mut PanelState) -> (RenderScene, Vec<render::PanelOverlay>) {
    let snapshot = state.view_snapshot();
    state.current_frame_with_snapshot(&snapshot)
}

fn assert_title(scene: &RenderScene, expected: &str) {
    let layouts = input_layouts(scene);
    assert_eq!(layouts.len(), 2);
    assert!(
        !layouts[0].truncated,
        "the private viewport must show the complete notice"
    );
    assert_eq!(layouts[0].lines.join(""), expected);
    assert!(layouts[0].lines.len() <= 1);
}

fn assert_render_only_state(state: &PanelState, feedback: &str, ticks: u8) {
    assert!(state.chrome.prediction_notice.is_none());
    assert_eq!(state.last_commit_feedback.as_deref(), Some(feedback));
    assert_eq!(state.commit_feedback_ticks, ticks);
    let mut settings = state.chrome.clone();
    assert!(!mirror_settings_chrome(&mut settings, &state.chrome));
    assert!(!mirror_settings_chrome(&mut settings, &state.chrome));
    assert!(settings.prediction_notice.is_none());
}

fn assert_notice_ignored_in_surface(state: &mut PanelState) {
    assert!(state.chrome.prediction_notice.is_none());
    let (baseline, overlays) = frame(state);
    assert!(overlays.is_empty());
    state.chrome.prediction_notice =
        Some(PredictionNotice::Error(PredictionError::MissingCredentials));
    let (with_notice, overlays) = frame(state);
    // Settings also uses InputLabel for its search box. Compare the whole
    // surface instead of confusing that shared role with the main title.
    assert_eq!(with_notice, baseline);
    assert!(overlays.is_empty());
    state.chrome.prediction_notice = None;
    state
        .render()
        .expect("present the isolated auxiliary surface");
    assert!(state.chrome.prediction_notice.is_none());
}

impl ApplicationHandler<PanelUserEvent> for PredictionFeedbackProbe {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        let window = Arc::new(
            events
                .create_window(
                    panel_window_attributes().with_inner_size(PhysicalSize::new(900, 500)),
                )
                .unwrap(),
        );
        let mut panel = pollster::block_on(PanelState::new(window)).unwrap();
        panel.engine.configure_prediction(None);
        panel.engine.set_preferences_enabled(false);
        panel.chrome.ui_language = UiLanguage::ChineseSimplified;
        panel.chrome.theme_preset = ThemePreset::Suzaku;
        panel.chrome.panel_layout_mode = PanelLayoutMode::FollowCaret;
        panel.chrome.detected_panel_layout = PanelLayoutMode::FollowCaret;
        panel.chrome.compact_mode = false;
        panel.chrome.settings_open = false;
        panel.chrome.llm_enabled = true;
        panel.chrome.set_seed_text("hel".into());
        panel.chrome.move_caret_to_end();
        panel.native.showing = false;
        panel.native.frame = None;
        panel.is_focused = true;
        panel.chrome.input_focused = true;
        panel.resize(900, 500);
        assert_eq!(panel.window.inner_size(), PhysicalSize::new(900, 500));
        let feedback = "Sent to active app: synthetic retained delivery";
        let ticks = 17;
        panel.last_commit_feedback = Some(feedback.into());
        panel.commit_feedback_ticks = ticks;
        let mut app = PanelApp::new(self.proxy.clone(), None);
        // Never call PanelApp::resumed: that would start tray/native ownership.
        app.panel = Some(panel);
        app.panel_visible = true;
        assert!(app.tray.is_none() && app.native_sync.is_none());

        // Initialize cold GPU/font/dictionary resources before starting the
        // controlled provider's unchanged two-second wait. This fixture checks
        // state transitions, not cold-start rendering performance.
        {
            let panel = app.panel.as_mut().unwrap();
            panel.engine.seed("hel");
            panel.refresh_composition_candidates();
            panel.render().expect("initialize the isolated GPU surface");
            assert_render_only_state(panel, feedback, ticks);
        }

        for error in [
            PredictionError::Timeout,
            PredictionError::MissingCredentials,
        ] {
            let (entered, entered_rx) = mpsc::channel();
            let (reply, reply_rx) = mpsc::channel();
            let panel = app.panel.as_mut().unwrap();
            panel.engine.configure_prediction(None);
            panel.engine.seed("");
            panel
                .engine
                .configure_prediction_provider(Some(Arc::new(ControlledProvider {
                    entered,
                    reply: Mutex::new(reply_rx),
                })));
            panel.engine.seed("hel");
            panel.refresh_composition_candidates();
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(panel.engine.prediction_status(), PredictionStatus::Pending);
            let snapshot = panel.engine.snapshot();
            let candidates = panel.engine.candidates().to_vec();
            let persisted = app_state::PersistedDisplaySettings::from(&panel.chrome);
            let (pending, pending_overlays) = frame(panel);
            let waiting = PredictionNotice::Waiting.render(panel.chrome.ui_language);
            assert_title(&pending, &waiting);
            assert_eq!(
                pending_overlays.len(),
                1,
                "the delivery overlay retains precedence"
            );
            assert!(!pending_overlays[0].atlas_glyphs.is_empty());
            assert_render_only_state(panel, feedback, ticks);
            panel
                .render()
                .expect("present the actual Pending GPU surface");
            assert_title(panel.last_scene.as_ref().unwrap(), &waiting);

            reply.send(error.clone()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            let update = loop {
                let update = prediction_feedback::poll(&mut panel.engine);
                if update.changed() {
                    break update;
                }
                assert!(
                    Instant::now() < deadline,
                    "no failed request feedback update"
                );
                std::thread::sleep(Duration::from_millis(2));
            };
            assert!(update.state_changed && !update.candidates_changed);
            assert_eq!(panel.engine.prediction_error(), Some(&error));
            assert_eq!(
                panel.engine.prediction_status(),
                PredictionStatus::Unavailable
            );
            assert_eq!(panel.engine.snapshot(), snapshot);
            assert_eq!(panel.engine.candidates(), candidates);
            let expected = PredictionNotice::Error(error.clone()).render(panel.chrome.ui_language);
            let (failed, failed_overlays) = frame(panel);
            assert_title(&failed, &expected);
            assert_ne!(expected, waiting);
            assert_eq!(input_layouts(&failed)[1], input_layouts(&pending)[1]);
            assert_eq!(failed.interactive_targets, pending.interactive_targets);
            assert_eq!(failed.hit_targets, pending.hit_targets);
            assert_eq!(failed.labels, pending.labels);
            assert_eq!(failed_overlays.len(), 1);
            assert_eq!(failed_overlays[0].quads, pending_overlays[0].quads);
            assert_eq!(
                failed_overlays[0].atlas_glyphs,
                pending_overlays[0].atlas_glyphs
            );
            assert_eq!(
                app_state::PersistedDisplaySettings::from(&panel.chrome),
                persisted
            );
            assert_render_only_state(panel, feedback, ticks);
            assert!(!prediction_feedback::poll(&mut panel.engine).changed());
            panel
                .render()
                .expect("present the actual failed-request GPU surface");
            assert_title(panel.last_scene.as_ref().unwrap(), &expected);
            assert_render_only_state(panel, feedback, ticks);
        }

        let panel = app.panel.as_mut().unwrap();
        let brand = panel.chrome.ui_language.tr("Suzaku · Input").to_owned();
        panel.native.showing = true;
        let (native, _) = frame(panel);
        assert_title(&native, &brand);
        panel.native.showing = false;
        let private_frame = NativeComposition {
            host: "00000000-0000-0000-0000-000000000001".into(),
            context: 1,
            revision: 1,
            focused: true,
            private: true,
            cursor: None,
            language: "en".into(),
            seed: String::new(),
            selected: 0,
            candidates: Vec::new(),
        };
        // Real frame parsing/receiving, not an invented IPC service or a bool
        // standing in for the focused private context in production rendering.
        let parsed =
            NativeComposition::parse(private_frame.to_json().to_string().as_bytes()).unwrap();
        panel.receive_native_frame(Some(parsed));
        assert!(
            panel
                .native
                .frame
                .as_ref()
                .is_some_and(|frame| frame.focused && frame.private)
        );
        assert!(!panel.native.showing);
        let (private, _) = frame(panel);
        assert_title(&private, &brand);
        panel.receive_native_frame(None);
        assert!(panel.native.frame.is_none());
        for (kind, native, report, compact) in [
            (PanelWindowKind::Main, true, false, false),
            (PanelWindowKind::Settings, false, false, false),
            (PanelWindowKind::Main, false, true, false),
            (PanelWindowKind::Main, false, false, true),
        ] {
            assert_eq!(
                prediction_feedback::notice(kind, native, report, compact, &panel.engine),
                None
            );
        }
        panel.chrome.compact_mode = true;
        let (compact, compact_overlays) = frame(panel);
        assert!(compact.text_sections.is_empty());
        assert!(compact_overlays.is_empty());
        panel.chrome.compact_mode = false;
        assert_render_only_state(panel, feedback, ticks);

        let settings_window = Arc::new(
            events
                .create_window(settings_window_attributes(&panel.chrome))
                .unwrap(),
        );
        let mut settings = pollster::block_on(PanelState::new_settings(
            settings_window,
            panel.chrome.clone(),
        ))
        .unwrap();
        assert_notice_ignored_in_surface(&mut settings);
        settings.begin_report_details(
            report_view::ReportSnapshot::InputMethodError("synthetic read-only fixture".into()),
            true,
        );
        assert_notice_ignored_in_surface(&mut settings);
        assert_render_only_state(app.panel.as_ref().unwrap(), feedback, ticks);
        println!(
            "PASS: private GPU Pending/error titles stay render-local, retain input/candidates/delivery overlays and respect native/private/settings/report/orb boundaries"
        );
        self.completed = true;
        events.exit();
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
