//! Signal controls use the shared engine, not desktop input or a live model.
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, ThreadId},
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        Candidate, EngineConfig, LanguagePlugin, PredictionStatus, SignalState, Snapshot,
        XRTabletImeEngine, candidate_mix::CandidateSource,
    },
    prediction::{
        PredictionCancellation, PredictionCandidate, PredictionError, PredictionKind,
        PredictionProvider, PredictionRequest, PredictionResponse,
    },
};

const WAIT: Duration = Duration::from_secs(2);
const CASES: [(&str, &str, &str); 3] = [
    ("en", "hel", "hello from the signal fixture."),
    ("zh-Hans", "nihao", "你好，信号测试完成。"),
    ("ja", "nihongo", "日本語のテストです。"),
];

fn weak_signal() -> SignalState {
    SignalState {
        pointer_precision: 0.2,
        gaze_stability: 0.2,
        host_intent_weight: 0.4,
        source_confidence: 0.4,
    }
}

#[derive(Debug, PartialEq)]
struct VisibleState {
    snapshot: Snapshot,
    pool: Vec<Candidate>,
    explicit: Option<String>,
    status: PredictionStatus,
    error: Option<PredictionError>,
    preferences: usize,
}

fn visible(engine: &XRTabletImeEngine) -> VisibleState {
    VisibleState {
        snapshot: engine.snapshot(),
        pool: engine.candidates().to_vec(),
        explicit: engine.selected_completion_text(true).map(str::to_owned),
        status: engine.prediction_status(),
        error: engine.prediction_error().cloned(),
        preferences: engine.preference_count(),
    }
}

fn engine(language: &str, mixed: bool) -> XRTabletImeEngine {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    engine.set_preferences_enabled(false);
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    engine
}

#[test]
fn bool_and_legacy_snapshot_apis_have_identical_changed_and_unchanged_effects() {
    for (language, seed, _) in CASES {
        for mixed in [false, true] {
            let (mut boolean, mut legacy) = (engine(language, mixed), engine(language, mixed));
            assert_eq!(visible(&boolean), visible(&legacy));
            for (signal, changed) in [
                (SignalState::default(), true),
                (SignalState::default(), false),
                (weak_signal(), true),
                (weak_signal(), false),
                (SignalState::default(), true),
            ] {
                boolean.seed(seed);
                legacy.seed(seed);
                let index = (boolean.candidates().len() - 1).min(1);
                boolean.select_candidate(index);
                legacy.select_candidate(index);
                assert_eq!(boolean.update_signal_if_changed(signal.clone()), changed);
                assert_eq!(legacy.update_signal(signal), boolean.snapshot());
                assert_eq!(visible(&boolean), visible(&legacy));
                assert!(boolean.snapshot().committed_text.is_empty());
                assert_eq!(boolean.snapshot().seed_text, seed);
            }
        }
    }
}

struct ReplacementPlugin(&'static str);

impl LanguagePlugin for ReplacementPlugin {
    fn id(&self) -> &str {
        self.0
    }

    fn expand_token(&self, token: &str, _: bool) -> Vec<String> {
        vec![token.into()]
    }

    fn build_candidates(&self, _: &[String], seed: &str, confidence: f32) -> Vec<Candidate> {
        vec![Candidate {
            text: format!("replacement: {seed}"),
            label: "replacement plugin".into(),
            score: confidence,
            ..Default::default()
        }]
    }
}

#[test]
fn initial_calibration_and_active_plugin_refresh_are_changes_but_inactive_registration_is_not() {
    let mut engine = engine("en", false);
    assert_eq!(engine.snapshot().confidence, 1.0);
    assert!(engine.update_signal_if_changed(SignalState::default()));
    assert!((engine.snapshot().confidence - 0.9).abs() < 0.000_001);
    assert!(!engine.update_signal_if_changed(SignalState::default()));
    engine.seed("hel");
    engine.select_candidate(1);
    let selected = visible(&engine);
    engine.register_language_plugin(ReplacementPlugin("inactive-fixture"));
    assert!(!engine.update_signal_if_changed(SignalState::default()));
    assert_eq!(visible(&engine), selected);
    engine.register_language_plugin(ReplacementPlugin("en"));
    assert_eq!(visible(&engine), selected, "registration remains lazy");
    assert!(engine.update_signal_if_changed(SignalState::default()));
    assert_eq!(engine.candidates()[0].text, "replacement: hel");
    assert_eq!(engine.selected_completion_text(true), None);
    engine.select_candidate(0);
    let refreshed = visible(&engine);
    assert!(!engine.update_signal_if_changed(SignalState::default()));
    assert_eq!(visible(&engine), refreshed);
}

type Reply = Result<PredictionResponse, PredictionError>;

struct Entered {
    request: PredictionRequest,
    cancellation: PredictionCancellation,
    thread: ThreadId,
}

struct Provider {
    entered: mpsc::Sender<Entered>,
    replies: Mutex<mpsc::Receiver<Reply>>,
    calls: Arc<AtomicUsize>,
}

impl PredictionProvider for Provider {
    fn predict(&self, request: &PredictionRequest, cancellation: &PredictionCancellation) -> Reply {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered
            .send(Entered {
                request: request.clone(),
                cancellation: cancellation.clone(),
                thread: thread::current().id(),
            })
            .map_err(|_| PredictionError::Unavailable)?;
        // Return even when cancelled: the actual worker must reject the old generation.
        self.replies
            .lock()
            .unwrap()
            .recv_timeout(WAIT)
            .unwrap_or(Err(PredictionError::Timeout))
    }
}

struct Controlled {
    engine: XRTabletImeEngine,
    entered: mpsc::Receiver<Entered>,
    replies: mpsc::Sender<Reply>,
    calls: Arc<AtomicUsize>,
}

impl Controlled {
    fn new(language: &str, mixed: bool) -> Self {
        let mut engine = engine(language, mixed);
        assert!(engine.update_signal_if_changed(SignalState::default()));
        let (entered_tx, entered) = mpsc::channel();
        let (replies, replies_rx) = mpsc::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        engine.configure_prediction_provider(Some(Arc::new(Provider {
            entered: entered_tx,
            replies: Mutex::new(replies_rx),
            calls: calls.clone(),
        })));
        Self {
            engine,
            entered,
            replies,
            calls,
        }
    }

    fn receive(&self, language: &str, seed: &str, count: usize) -> Entered {
        let entered = self
            .entered
            .recv_timeout(WAIT)
            .expect("actual worker entry");
        entered.request.validate().unwrap();
        assert_eq!(entered.request.input.language_id, language);
        assert_eq!(entered.request.input.seed_text, seed);
        assert!(entered.request.input.context_before_cursor.is_empty());
        assert!(!entered.cancellation.is_cancelled());
        assert_eq!(self.calls.load(Ordering::SeqCst), count);
        entered
    }

    fn unchanged(&mut self, signal: SignalState, entered: &Entered, count: usize) {
        let before = visible(&self.engine);
        let cancelled = entered.cancellation.is_cancelled();
        for _ in 0..2 {
            assert!(!self.engine.update_signal_if_changed(signal.clone()));
            assert_eq!(visible(&self.engine), before);
            assert_eq!(self.engine.update_signal(signal.clone()), before.snapshot);
            assert_eq!(visible(&self.engine), before);
            assert_eq!(entered.cancellation.is_cancelled(), cancelled);
            assert_eq!(self.calls.load(Ordering::SeqCst), count);
            assert!(matches!(
                self.entered.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
        }
    }

    fn settle(&mut self, status: PredictionStatus) {
        let deadline = Instant::now() + WAIT;
        while self.engine.prediction_pending() && Instant::now() < deadline {
            self.engine.poll_prediction();
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(self.engine.prediction_status(), status);
    }
}

fn response(request: &PredictionRequest, text: &str) -> PredictionResponse {
    let response = PredictionResponse::new(
        request,
        vec![PredictionCandidate {
            text: text.into(),
            score_bias: 1.0,
            kind: Some(PredictionKind::Sentence),
        }],
    );
    response.validate_for(request).unwrap();
    response
}

#[test]
fn unchanged_signals_preserve_pending_ready_failed_and_explicit_choices_in_three_languages() {
    for (language, seed, sentence) in CASES {
        for mixed in [false, true] {
            let mut control = Controlled::new(language, mixed);
            control.engine.seed(seed);
            let old = control.receive(language, seed, 1);
            control.unchanged(SignalState::default(), &old, 1);
            control.engine.select_candidate(1);
            assert!(old.cancellation.is_cancelled());
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert!(control.engine.selected_completion_text(true).is_some());
            control.unchanged(SignalState::default(), &old, 1);

            control.engine.seed(seed);
            control
                .replies
                .send(Ok(response(&old.request, sentence)))
                .unwrap();
            let fresh = control.receive(language, seed, 2);
            assert_eq!(fresh.thread, old.thread);
            assert_ne!(fresh.request.request_id, old.request.request_id);
            assert!(!control.engine.poll_prediction());
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Pending
            );
            control
                .replies
                .send(Ok(response(&fresh.request, sentence)))
                .unwrap();
            control.settle(PredictionStatus::Ready);
            assert_eq!(control.engine.prediction_error(), None);
            let model = control
                .engine
                .candidates()
                .iter()
                .position(|candidate| {
                    candidate.text == sentence && candidate.source == CandidateSource::Model
                })
                .expect("usable synthetic model sentence");
            // Cover the literal/conversion anchor, a local alternative and the model choice.
            for index in [0, 1, model] {
                control.engine.select_candidate(index);
                assert!(control.engine.selected_completion_text(true).is_some());
                control.unchanged(SignalState::default(), &fresh, 2);
            }

            control.engine.seed(seed);
            let failed = control.receive(language, seed, 3);
            assert_eq!(failed.thread, fresh.thread);
            control.replies.send(Err(PredictionError::Timeout)).unwrap();
            control.settle(PredictionStatus::Unavailable);
            assert_eq!(
                control.engine.prediction_error(),
                Some(&PredictionError::Timeout)
            );
            control.engine.select_candidate(1);
            control.unchanged(SignalState::default(), &failed, 3);
            assert_eq!(control.engine.snapshot().seed_text, seed);
            assert!(control.engine.snapshot().committed_text.is_empty());
            assert_eq!(control.engine.preference_count(), 0);
        }
    }
}

#[test]
fn changed_signal_fallback_and_restore_coalesce_to_one_fresh_generation_without_committing() {
    for (language, seed, sentence) in CASES {
        let mut control = Controlled::new(language, true);
        control.engine.seed(seed);
        let old = control.receive(language, seed, 1);
        let anchor = control.engine.candidates()[0].clone();
        for _ in 0..2 {
            assert!(control.engine.update_signal_if_changed(weak_signal()));
            assert!(old.cancellation.is_cancelled());
            assert!(control.engine.snapshot().degraded);
            assert!(control.engine.candidates().len() <= 3);
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            control.unchanged(weak_signal(), &old, 1);
            assert!(
                control
                    .engine
                    .update_signal_if_changed(SignalState::default())
            );
            assert!(!control.engine.snapshot().degraded);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Pending
            );
            control.unchanged(SignalState::default(), &old, 1);
        }
        let obsolete = format!("{sentence} obsolete");
        control
            .replies
            .send(Ok(response(&old.request, &obsolete)))
            .unwrap();
        let fresh = control.receive(language, seed, 2);
        assert_eq!(fresh.thread, old.thread);
        assert_ne!(fresh.request.request_id, old.request.request_id);
        assert_eq!(fresh.request.input, old.request.input);
        assert!(!control.engine.poll_prediction());
        assert_eq!(
            control.engine.prediction_status(),
            PredictionStatus::Pending
        );
        assert_eq!(control.engine.prediction_error(), None);
        let latest = format!("{sentence} latest");
        control
            .replies
            .send(Ok(response(&fresh.request, &latest)))
            .unwrap();
        control.settle(PredictionStatus::Ready);
        assert_eq!(control.engine.candidates()[0], anchor);
        assert!(
            !control
                .engine
                .candidates()
                .iter()
                .any(|candidate| candidate.text == obsolete)
        );
        assert!(control.engine.candidates().iter().any(|candidate| {
            candidate.text == latest && candidate.source == CandidateSource::Model
        }));
        assert_eq!(control.engine.snapshot().seed_text, seed);
        assert!(control.engine.snapshot().committed_text.is_empty());
        assert_eq!(control.engine.selected_completion_text(true), None);
        control.unchanged(SignalState::default(), &fresh, 2);
    }
}
