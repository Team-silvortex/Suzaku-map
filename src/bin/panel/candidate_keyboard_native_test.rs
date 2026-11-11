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
                signal_noop_and_native_case(&mut state);
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
