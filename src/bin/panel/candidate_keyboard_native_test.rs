//! Production keyboard-command helpers, only in the opt-in private GPU fixture.
use super::*;
use std::{
    io::{Read, Write},
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        mpsc,
    },
};
use suzaku_map::{
    ime::{
        Candidate, PredictionStatus, Snapshot,
        companion::NativeComposition,
        gpu::{PanelLayoutMode, RenderScene, TranslationPhase, TranslationView},
    },
    languages::{
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest},
        translation::{TranslationError, TranslationProvider, TranslationRequest},
    },
    platform::ime_host_dispatch::current_ime_host_dispatch,
    prediction::{
        PredictionCancellation, PredictionCandidate, PredictionError, PredictionProvider,
        PredictionRequest, PredictionResponse,
    },
};
use winit::{dpi::PhysicalSize, platform::x11::EventLoopBuilderExtX11};

fn private_fixture() {
    assert_eq!(std::env::var("SUZAKU_PANEL_NATIVE_QA").as_deref(), Ok("1"));
    let temporary = std::env::temp_dir();
    for name in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_RUNTIME_DIR",
        "SUZAKU_IME_CONFIG",
    ] {
        let path = PathBuf::from(std::env::var_os(name).expect("private fixture path"));
        assert!(path.is_absolute() && path.starts_with(&temporary), "{name}");
        assert_ne!(path, temporary, "dedicated fixture paths are required");
        assert!(
            !path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
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
    let display = std::env::var("DISPLAY").expect("private Xvfb display");
    let display = display.split('.').next().unwrap();
    assert!(display.starts_with(':'));
    assert_ne!(display, ":0", "never use the normal desktop");
    // These explicit fixture flags avoid probing/starting the personal service.
    for name in [
        "SUZAKU_LINUX_IME_DAEMON_READY",
        "SUZAKU_LINUX_IME_REGISTERED",
        "SUZAKU_LINUX_IME_RUNTIME_VISIBLE",
        "SUZAKU_LINUX_IME_HOST_READY",
        "SUZAKU_LINUX_IME_MARKED_TEXT",
        "SUZAKU_LINUX_IME_COMMIT",
    ] {
        assert_eq!(std::env::var(name).as_deref(), Ok("1"), "{name}");
    }
    assert_eq!(
        std::env::var("SUZAKU_LINUX_IME_FRAMEWORK").as_deref(),
        Ok("ibus")
    );
    assert_eq!(std::env::var("SUZAKU_LINUX_IME_ACTIVE").as_deref(), Ok("0"));
}

#[derive(Clone, Copy)]
enum Case {
    Numbers,
    Enter,
}

fn run(case: Case) {
    private_fixture();
    let mut builder = EventLoop::<()>::with_user_event();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    let mut probe = KeyboardProbe {
        case,
        completed: false,
    };
    events.run_app(&mut probe).unwrap();
    assert!(probe.completed);
}

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn numbered_choices_wait_for_the_presented_candidate_identity() {
    run(Case::Numbers);
}

#[test]
#[ignore = "requires isolated Xvfb and SUZAKU_PANEL_NATIVE_QA=1"]
fn keyboard_enter_honors_explicit_selection_and_acknowledgements() {
    run(Case::Enter);
}

struct KeyboardProbe {
    case: Case,
    completed: bool,
}

fn reset(state: &mut PanelState) {
    state.engine.configure_prediction(None);
    state.engine.clear_session_context();
    state.chrome.set_seed_text("hel".into());
    state.chrome.move_caret_to_end();
    state.chrome.blur_input();
    state.chrome.settings_open = false;
    state.chrome.input_modes_expanded = false;
    state.completion_history.clear();
    state.last_interaction_action = None;
    state.last_commit_attempt = None;
    state.engine.seed("hel");
    state.refresh_composition_candidates();
}

impl ApplicationHandler for KeyboardProbe {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        let window = Arc::new(
            events
                .create_window(
                    panel_window_attributes()
                        .with_title("Synthetic keyboard candidate QA")
                        .with_inner_size(PhysicalSize::new(900, 480)),
                )
                .unwrap(),
        );
        let mut state = pollster::block_on(PanelState::new(window)).unwrap();
        state.chrome.llm_enabled = false;
        state.chrome.panel_layout_mode = PanelLayoutMode::FollowCaret;
        state.chrome.detected_panel_layout = PanelLayoutMode::FollowCaret;
        state.chrome.compact_mode = false;
        state.native = Default::default();
        state.runs_without_window_focus = true;
        state.is_focused = false;
        state.engine.set_language("en");
        state.engine.set_preferences_enabled(false);
        // Cold shader/font/dictionary work is outside either controlled provider's
        // unchanged two-second request/reply wait, not a cold-start performance claim.
        reset(&mut state);
        native_sync::present_test_frame(&mut state);
        match self.case {
            Case::Numbers => {
                numbers(&mut state);
                signal_changed_case(&mut state);
            }
            Case::Enter => {
                enter(&mut state);
                exact_candidate_identity(&mut state);
                unresolved_enter_identity(&mut state);
                held_sentence_identity_boundaries(&mut state);
                sentence_preview_identity_boundaries(&mut state);
                signal_noop_and_native_case(&mut state);
                native_recovery_survives_focus_before_editing(&mut state);
                visible_local_selection_boundaries(&mut state);
                occluded_pointer_boundaries(&mut state, events);
            }
        }
        self.completed = true;
        events.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

struct GatedProvider {
    entered: mpsc::Sender<()>,
    reply: Mutex<mpsc::Receiver<()>>,
}

impl LlmCompletionProvider for GatedProvider {
    fn provider_id(&self) -> &str {
        "keyboard-candidate-fixture"
    }
    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        if request.seed_text != "hel" {
            return Vec::new();
        }
        self.entered.send(()).unwrap();
        self.reply
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        vec![LlmCompletion {
            text: "helipad".into(),
            score_bias: 1.0,
            kind: None,
        }]
    }
}

fn numbers(state: &mut PanelState) {
    state.engine.seed("");
    let (entered, entered_rx) = mpsc::channel();
    let (reply, reply_rx) = mpsc::channel();
    state
        .engine
        .configure_prediction(Some(Arc::new(GatedProvider {
            entered,
            reply: Mutex::new(reply_rx),
        })));
    state.engine.seed("hel");
    state.refresh_composition_candidates();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    native_sync::present_test_frame(state);
    let source = state.chrome.sentence_candidate_source_indices[2];
    assert_eq!(state.engine.candidates()[source].text, "help");
    assert!(
        state
            .last_scene
            .as_ref()
            .unwrap()
            .interactive_targets
            .iter()
            .any(|target| target.kind == InteractionKind::Candidate(source))
    );
    reply.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !state.engine.poll_prediction() {
        assert!(
            Instant::now() < deadline,
            "the synthetic model result did not arrive"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    state.refresh_composition_candidates();
    assert!(
        state.last_scene.is_none(),
        "the changed candidate pool was not presented"
    );
    assert_eq!(state.engine.candidates()[source].text, "helipad");
    let before = state.engine.snapshot();
    let seed = state.chrome.seed_text.clone();
    let caret = state.chrome.caret_index;
    let history = state.completion_history.labels();
    let revision = state.completion_history.revision();
    assert!(
        state.try_numbered_candidate(2),
        "a stale valid slot must consume Digit3, not open handwriting"
    );
    assert_eq!(
        state.chrome.seed_text, seed,
        "Digit3 adopted an unseen model candidate"
    );
    assert_eq!(state.chrome.caret_index, caret);
    assert_eq!(state.engine.snapshot(), before);
    assert_eq!(state.completion_history.labels(), history);
    assert_eq!(state.completion_history.revision(), revision);
    native_sync::present_test_frame(state);
    assert!(state.try_numbered_candidate(2));
    assert_eq!(
        state.chrome.seed_text, "helipad",
        "the newly presented slot must remain usable"
    );
    assert_eq!(state.engine.snapshot().seed_text, "helipad");
    assert_eq!(
        state.engine.snapshot().committed_text,
        before.committed_text
    );
    assert_eq!(state.completion_history.labels(), ["helipad"]);
    state.engine.configure_prediction(None);
}

struct AckServer {
    path: PathBuf,
    ack: Arc<AtomicU8>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl AckServer {
    fn start() -> Self {
        let path =
            PathBuf::from(std::env::var_os("SUZAKU_LINUX_IME_SOCKET").expect("private socket"));
        let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
        assert!(path.starts_with(&runtime) && path != runtime);
        assert!(
            !path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        );
        assert!(
            std::fs::symlink_metadata(&path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "never replace an existing socket or file"
        );
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let ack = Arc::new(AtomicU8::new(b'1'));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (server_ack, server_requests, server_stop) =
            (ack.clone(), requests.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !server_stop.load(Ordering::Acquire) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_millis(200)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_millis(200)))
                            .unwrap();
                        let mut request = String::new();
                        (&mut stream)
                            .take(16 * 1024)
                            .read_to_string(&mut request)
                            .unwrap();
                        if let Some(text) = request.strip_prefix('C') {
                            server_requests.lock().unwrap().push(text.to_owned());
                            let _ = stream.write_all(&[server_ack.load(Ordering::Acquire)]);
                        } else {
                            let _ = stream.write_all(b"0");
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("private ACK fixture accept failed: {error}"),
                }
            }
        });
        Self {
            path,
            ack,
            requests,
            stop,
            thread: Some(thread),
        }
    }
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for AckServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        std::fs::remove_file(&self.path).expect("remove only the socket this fixture created");
    }
}

fn enter(state: &mut PanelState) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !current_ime_host_dispatch().marked_text_roundtrip {
        assert!(
            Instant::now() < deadline,
            "the explicitly ready local bridge did not initialize"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let server = AckServer::start();
    for text in ["hello", "help"] {
        reset(state);
        native_sync::present_test_frame(state);
        let selected = state
            .engine
            .candidates()
            .iter()
            .position(|candidate| candidate.text == text)
            .unwrap();
        assert_ne!(selected, 0);
        state.move_candidate_selection(selected as isize);
        native_sync::present_test_frame(state);
        assert_eq!(state.engine.selected_completion_text(true), Some(text));
        let before = server.requests().len();
        state.commit_keyboard_candidate();
        assert_eq!(
            server.requests()[before..],
            [text],
            "Enter replaced an explicit choice with the first candidate"
        );
        assert!(state.chrome.seed_text.is_empty());
        assert!(state.engine.snapshot().seed_text.is_empty());
        state.commit_keyboard_candidate();
        assert_eq!(
            server.requests().len(),
            before + 1,
            "completed drafts must not replay"
        );
    }
    // Without an explicit choice retain the pre-existing primary-sentence action.
    reset(state);
    native_sync::present_test_frame(state);
    assert_eq!(state.engine.selected_completion_text(true), None);
    let primary = state.engine.candidates()[state.chrome.sentence_candidate_source_indices[0]]
        .text
        .clone();
    let before = server.requests().len();
    state.commit_keyboard_candidate();
    assert_eq!(server.requests()[before..], [primary]);
    assert!(state.chrome.seed_text.is_empty());

    // A rejection retains both the selected bytes and the editable source.
    reset(state);
    native_sync::present_test_frame(state);
    let selected = state
        .engine
        .candidates()
        .iter()
        .position(|candidate| candidate.text == "help")
        .unwrap();
    state.move_candidate_selection(selected as isize);
    native_sync::present_test_frame(state);
    let before = state.engine.snapshot();
    let seed = state.chrome.seed_text.clone();
    let count = server.requests().len();
    server.ack.store(b'0', Ordering::Release);
    state.commit_keyboard_candidate();
    assert_eq!(server.requests()[count..], ["help"]);
    assert_eq!(state.engine.snapshot(), before);
    assert_eq!(state.chrome.seed_text, seed);
    assert_eq!(state.engine.selected_completion_text(true), Some("help"));
    for _ in 0..3 {
        assert!(!prediction_feedback::poll(&mut state.engine).changed());
        native_sync::present_test_frame(state);
    }
    assert_eq!(
        server.requests().len(),
        count + 1,
        "failure presentation must not retry delivery"
    );

    // Even an identical remeasure is not a newly presented keyboard target.
    // Consume an Enter while the scene is dirty without sending unseen state;
    // after an actual presentation, the ordinary primary action still works.
    reset(state);
    native_sync::present_test_frame(state);
    state.refresh_composition_candidates();
    assert!(state.last_scene.is_none());
    let before = state.engine.snapshot();
    let chrome = state.chrome.clone();
    let history = state.completion_history.labels();
    let revision = state.completion_history.revision();
    let count = server.requests().len();
    server.ack.store(b'1', Ordering::Release);
    state.commit_keyboard_candidate();
    assert_eq!(
        server.requests().len(),
        count,
        "unpresented Enter reached the wire"
    );
    assert_eq!(state.engine.snapshot(), before);
    assert_eq!(state.chrome, chrome);
    assert_eq!(state.completion_history.labels(), history);
    assert_eq!(state.completion_history.revision(), revision);
    native_sync::present_test_frame(state);
    let primary = state.engine.candidates()[state.chrome.sentence_candidate_source_indices[0]]
        .text
        .clone();
    state.commit_keyboard_candidate();
    assert_eq!(server.requests()[count..], [primary]);
    assert!(state.chrome.seed_text.is_empty());
    assert!(state.engine.snapshot().seed_text.is_empty());
}

struct CaptionCandidates(Vec<Candidate>);

impl suzaku_map::ime::LanguagePlugin for CaptionCandidates {
    fn id(&self) -> &str {
        "candidate-caption-qa"
    }

    fn expand_token(&self, token: &str, _: bool) -> Vec<String> {
        vec![token.into()]
    }

    fn direct_candidates(&self, _: &str, _: f32) -> Option<Vec<Candidate>> {
        Some(self.0.clone())
    }

    fn build_candidates(&self, _: &[String], _: &str, _: f32) -> Vec<Candidate> {
        self.0.clone()
    }
}

fn exact_candidate_identity(state: &mut PanelState) {
    let server = AckServer::start();
    for entries in [
        [
            ("Hello world.", "Hello world."),
            ("hello world!", "hello world!"),
        ],
        [
            ("first payload", "Same caption"),
            ("second payload", "Same caption"),
        ],
    ] {
        state.engine.register_language_plugin(CaptionCandidates(
            entries
                .into_iter()
                .enumerate()
                .map(|(index, (text, label))| Candidate {
                    text: text.into(),
                    label: label.into(),
                    score: 100.0 - index as f32,
                    kind: Default::default(),
                    source: Default::default(),
                })
                .collect(),
        ));
        state.engine.set_language("candidate-caption-qa");
        for (selected, entry) in entries.iter().enumerate() {
            reset(state);
            native_sync::present_test_frame(state);
            assert_eq!(state.chrome.sentence_candidate_source_indices, [0, 1]);
            let expected = entry.0;
            let count = server.requests().len();
            // Traverse the same presented press/release path as a sentence card.
            let rect = state
                .last_scene
                .as_ref()
                .unwrap()
                .interactive_targets
                .iter()
                .find(|target| target.kind == InteractionKind::Candidate(selected))
                .unwrap()
                .rect;
            state.cursor_position = Some((rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0));
            state.begin_primary_press(false);
            state.complete_primary_release(false);
            assert_eq!(
                server.requests()[count..],
                [expected],
                "the displayed source index must preserve the exact selected payload"
            );
            assert!(state.chrome.seed_text.is_empty());
        }

        for invalid in [0, 1, usize::MAX] {
            reset(state);
            state.chrome.sentence_candidates = vec!["obsolete caption".into()];
            state.chrome.sentence_candidate_source_indices = vec![0];
            let before = state.engine.snapshot();
            let chrome = state.chrome.clone();
            let count = server.requests().len();
            assert!(!state.commit_sentence_candidate(invalid));
            assert_eq!(
                server.requests().len(),
                count,
                "unknown identities must not send"
            );
            assert_eq!(state.engine.snapshot(), before);
            assert_eq!(state.chrome, chrome);
        }

        reset(state);
        state.chrome.sentence_candidate_source_indices.clear();
        let before = state.engine.snapshot();
        let count = server.requests().len();
        assert!(!state.commit_primary_sentence_candidate());
        assert_eq!(
            server.requests().len(),
            count,
            "a caption without an identity is not a send target"
        );
        assert_eq!(state.engine.snapshot(), before);
    }
    state.engine.set_language("en");
    reset(state);
}

fn unresolved_enter_identity(state: &mut PanelState) {
    let server = AckServer::start();
    for missing_mapping in [false, true] {
        reset(state);
        if missing_mapping {
            state.chrome.sentence_candidate_source_indices.clear();
        } else {
            state.chrome.sentence_candidates[0] = "obsolete caption".into();
        }
        native_sync::present_test_frame(state);
        let before = state.engine.snapshot();
        let chrome = state.chrome.clone();
        assert!(state.engine.selected_completion_text(true).is_none());
        state.commit_keyboard_candidate();
        assert!(
            server.requests().is_empty(),
            "Enter must not replace an unresolved card with the default candidate"
        );
        assert_eq!(state.engine.snapshot(), before);
        assert_eq!(state.chrome, chrome);
    }
}

fn assert_visible_local_choice(state: &PanelState, source: usize, caption: &str) {
    let scene = state
        .last_scene
        .as_ref()
        .expect("actually presented choices");
    assert!(scene.interactive_targets.iter().any(|target| {
        target.kind == InteractionKind::Candidate(source)
            && target.rect[2] > 0.0
            && target.rect[3] > 0.0
    }));
    assert!(
        scene
            .text_sections
            .iter()
            .filter(|section| section.role == suzaku_map::ime::gpu::TextRole::CandidatePrimary)
            .flat_map(|section| &section.layouts)
            .flat_map(|layout| &layout.lines)
            .any(|line| line.contains(caption)),
        "the selected caption must be drawn, not merely present in scene metadata"
    );
}

fn visible_local_selection_boundaries(state: &mut PanelState) {
    const ENTRIES: [(&str, &str); 6] = [
        ("fixture complete payload A", "PickA"),
        ("fixture complete payload B", "PickB"),
        ("fixture complete payload C", "PickC"),
        ("fixture complete payload D", "PickD"),
        ("fixture complete payload E", "PickE"),
        ("fixture complete payload F", "PickF"),
    ];
    let mut fixture = XRTabletImeEngine::new(EngineConfig::default());
    fixture.set_preferences_enabled(false);
    fixture.register_language_plugin(CaptionCandidates(
        ENTRIES
            .iter()
            .enumerate()
            .map(|(index, (text, label))| Candidate {
                text: (*text).into(),
                label: (*label).into(),
                score: 100.0 - index as f32,
                kind: suzaku_map::ime::candidate_mix::CandidateKind::Sentence,
                source: Default::default(),
            })
            .collect(),
    ));
    fixture.set_language("candidate-caption-qa");
    let original_engine = std::mem::replace(&mut state.engine, fixture);
    let original_chrome = state.chrome.clone();
    let original_history = std::mem::take(&mut state.completion_history);
    let original_native = std::mem::take(&mut state.native);
    state.is_focused = false;
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    let server = AckServer::start();
    server.ack.store(b'0', Ordering::Release);

    for selected in [4usize, 5] {
        reset(state);
        native_sync::present_test_frame(state);
        assert_eq!(state.engine.candidates().len(), 6);
        assert!(state.next_token_completions.is_empty());
        assert_eq!(state.chrome.sentence_candidate_source_indices, [0, 1, 2, 3]);
        let before = state.engine.snapshot();
        let scene = state.last_scene.clone();
        state.move_candidate_selection(0);
        state.move_candidate_selection(-1);
        assert_eq!(state.engine.snapshot(), before);
        assert_eq!(
            state.last_scene, scene,
            "boundary navigation must retain presentation"
        );
        assert!(state.engine.selected_completion_text(true).is_none());

        state.move_candidate_selection(selected as isize);
        assert_eq!(state.engine.snapshot().selected_index, selected);
        let before = state.engine.snapshot();
        let history = (
            state.completion_history.revision(),
            state.completion_history.labels(),
        );
        let count = server.requests().len();
        state.commit_keyboard_candidate();
        assert_eq!(
            server.requests().len(),
            count,
            "a newly exposed hidden selection must wait for real presentation: {selected}"
        );
        assert_eq!(state.engine.snapshot(), before);
        assert_eq!(
            (
                state.completion_history.revision(),
                state.completion_history.labels()
            ),
            history
        );
        native_sync::present_test_frame(state);
        assert_visible_local_choice(state, selected, ENTRIES[selected].1);
        state.commit_keyboard_candidate();
        assert_eq!(server.requests()[count..], [ENTRIES[selected].0]);
        assert_eq!(
            state.engine.snapshot(),
            before,
            "rejected delivery must retain selection"
        );
        assert_eq!(state.chrome.seed_text, "hel");

        state.move_candidate_selection(2 - selected as isize);
        assert_eq!(state.engine.snapshot().selected_index, 2);
        assert_eq!(state.chrome.sentence_candidate_source_indices, [0, 1, 2, 3]);
        native_sync::present_test_frame(state);
        assert_visible_local_choice(state, 2, ENTRIES[2].1);
        if selected == 5 {
            state.move_candidate_selection(3);
            native_sync::present_test_frame(state);
            assert_visible_local_choice(state, 5, ENTRIES[5].1);
            assert_eq!(state.chrome.sentence_candidate_source_indices[3], 5);
            let count = server.requests().len();
            assert!(state.try_numbered_candidate(3));
            assert_eq!(state.chrome.seed_text, ENTRIES[5].0);
            assert_eq!(state.engine.snapshot().seed_text, ENTRIES[5].0);
            assert!(state.engine.snapshot().committed_text.is_empty());
            assert_eq!(
                server.requests().len(),
                count,
                "number adoption is an editable draft"
            );
            assert!(!state.completion_history.labels().is_empty());
        }
    }

    for explicit in [false, true] {
        reset(state);
        state.continue_sentence_candidate(0);
        assert!(!state.completion_history.labels().is_empty());
        if explicit {
            state.move_candidate_selection(4);
        }
        native_sync::present_test_frame(state);
        let selected = if explicit { 4 } else { 0 };
        assert_visible_local_choice(state, selected, ENTRIES[selected].1);
        assert_eq!(
            state.engine.selected_completion_text(true).is_some(),
            explicit
        );
        show_translation(state);
        native_sync::present_test_frame(state);
        assert!(
            state
                .last_scene
                .as_ref()
                .unwrap()
                .interactive_targets
                .iter()
                .all(|target| { !matches!(target.kind, InteractionKind::Candidate(_)) })
        );
        let before = state.engine.snapshot();
        let seed = state.chrome.seed_text.clone();
        let history = (
            state.completion_history.revision(),
            state.completion_history.labels(),
        );
        let count = server.requests().len();
        state.commit_keyboard_candidate();
        assert_eq!(
            server.requests().len(),
            count,
            "Translation hides every send choice: {explicit}"
        );
        assert_eq!(state.engine.snapshot(), before);
        assert_eq!(state.chrome.seed_text, seed);
        assert_eq!(
            (
                state.completion_history.revision(),
                state.completion_history.labels()
            ),
            history
        );
        state.chrome.active_input_mode = InputMode::VirtualKeyboard;
        state.chrome.input_modes_expanded = false;
        state.chrome.blur_input();
        native_sync::present_test_frame(state);
        assert_visible_local_choice(state, selected, ENTRIES[selected].1);
        state.commit_keyboard_candidate();
        assert_eq!(server.requests()[count..], [ENTRIES[selected].0]);
        assert_eq!(state.engine.snapshot(), before);
        assert_eq!(state.chrome.seed_text, seed);
    }

    reset(state);
    state.chrome.set_seed_text(String::new());
    state.chrome.move_caret_to_end();
    state.refresh_seed();
    native_sync::present_test_frame(state);
    let before = state.engine.snapshot();
    let scene = state.last_scene.clone();
    let count = server.requests().len();
    state.move_candidate_selection(1);
    assert_eq!(state.engine.snapshot(), before);
    assert_eq!(state.last_scene, scene);
    assert!(state.chrome.sentence_candidates.is_empty());
    state.commit_keyboard_candidate();
    assert_eq!(
        server.requests().len(),
        count,
        "empty drafts cannot acquire a candidate"
    );
    assert_eq!(state.engine.snapshot(), before);
    state.engine = original_engine;
    state.chrome = original_chrome;
    state.completion_history = original_history;
    state.native = original_native;
    state.refresh_composition_candidates();
}

fn occluded_pointer_event(state: &mut PanelState, events: &ActiveEventLoop, event: WindowEvent) {
    assert!(input::handle_panel_window_event(
        state, events, event, false
    ));
}

fn occluded_pointer_gesture(
    state: &mut PanelState,
    events: &ActiveEventLoop,
    touch: bool,
    phase: winit::event::TouchPhase,
    point: (f64, f64),
) {
    use winit::event::{DeviceId, ElementState, MouseButton, Touch, TouchPhase};
    let device_id = DeviceId::dummy();
    let position = winit::dpi::PhysicalPosition::new(point.0, point.1);
    if touch {
        occluded_pointer_event(
            state,
            events,
            WindowEvent::Touch(Touch {
                device_id,
                phase,
                location: position,
                force: None,
                id: 811,
            }),
        );
    } else {
        occluded_pointer_event(
            state,
            events,
            WindowEvent::CursorMoved {
                device_id,
                position,
            },
        );
        let element_state = match phase {
            TouchPhase::Started => ElementState::Pressed,
            TouchPhase::Ended => ElementState::Released,
            _ => panic!("the controlled mouse fixture only presses and releases"),
        };
        occluded_pointer_event(
            state,
            events,
            WindowEvent::MouseInput {
                device_id,
                state: element_state,
                button: MouseButton::Left,
            },
        );
    }
}

fn occluded_pointer_boundaries(state: &mut PanelState, events: &ActiveEventLoop) {
    use winit::event::TouchPhase;
    // Controlled production event dispatch in the private X11 fixture, not a
    // claim that a real compositor occlusion or physical touch was observed.
    state.engine.set_language("en");
    state.engine.set_preferences_enabled(false);
    state.runs_without_window_focus = true;
    state.is_focused = false;
    state.chrome.active_input_mode = InputMode::VirtualKeyboard;
    let server = AckServer::start();
    server.ack.store(b'0', Ordering::Release);
    let prepare = |state: &mut PanelState| {
        state.cancel_primary_interaction();
        state.native = Default::default();
        reset(state);
        native_sync::present_test_frame(state);
        assert_eq!(state.engine.candidates()[1].text, "hello");
        assert_visible_local_choice(state, 1, "hello");
        let rect = state
            .last_scene
            .as_ref()
            .unwrap()
            .interactive_targets
            .iter()
            .find(|target| target.kind == InteractionKind::Candidate(1))
            .unwrap()
            .rect;
        (
            f64::from(rect[0] + rect[2] * 0.5),
            f64::from(rect[1] + rect[3] * 0.5),
        )
    };
    let retained = |state: &PanelState, before: &Snapshot, history: &(u64, Vec<String>)| {
        assert_eq!(&state.engine.snapshot(), before);
        assert_eq!(state.chrome.seed_text, "hel");
        assert_eq!(
            (
                state.completion_history.revision(),
                state.completion_history.labels()
            ),
            *history
        );
    };
    for touch in [false, true] {
        for case in [
            "covered release",
            "covered presented release",
            "uncovered unpresented release",
            "uncovered presented release",
        ] {
            let point = prepare(state);
            let before = state.engine.snapshot();
            let history = (
                state.completion_history.revision(),
                state.completion_history.labels(),
            );
            let count = server.requests().len();
            occluded_pointer_gesture(state, events, touch, TouchPhase::Started, point);
            assert_eq!(
                state.interaction.pressed_interaction,
                Some(InteractionKind::Candidate(1))
            );
            occluded_pointer_event(state, events, WindowEvent::Occluded(true));
            if case == "covered presented release" {
                native_sync::present_test_frame(state);
            }
            if case.starts_with("uncovered") {
                occluded_pointer_event(state, events, WindowEvent::Occluded(false));
                if case == "uncovered presented release" {
                    native_sync::present_test_frame(state);
                }
            }
            occluded_pointer_gesture(state, events, touch, TouchPhase::Ended, point);
            assert_eq!(
                server.requests().len(),
                count,
                "a retired press sent a candidate: touch={touch}, {case}"
            );
            retained(state, &before, &history);
            occluded_pointer_event(state, events, WindowEvent::Occluded(false));
            native_sync::present_test_frame(state);
            occluded_pointer_gesture(state, events, touch, TouchPhase::Started, point);
            occluded_pointer_gesture(state, events, touch, TouchPhase::Ended, point);
            assert_eq!(server.requests()[count..], ["hello"]);
            assert_eq!(state.chrome.seed_text, "hel");
            assert_eq!(
                state.engine.snapshot().committed_text,
                before.committed_text
            );
            assert_eq!(state.completion_history.labels(), history.1);
        }

        let point = prepare(state);
        let before = state.engine.snapshot();
        let history = (
            state.completion_history.revision(),
            state.completion_history.labels(),
        );
        let count = server.requests().len();
        occluded_pointer_event(state, events, WindowEvent::Occluded(true));
        native_sync::present_test_frame(state);
        occluded_pointer_gesture(state, events, touch, TouchPhase::Started, point);
        assert!(
            state.interaction.pressed_interaction.is_none(),
            "a covered redraw rearmed a new press: touch={touch}"
        );
        assert!(
            state.interaction.touch_owner.is_none(),
            "a covered touch acquired an owner"
        );
        occluded_pointer_gesture(state, events, touch, TouchPhase::Ended, point);
        assert_eq!(server.requests().len(), count);
        retained(state, &before, &history);
        occluded_pointer_event(state, events, WindowEvent::Occluded(false));
        occluded_pointer_gesture(state, events, touch, TouchPhase::Started, point);
        occluded_pointer_gesture(state, events, touch, TouchPhase::Ended, point);
        assert_eq!(
            server.requests().len(),
            count,
            "uncovering alone is not a new presentation: touch={touch}"
        );
        retained(state, &before, &history);
        native_sync::present_test_frame(state);
        occluded_pointer_gesture(state, events, touch, TouchPhase::Started, point);
        occluded_pointer_gesture(state, events, touch, TouchPhase::Ended, point);
        assert_eq!(server.requests()[count..], ["hello"]);
        assert_eq!(state.chrome.seed_text, "hel");
        assert_eq!(
            state.engine.snapshot().committed_text,
            before.committed_text
        );

        // X11 also reports false for partial-visibility notifications. A
        // duplicate false must not retire a press that stayed fully usable.
        let point = prepare(state);
        let before = state.engine.snapshot();
        let count = server.requests().len();
        occluded_pointer_gesture(state, events, touch, TouchPhase::Started, point);
        occluded_pointer_event(state, events, WindowEvent::Occluded(false));
        occluded_pointer_gesture(state, events, touch, TouchPhase::Ended, point);
        assert_eq!(server.requests()[count..], ["hello"]);
        assert_eq!(state.chrome.seed_text, "hel");
        assert_eq!(
            state.engine.snapshot().committed_text,
            before.committed_text
        );
    }
}

struct GestureSentenceCandidates {
    language: &'static str,
    candidate: Candidate,
}

impl suzaku_map::ime::LanguagePlugin for GestureSentenceCandidates {
    fn id(&self) -> &str {
        self.language
    }

    fn expand_token(&self, token: &str, _: bool) -> Vec<String> {
        vec![token.into()]
    }

    fn direct_candidates(&self, _: &str, _: f32) -> Option<Vec<Candidate>> {
        Some(vec![self.candidate.clone()])
    }

    fn build_candidates(&self, _: &[String], _: &str, _: f32) -> Vec<Candidate> {
        vec![self.candidate.clone()]
    }
}

fn install_gesture_sentence(
    state: &mut PanelState,
    language: &'static str,
    seed: &str,
    payload: &str,
    caption: &str,
) {
    // Exercise the public plugin-reload path, not mutable engine internals or
    // an HTTP provider that cannot independently choose its display caption.
    state
        .engine
        .register_language_plugin(GestureSentenceCandidates {
            language,
            candidate: Candidate {
                text: payload.into(),
                label: caption.into(),
                score: 100.0,
                kind: suzaku_map::ime::candidate_mix::CandidateKind::Sentence,
                source: Default::default(),
            },
        });
    state.engine.set_language(language);
    state.chrome.set_seed_text(seed.into());
    state.chrome.move_caret_to_end();
    state.engine.seed(seed);
    state.refresh_composition_candidates();
    assert!(state.next_token_completions.is_empty());
    assert_eq!(state.chrome.sentence_candidate_source_indices, [0]);
    assert_eq!(state.chrome.sentence_candidates, [caption]);
    assert_eq!(state.engine.candidates()[0].text, payload);
}

fn press_gesture_sentence(state: &mut PanelState, touch: bool, truncated: bool) -> [f32; 4] {
    native_sync::present_test_frame(state);
    let scene = state.last_scene.as_ref().unwrap();
    assert_eq!(scene.sentence_candidate_truncated.contains(&0), truncated);
    let rect = scene
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::Candidate(0))
        .unwrap()
        .rect;
    let cursor = (rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5);
    assert_eq!(
        scene.hit_interaction(cursor.0, cursor.1),
        Some(InteractionKind::Candidate(0))
    );
    state.cursor_position = Some(cursor);
    state.interaction.last_input_was_touch = touch;
    state.begin_primary_press(touch);
    assert_eq!(
        state.interaction.pressed_interaction,
        Some(InteractionKind::Candidate(0))
    );
    rect
}

fn held_sentence_identity_boundaries(state: &mut PanelState) {
    for touch in [false, true] {
        for boundary in ["payload", "seed", "language", "unchanged"] {
            reset(state);
            install_gesture_sentence(
                state,
                "gesture-sentence-a",
                "hel",
                "first payload",
                "Shared caption",
            );
            let server = AckServer::start();
            let rect = press_gesture_sentence(state, touch, false);
            let started = state.interaction.press_start_instant.unwrap();
            let expected = if boundary == "payload" {
                "second payload"
            } else {
                "first payload"
            };
            match boundary {
                "payload" => install_gesture_sentence(
                    state,
                    "gesture-sentence-a",
                    "hel",
                    expected,
                    "Shared caption",
                ),
                "seed" => install_gesture_sentence(
                    state,
                    "gesture-sentence-a",
                    "new",
                    expected,
                    "Shared caption",
                ),
                "language" => install_gesture_sentence(
                    state,
                    "gesture-sentence-b",
                    "hel",
                    expected,
                    "Shared caption",
                ),
                "unchanged" => state.refresh_composition_candidates(),
                _ => unreachable!(),
            }
            // Repainting must not turn an old press into permission for the new
            // payload or source draft, even when its caption/geometry agree.
            native_sync::present_test_frame(state);
            let scene = state.last_scene.as_ref().unwrap();
            assert_eq!(
                scene
                    .interactive_targets
                    .iter()
                    .find(|target| target.kind == InteractionKind::Candidate(0))
                    .unwrap()
                    .rect,
                rect,
                "the identity regression must not pass by moving the target: {boundary}, touch={touch}",
            );
            assert!(
                started.elapsed()
                    <= Duration::from_millis(u64::from(state.chrome.pointer_tap_max_ms)),
                "fixture exceeded the unchanged production tap deadline: {boundary}, touch={touch}",
            );
            let before = state.engine.snapshot();
            let chrome = state.chrome.clone();
            let history = state.completion_history.labels();
            let revision = state.completion_history.revision();
            state.complete_primary_release(touch);
            if boundary == "unchanged" {
                assert_eq!(
                    server.requests(),
                    [expected],
                    "an identical refresh lost the acquired candidate: touch={touch}"
                );
                assert!(state.chrome.seed_text.is_empty());
            } else {
                assert!(
                    server.requests().is_empty(),
                    "an old press sent a replacement candidate: {boundary}, touch={touch}"
                );
                assert_eq!(state.engine.snapshot(), before, "{boundary}, touch={touch}");
                assert_eq!(state.chrome, chrome, "{boundary}, touch={touch}");
                assert_eq!(state.completion_history.labels(), history);
                assert_eq!(state.completion_history.revision(), revision);
                press_gesture_sentence(state, touch, false);
                state.complete_primary_release(touch);
                assert_eq!(
                    server.requests(),
                    [expected],
                    "a fresh press must still send the current candidate: {boundary}, touch={touch}"
                );
                assert!(state.chrome.seed_text.is_empty());
            }
            let completed = state.engine.snapshot();
            state.complete_primary_release(touch);
            assert_eq!(
                server.requests(),
                [expected],
                "one press must never send twice"
            );
            assert_eq!(state.engine.snapshot(), completed);
        }
    }
    state.engine.set_language("en");
    reset(state);
}

fn sentence_preview_identity_boundaries(state: &mut PanelState) {
    let first =
        "First complete candidate needs its own readable scrolling preview before sending. "
            .repeat(12);
    let second =
        "Other complete candidate needs its own readable scrolling preview before sending. "
            .repeat(12);
    for touch in [false, true] {
        for boundary in ["payload", "seed", "language", "unchanged"] {
            reset(state);
            install_gesture_sentence(state, "gesture-sentence-a", "hel", &first, &first);
            let server = AckServer::start();
            press_gesture_sentence(state, touch, true);
            let initial = state.engine.snapshot();
            state.complete_primary_release(touch);
            assert!(server.requests().is_empty());
            assert_eq!(state.engine.snapshot(), initial);
            assert_eq!(state.interaction.sentence_candidate_scroll_index, Some(0));
            assert!(
                state
                    .interaction
                    .sentence_candidate_scroll_started_at
                    .is_some()
            );

            let expected = if boundary == "payload" {
                &second
            } else {
                &first
            };
            match boundary {
                "payload" => {
                    install_gesture_sentence(state, "gesture-sentence-a", "hel", expected, expected)
                }
                "seed" => {
                    install_gesture_sentence(state, "gesture-sentence-a", "new", expected, expected)
                }
                "language" => {
                    install_gesture_sentence(state, "gesture-sentence-b", "hel", expected, expected)
                }
                "unchanged" => state.refresh_composition_candidates(),
                _ => unreachable!(),
            }
            press_gesture_sentence(state, touch, true);
            let before = state.engine.snapshot();
            let chrome = state.chrome.clone();
            let history = state.completion_history.labels();
            let revision = state.completion_history.revision();
            state.complete_primary_release(touch);
            if boundary != "unchanged" {
                assert!(
                    server.requests().is_empty(),
                    "a new candidate inherited another preview's confirmation: {boundary}, touch={touch}"
                );
                assert_eq!(state.engine.snapshot(), before);
                assert_eq!(state.chrome, chrome);
                assert_eq!(state.completion_history.labels(), history);
                assert_eq!(state.completion_history.revision(), revision);
                assert_eq!(state.interaction.sentence_candidate_scroll_index, Some(0));
                assert!(
                    state
                        .interaction
                        .sentence_candidate_scroll_started_at
                        .is_some()
                );
                press_gesture_sentence(state, touch, true);
                state.complete_primary_release(touch);
            }
            assert_eq!(
                server.requests(),
                [expected.as_str()],
                "the current candidate must send only after its own preview: {boundary}, touch={touch}"
            );
            assert!(state.chrome.seed_text.is_empty());
            let completed = state.engine.snapshot();
            state.complete_primary_release(touch);
            assert_eq!(server.requests(), [expected.as_str()]);
            assert_eq!(state.engine.snapshot(), completed);
        }
    }
    state.engine.set_language("en");
    reset(state);
}

fn native_recovery_survives_focus_before_editing(state: &mut PanelState) {
    use suzaku_map::ime::companion::{NativeCandidate, NativeOperation};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::protocol::xproto::{ConnectionExt, MapState};

    // The non-focusing fixture deliberately starts hidden. A real focus lease
    // needs a mapped window; do not confuse that setup failure with recovery.
    let was_visible = state.window.is_visible();
    state.window.set_visible(true);
    let window_id = match state.window.window_handle().unwrap().as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window as u32,
        _ => panic!("the private recovery fixture requires its own X11 window"),
    };
    let (connection, _) = x11rb::connect(None).unwrap();
    let map_deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let attributes = connection
            .get_window_attributes(window_id)
            .unwrap()
            .reply()
            .unwrap();
        if attributes.map_state == MapState::VIEWABLE {
            break;
        }
        assert!(
            Instant::now() < map_deadline,
            "the private recovery window must be viewable before requesting focus",
        );
        std::thread::sleep(Duration::from_millis(2));
    }

    let original_chrome = state.chrome.clone();
    let original_native = std::mem::take(&mut state.native);
    let mut fixture = XRTabletImeEngine::new(EngineConfig::default());
    fixture.set_preferences_enabled(false);
    let original_engine = std::mem::replace(&mut state.engine, fixture);
    let saved = "saved local draft";
    for source in [
        "no queued draft",
        "pending literal",
        "semantic space",
        "semantic backspace",
        "confirmed action",
    ] {
        for focus_first in [false, true] {
            state.finish_text_editing();
            state.set_window_focus(false);
            state.native = Default::default();
            state.chrome.settings_open = false;
            state.chrome.compact_mode = false;
            state.chrome.llm_enabled = false;
            state.chrome.input_modes_expanded = false;
            state.chrome.set_seed_text(saved.into());
            state.chrome.move_caret_to_end();
            state.completion_history.clear();
            state.engine.clear_session_context();
            state.engine.seed(saved);
            state.refresh_composition_candidates();
            let frame = NativeComposition {
                host: "00000000-0000-0000-0000-000000000023".into(),
                context: 23,
                revision: 10,
                focused: true,
                private: false,
                cursor: None,
                language: "en".into(),
                seed: "hel".into(),
                selected: 0,
                candidates: vec![NativeCandidate {
                    text: "hello".into(),
                    label: "hello".into(),
                    ..Default::default()
                }],
            };
            let (sender, receiver) = mpsc::sync_channel(8);
            state.native.sender = Some(sender);
            state.receive_native_frame(Some(frame.clone()));
            assert!(state.native.showing);
            assert_eq!(state.chrome.seed_text, "hel");
            let mut old_reply = None;
            let expected = match source {
                "no queued draft" => saved,
                "pending literal" => {
                    assert!(state.native_keyboard_edit(Some("l")));
                    drop(receiver.try_recv().expect("the first native literal write"));
                    assert!(state.native_keyboard_edit(Some("o")));
                    assert_eq!(state.chrome.seed_text, "hello");
                    old_reply = Some((frame.host.clone(), frame.revision, false));
                    "hello"
                }
                "semantic space" | "semantic backspace" => {
                    assert!(state.native_keyboard_edit(if source == "semantic space" {
                        Some(" ")
                    } else {
                        None
                    }));
                    drop(receiver.try_recv().expect("the native semantic-key flight"));
                    assert!(state.native_keyboard_edit(Some("x")));
                    old_reply = Some((frame.host.clone(), frame.revision, true));
                    // Unknown semantic results cannot be guessed locally. Only
                    // the last public base and literal follow-up are recovered.
                    "helx"
                }
                "confirmed action" => {
                    assert!(state.native_action(NativeOperation::Adopt(0)));
                    drop(receiver.try_recv().expect("native adoption request"));
                    state.native_action_finished(frame.host.clone(), frame.revision, Ok(true));
                    old_reply = Some((frame.host.clone(), frame.revision, false));
                    "hello"
                }
                _ => unreachable!(),
            };
            assert!(
                receiver.try_recv().is_err(),
                "one native operation at a time"
            );

            // These are production callees in both possible event orders, not
            // an assertion about the private X server or a real WM's ordering.
            if focus_first {
                state.set_window_focus(true);
                state.begin_text_editing();
            } else {
                state.begin_text_editing();
                state.set_window_focus(true);
            }
            assert!(!state.native.showing);
            assert_eq!(
                state.chrome.seed_text, expected,
                "native recovery was lost: source={source}, focus_first={focus_first}",
            );
            assert_eq!(state.engine.snapshot().seed_text, expected);
            assert_eq!(state.chrome.caret_index, expected.chars().count());
            assert!(state.chrome.input_focused);
            assert!(state.completion_history.labels().is_empty());
            assert!(receiver.try_recv().is_err(), "recovery must remain local");

            state.handle_text_input("!");
            let edited = format!("{expected}!");
            assert_eq!(state.chrome.seed_text, edited);
            let before = state.engine.snapshot();
            let chrome = state.chrome.clone();
            if let Some((host, revision, semantic)) = old_reply {
                if semantic {
                    let mut reply = frame.clone();
                    reply.revision += 1;
                    reply.seed = "an old semantic reply".into();
                    state.native_keyboard_finished(host, revision, Ok(reply));
                } else {
                    state.native_action_finished(host, revision, Ok(true));
                }
            }
            let mut late_frame = frame;
            late_frame.revision += 2;
            late_frame.seed = "a later external frame".into();
            state.receive_native_frame(Some(late_frame));
            assert_eq!(state.engine.snapshot(), before);
            assert_eq!(state.chrome, chrome);
            assert!(state.engine.snapshot().committed_text.is_empty());
            assert!(
                receiver.try_recv().is_err(),
                "late replies must not replay or replace the edited local recovery",
            );
        }
    }
    native_recovery_waits_for_explicit_editing_after_focus_out(state);
    state.finish_text_editing();
    state.set_window_focus(false);
    state.native = original_native;
    state.engine = original_engine;
    state.chrome = original_chrome;
    state.refresh_composition_candidates();
    if was_visible == Some(false) {
        state.window.set_visible(false);
    }
}

fn native_recovery_waits_for_explicit_editing_after_focus_out(state: &mut PanelState) {
    use suzaku_map::ime::companion::{NativeCandidate, NativeOperation};

    let saved = "saved local draft";
    for (source, followup) in [
        ("no queued draft", "recover"),
        ("pending literal", "recover"),
        ("semantic space", "recover"),
        ("semantic backspace", "recover"),
        ("confirmed action", "recover"),
        ("pending literal", "local edit"),
        ("pending literal", "local clear"),
        ("pending literal", "new B typing"),
        ("confirmed action", "disconnect"),
        ("pending empty", "recover"),
    ] {
        state.finish_text_editing();
        state.set_window_focus(false);
        state.native = Default::default();
        state.chrome.settings_open = false;
        state.chrome.compact_mode = false;
        state.chrome.llm_enabled = false;
        state.chrome.input_modes_expanded = false;
        state.chrome.set_seed_text(saved.into());
        state.chrome.move_caret_to_end();
        state.completion_history.clear();
        state.engine.clear_session_context();
        state.engine.seed(saved);
        state.refresh_composition_candidates();
        let frame = NativeComposition {
            host: "00000000-0000-0000-0000-000000000023".into(),
            context: 23,
            revision: 10,
            focused: true,
            private: false,
            cursor: None,
            language: "en".into(),
            seed: "hel".into(),
            selected: 0,
            candidates: vec![NativeCandidate {
                text: "hello".into(),
                label: "hello".into(),
                ..Default::default()
            }],
        };
        let (sender, receiver) = mpsc::sync_channel(8);
        state.native.sender = Some(sender);
        state.receive_native_frame(Some(frame.clone()));
        let expected = match source {
            "no queued draft" => saved,
            "pending literal" => {
                assert!(state.native_keyboard_edit(Some("l")));
                drop(receiver.try_recv().expect("A's initial literal write"));
                assert!(state.native_keyboard_edit(Some("o")));
                "hello"
            }
            "pending empty" => {
                assert!(state.native_keyboard_edit(Some("l")));
                drop(receiver.try_recv().expect("A's initial empty-draft flight"));
                for _ in 0..4 {
                    assert!(state.native_keyboard_edit(None));
                }
                assert!(state.chrome.seed_text.is_empty());
                ""
            }
            "semantic space" | "semantic backspace" => {
                assert!(state.native_keyboard_edit(if source == "semantic space" {
                    Some(" ")
                } else {
                    None
                }));
                drop(receiver.try_recv().expect("A's semantic flight"));
                assert!(state.native_keyboard_edit(Some("x")));
                "helx"
            }
            "confirmed action" => {
                assert!(state.native_action(NativeOperation::Adopt(0)));
                drop(receiver.try_recv().expect("A's adoption request"));
                state.native_action_finished(frame.host.clone(), frame.revision, Ok(true));
                "hello"
            }
            _ => unreachable!(),
        };
        assert!(receiver.try_recv().is_err());
        if followup == "disconnect" {
            // A confirmed-action-only draft also needs a local copy when the
            // reader disconnects before its authoritative replacement frame.
            state.receive_native_frame(None);
            assert!(receiver.try_recv().is_err());
        }

        // The real host increments context at FocusOut and redacts the entire
        // inactive composition. This is not a fabricated same-context update.
        let mut inactive = frame.clone();
        inactive.context += 1;
        inactive.revision += 1;
        inactive.focused = false;
        inactive.seed.clear();
        inactive.candidates.clear();
        inactive.selected = 0;
        inactive.cursor = None;
        assert_eq!(
            NativeComposition::parse(&serde_json::to_vec(&inactive.to_json()).unwrap()).unwrap(),
            inactive
        );
        state.receive_native_frame(Some(inactive.clone()));
        inactive.revision += 1;
        state.receive_native_frame(Some(inactive.clone()));
        state.receive_native_frame(None);
        inactive.revision += 1;
        state.receive_native_frame(Some(inactive));

        let mut next = frame.clone();
        next.context += 2;
        next.revision += 4;
        next.seed = "new field B".into();
        next.candidates.clear();
        if source == "semantic space" {
            next.host = "00000000-0000-0000-0000-000000000024".into();
        } else if source == "semantic backspace" {
            next.private = true;
            next.seed.clear();
        } else if source == "confirmed action" {
            next.language = "ja".into();
        }
        assert_eq!(
            NativeComposition::parse(&serde_json::to_vec(&next.to_json()).unwrap()).unwrap(),
            next
        );
        state.receive_native_frame(Some(next.clone()));
        assert!(receiver.try_recv().is_err(), "A must never replay into B");

        if followup == "new B typing" {
            state.native_action_finished(frame.host.clone(), frame.revision, Ok(false));
            assert!(state.native_keyboard_edit(Some("z")));
            drop(
                receiver
                    .try_recv()
                    .expect("only B's explicit new literal may send"),
            );
        }
        state.set_window_focus(true);
        let before_click = if followup == "new B typing" {
            "new field Bz"
        } else {
            saved
        };
        assert_eq!(
            state.chrome.seed_text, before_click,
            "ordinary focus consumed detached A: source={source}, followup={followup}"
        );
        assert!(receiver.try_recv().is_err());

        if source == "semantic space" || source == "semantic backspace" {
            let mut reply = frame.clone();
            reply.revision += 1;
            reply.seed = "old semantic reply".into();
            state.native_keyboard_finished(frame.host.clone(), frame.revision, Ok(reply));
        } else if source != "no queued draft" {
            state.native_action_finished(frame.host.clone(), frame.revision, Ok(true));
        }
        assert_eq!(state.chrome.seed_text, before_click);
        assert!(receiver.try_recv().is_err(), "late A reply must not send");

        let expected = match followup {
            "local edit" => {
                // Mirror the input-focus flag set by the real Focused handler,
                // without consuming the detached recovery through SeedInput.
                state.chrome.focus_input();
                state.chrome.move_caret_to_end();
                state.handle_text_input("!");
                "saved local draft!"
            }
            "local clear" => {
                state.reset_after_commit();
                ""
            }
            "new B typing" => "new field Bz",
            _ => expected,
        };
        state.begin_text_editing();
        assert_eq!(
            state.chrome.seed_text, expected,
            "explicit recovery lost or mixed a source: source={source}, followup={followup}"
        );
        assert_eq!(state.engine.snapshot().seed_text, expected);
        assert!(!state.native.showing);
        assert!(receiver.try_recv().is_err(), "recovery is local only");

        // A fixture-only return to the original baseline must not resurrect a
        // consumed cache. Do not call the production cache-clearing edit path.
        state.chrome.set_seed_text(saved.into());
        state.chrome.move_caret_to_end();
        state.engine.seed(saved);
        state.refresh_composition_candidates();
        state.begin_text_editing();
        assert_eq!(
            state.chrome.seed_text, saved,
            "recovery must be consumed once"
        );
        assert_eq!(state.engine.snapshot().seed_text, saved);
        assert!(receiver.try_recv().is_err());
        assert!(state.engine.snapshot().committed_text.is_empty());
    }
}

#[derive(Debug, PartialEq)]
struct SignalDraft {
    seed: String,
    caret: usize,
    history: Vec<String>,
    revision: u64,
    translation: TranslationView,
    feedback: Option<String>,
    feedback_ticks: u8,
}

impl From<&PanelState> for SignalDraft {
    fn from(state: &PanelState) -> Self {
        Self {
            seed: state.chrome.seed_text.clone(),
            caret: state.chrome.caret_index,
            history: state.completion_history.labels(),
            revision: state.completion_history.revision(),
            translation: state.chrome.translation.clone(),
            feedback: state.last_commit_feedback.clone(),
            feedback_ticks: state.commit_feedback_ticks,
        }
    }
}

#[derive(Debug, PartialEq)]
struct SignalStateView {
    draft: SignalDraft,
    chrome: PanelChromeState,
    engine: Snapshot,
    candidates: Vec<Candidate>,
    selected: Option<String>,
    prediction: PredictionStatus,
    error: Option<PredictionError>,
    scene: Option<RenderScene>,
}

impl From<&PanelState> for SignalStateView {
    fn from(state: &PanelState) -> Self {
        Self {
            draft: state.into(),
            chrome: state.chrome.clone(),
            engine: state.engine.snapshot(),
            candidates: state.engine.candidates().to_vec(),
            selected: state
                .engine
                .selected_completion_text(true)
                .map(str::to_owned),
            prediction: state.engine.prediction_status(),
            error: state.engine.prediction_error().cloned(),
            scene: state.last_scene.clone(),
        }
    }
}

fn signal_history(state: &mut PanelState) {
    state.completion_history.clear();
    let adopted = state.completion_history.apply(
        "h",
        &suzaku_map::panel_support::NextTokenCompletion {
            label: "hel".into(),
            seed_before: "h".into(),
            seed_after: "hel".into(),
        },
    );
    assert_eq!(adopted.as_deref(), Some(state.chrome.seed_text.as_str()));
    state.chrome.caret_index = 1;
}

fn assert_candidate_mirror(state: &PanelState) {
    for (&index, label) in state
        .chrome
        .sentence_candidate_source_indices
        .iter()
        .zip(&state.chrome.sentence_candidates)
    {
        assert_eq!(
            label,
            &state.engine.candidates()[index].label,
            "signal changes must refresh the UI mirror, not just the engine"
        );
    }
}

struct GatedTranslation {
    entered: mpsc::Sender<()>,
    reply: Mutex<mpsc::Receiver<String>>,
}

impl TranslationProvider for GatedTranslation {
    fn translate(&self, request: &TranslationRequest) -> Result<String, TranslationError> {
        assert_eq!(request.text, "hel");
        self.entered.send(()).unwrap();
        Ok(self
            .reply
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap())
    }
}

struct FixedTranslation;
impl TranslationProvider for FixedTranslation {
    fn translate(&self, request: &TranslationRequest) -> Result<String, TranslationError> {
        assert_eq!(request.text, "hel");
        Ok("translated fixture".into())
    }
}

fn show_translation(state: &mut PanelState) {
    state.chrome.active_input_mode = InputMode::Translation;
    state.chrome.input_modes_expanded = true;
    state.chrome.blur_input();
}

fn translation_ready(state: &mut PanelState) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while state.chrome.translation.phase == TranslationPhase::Pending {
        state.poll_translation();
        assert!(
            Instant::now() < deadline,
            "the synthetic translation did not complete"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(state.chrome.translation.phase, TranslationPhase::Ready);
    assert_eq!(state.chrome.translation.text, "translated fixture");
}

fn signal_changed_case(state: &mut PanelState) {
    reset(state);
    state.reset_signal();
    state.engine.seed("");
    let (entered, entered_rx) = mpsc::channel();
    let (reply, reply_rx) = mpsc::channel();
    state
        .engine
        .configure_prediction(Some(Arc::new(GatedProvider {
            entered,
            reply: Mutex::new(reply_rx),
        })));
    state.engine.seed("hel");
    state.refresh_composition_candidates();
    signal_history(state);
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    reply.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !state.engine.poll_prediction() {
        assert!(
            Instant::now() < deadline,
            "the synthetic signal-control result did not arrive"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    state.refresh_composition_candidates();
    assert_eq!(state.engine.candidates()[2].text, "helipad");
    native_sync::present_test_frame(state);
    let target = state
        .last_scene
        .as_ref()
        .unwrap()
        .interactive_targets
        .iter()
        .find(|target| target.kind == InteractionKind::Candidate(2))
        .unwrap()
        .rect;
    state.cursor_position = Some((target[0] + target[2] * 0.5, target[1] + target[3] * 0.5));
    state.begin_primary_press(false);
    assert_eq!(
        state.interaction.pressed_interaction,
        Some(InteractionKind::Candidate(2))
    );
    let draft = SignalDraft::from(&*state);
    let server = AckServer::start();
    state.degrade_signal();
    assert!(state.engine.snapshot().degraded);
    assert_eq!(state.engine.candidates()[2].text, "help");
    assert_candidate_mirror(state);
    assert!(
        state.last_scene.is_none(),
        "the old model-labelled scene must be invalidated"
    );
    assert!(
        state.interaction.pressed_interaction.is_none(),
        "a replaced candidate must lose its old press"
    );
    assert_eq!(SignalDraft::from(&*state), draft);
    let before = state.engine.snapshot();
    assert!(state.try_numbered_candidate(2));
    state.commit_keyboard_candidate();
    assert!(
        server.requests().is_empty(),
        "signal invalidation must prevent stale Enter delivery"
    );
    assert_eq!(state.engine.snapshot(), before);
    assert_eq!(SignalDraft::from(&*state), draft);
    native_sync::present_test_frame(state);
    state.engine.select_candidate(1);
    native_sync::present_test_frame(state);
    let unchanged = SignalStateView::from(&*state);
    for _ in 0..3 {
        state.degrade_signal();
        assert_eq!(
            SignalStateView::from(&*state),
            unchanged,
            "repeating D must retain a presented explicit choice"
        );
    }
    assert!(state.try_numbered_candidate(2));
    assert_eq!(state.chrome.seed_text, "help");
    assert_eq!(state.engine.snapshot().seed_text, "help");
    assert!(state.engine.snapshot().committed_text.is_empty());
    drop(server);

    // The signal controls change prediction metadata, not the source/target of
    // a real translation request or the reversible adoption of this same draft.
    state.cancel_translation();
    reset(state);
    state.reset_signal();
    signal_history(state);
    show_translation(state);
    let (entered, entered_rx) = mpsc::channel();
    let (reply, reply_rx) = mpsc::channel();
    state.start_translation_fixture(Arc::new(GatedTranslation {
        entered,
        reply: Mutex::new(reply_rx),
    }));
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let draft = SignalDraft::from(&*state);
    assert_eq!(draft.translation.phase, TranslationPhase::Pending);
    state.degrade_signal();
    assert_eq!(SignalDraft::from(&*state), draft);
    state.reset_signal();
    assert_eq!(SignalDraft::from(&*state), draft);
    reply.send("translated fixture".into()).unwrap();
    translation_ready(state);
    let ready = SignalDraft::from(&*state);
    state.degrade_signal();
    assert_eq!(SignalDraft::from(&*state), ready);
    state.reset_signal();
    assert_eq!(SignalDraft::from(&*state), ready);
    state.cancel_translation();
}

struct SignalPrediction {
    entered: mpsc::Sender<PredictionCancellation>,
    reply: Mutex<mpsc::Receiver<()>>,
    calls: Arc<AtomicUsize>,
}

impl PredictionProvider for SignalPrediction {
    fn predict(
        &self,
        request: &PredictionRequest,
        cancellation: &PredictionCancellation,
    ) -> Result<PredictionResponse, PredictionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.send(cancellation.clone()).unwrap();
        self.reply
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        Ok(PredictionResponse::new(
            request,
            vec![PredictionCandidate {
                text: "helipad".into(),
                score_bias: 1.0,
                kind: None,
            }],
        ))
    }
}

fn signal_noop_and_native_case(state: &mut PanelState) {
    state.cancel_translation();
    reset(state);
    state.reset_signal();
    signal_history(state);
    show_translation(state);
    state.start_translation_fixture(Arc::new(FixedTranslation));
    translation_ready(state);
    state.engine.select_candidate(1);
    native_sync::present_test_frame(state);
    assert_eq!(state.engine.selected_completion_text(true), Some("hello"));
    let before = SignalStateView::from(&*state);
    for _ in 0..3 {
        state.reset_signal();
        assert_eq!(
            SignalStateView::from(&*state),
            before,
            "repeating R must not rebuild or cancel unrelated work"
        );
    }

    state.cancel_translation();
    reset(state);
    state.reset_signal();
    state.engine.seed("");
    let (entered, entered_rx) = mpsc::channel();
    let (reply, reply_rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    state
        .engine
        .configure_prediction_provider(Some(Arc::new(SignalPrediction {
            entered,
            reply: Mutex::new(reply_rx),
            calls: calls.clone(),
        })));
    state.engine.seed("hel");
    state.refresh_composition_candidates();
    signal_history(state);
    let cancellation = entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    show_translation(state);
    let (translated, translated_rx) = mpsc::channel();
    let (translate, translate_rx) = mpsc::channel();
    state.start_translation_fixture(Arc::new(GatedTranslation {
        entered: translated,
        reply: Mutex::new(translate_rx),
    }));
    translated_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    native_sync::present_test_frame(state);
    let before = SignalStateView::from(&*state);
    for _ in 0..3 {
        state.reset_signal();
        assert_eq!(SignalStateView::from(&*state), before);
        assert!(
            !cancellation.is_cancelled(),
            "an equal signal cancelled the original request"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            entered_rx.try_recv().is_err(),
            "an equal signal queued another prediction"
        );
    }
    reply.send(()).unwrap();
    translate.send("translated fixture".into()).unwrap();
    translation_ready(state);
    let deadline = Instant::now() + Duration::from_secs(2);
    while state.engine.prediction_pending() {
        state.engine.poll_prediction();
        assert!(
            Instant::now() < deadline,
            "the unchanged prediction was not accepted"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(state.engine.prediction_status(), PredictionStatus::Ready);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(!cancellation.is_cancelled());
    assert!(entered_rx.try_recv().is_err());
    state.engine.configure_prediction(None);

    for (showing, private) in [(true, false), (true, true), (false, true)] {
        state.native = Default::default();
        state.native.showing = showing;
        state.native.frame = Some(NativeComposition {
            host: "00000000-0000-0000-0000-000000000001".into(),
            context: 7,
            revision: 8,
            focused: true,
            private,
            cursor: None,
            language: "en".into(),
            seed: if private {
                String::new()
            } else {
                "external".into()
            },
            selected: 0,
            candidates: Vec::new(),
        });
        state.is_focused = false;
        state.chrome.blur_input();
        state.chrome.settings_open = false;
        let (sender, receiver) = mpsc::sync_channel(2);
        state.native.sender = Some(sender);
        let before = SignalStateView::from(&*state);
        let frame = state.native.frame.clone();
        let busy = state.native_candidates_busy();
        state.degrade_signal();
        assert_eq!(
            SignalStateView::from(&*state),
            before,
            "D changed a nonlocal/private context"
        );
        state.reset_signal();
        assert_eq!(
            SignalStateView::from(&*state),
            before,
            "R changed a nonlocal/private context"
        );
        assert_eq!(state.native.frame, frame);
        assert_eq!(state.native_candidates_busy(), busy);
        assert!(
            receiver.try_recv().is_err(),
            "signal-only control queued a Native Replace"
        );
    }
    // A focused private frame can be stale while the independent editor now
    // owns focus. Do not mechanically turn that leftover frame into a lockout.
    state.native.showing = false;
    state.is_focused = true;
    state.chrome.focus_input();
    let (sender, receiver) = mpsc::sync_channel(2);
    state.native.sender = Some(sender);
    let draft = SignalDraft::from(&*state);
    state.degrade_signal();
    assert!(state.engine.snapshot().degraded);
    assert_candidate_mirror(state);
    assert_eq!(SignalDraft::from(&*state), draft);
    state.reset_signal();
    assert!(!state.engine.snapshot().degraded);
    assert_candidate_mirror(state);
    assert_eq!(SignalDraft::from(&*state), draft);
    assert!(receiver.try_recv().is_err());
    state.native = Default::default();
    state.cancel_translation();
}
