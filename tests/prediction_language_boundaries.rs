//! Controlled cross-language races through one real prediction worker.
//! Synthetic text only: no HTTP, model, personal settings or persistent preferences.
use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource},
    },
    prediction::{
        PredictionCancellation, PredictionCandidate, PredictionError, PredictionKind,
        PredictionProvider, PredictionRequest, PredictionResponse,
    },
};

const WAIT: Duration = Duration::from_secs(2);
type Reply = Result<PredictionResponse, PredictionError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Case {
    language: &'static str,
    seed: &'static str,
    sentence: &'static str,
}

const CASES: [Case; 3] = [
    Case {
        language: "en",
        seed: "hel",
        sentence: "hello from the language boundary fixture.",
    },
    Case {
        language: "zh-Hans",
        seed: "nihao",
        sentence: "你好，语言边界测试完成。",
    },
    Case {
        language: "ja",
        seed: "nihongo",
        sentence: "日本語の言語境界テストです。",
    },
];

struct Entered {
    request: PredictionRequest,
    cancellation: PredictionCancellation,
    thread: ThreadId,
}

struct ControlledProvider {
    entered: mpsc::Sender<Entered>,
    replies: Mutex<mpsc::Receiver<Reply>>,
}

impl PredictionProvider for ControlledProvider {
    fn predict(&self, request: &PredictionRequest, cancellation: &PredictionCancellation) -> Reply {
        self.entered
            .send(Entered {
                request: request.clone(),
                cancellation: cancellation.clone(),
                thread: thread::current().id(),
            })
            .map_err(|_| PredictionError::Unavailable)?;
        // Deliberately ignore cancellation here. The real worker must prevent
        // an obsolete provider result/error from contaminating the new draft.
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
}

impl Controlled {
    fn new(case: Case) -> Self {
        let mut engine = XRTabletImeEngine::new(EngineConfig {
            default_language: case.language.into(),
            ..Default::default()
        });
        engine.set_preferences_enabled(false);
        engine.enable_ibus_candidate_mix();
        let (entered_tx, entered) = mpsc::channel();
        let (replies, replies_rx) = mpsc::channel();
        engine.configure_prediction_provider(Some(Arc::new(ControlledProvider {
            entered: entered_tx,
            replies: Mutex::new(replies_rx),
        })));
        assert_eq!(engine.prediction_status(), PredictionStatus::Idle);
        Self {
            engine,
            entered,
            replies,
        }
    }

    fn receive(&self, case: Case) -> Entered {
        let entered = self
            .entered
            .recv_timeout(WAIT)
            .expect("actual provider entry");
        assert_eq!(entered.request.input.language_id, case.language);
        assert_eq!(entered.request.input.seed_text, case.seed);
        assert!(entered.request.input.context_before_cursor.is_empty());
        entered.request.validate().unwrap();
        entered
    }

    fn switch_while_old_is_inflight(&mut self, old: &Entered, target: Case) {
        self.engine.set_language(target.language);
        self.engine.seed(target.seed);
        assert!(old.cancellation.is_cancelled());
        assert_eq!(self.engine.prediction_status(), PredictionStatus::Pending);
        assert_eq!(self.engine.prediction_error(), None);
        assert_eq!(self.engine.snapshot().active_language, target.language);
        assert_eq!(self.engine.snapshot().seed_text, target.seed);
        assert!(self.engine.snapshot().committed_text.is_empty());
    }

    fn settle_success(&mut self, fresh: &Entered, target: Case) {
        self.replies
            .send(Ok(PredictionResponse::new(
                &fresh.request,
                vec![PredictionCandidate {
                    text: target.sentence.into(),
                    score_bias: 0.6,
                    kind: Some(PredictionKind::Sentence),
                }],
            )))
            .unwrap();
        let deadline = Instant::now() + WAIT;
        while self.engine.prediction_pending() && Instant::now() < deadline {
            self.engine.poll_prediction();
            if self.engine.prediction_pending() {
                thread::sleep(Duration::from_millis(2));
            }
        }
        assert_eq!(self.engine.prediction_status(), PredictionStatus::Ready);
        assert_eq!(self.engine.prediction_error(), None);
        assert!(self.engine.candidates().iter().any(|candidate| {
            candidate.text == target.sentence
                && candidate.kind == CandidateKind::Sentence
                && candidate.source == CandidateSource::Model
        }));
        assert_eq!(self.engine.snapshot().active_language, target.language);
        assert_eq!(self.engine.snapshot().seed_text, target.seed);
        assert!(self.engine.snapshot().committed_text.is_empty());
        assert!(!fresh.cancellation.is_cancelled());
        assert!(
            self.entered.try_recv().is_err(),
            "unexpected additional provider request"
        );
        assert!(self.engine.candidates().len() <= 12);
    }
}

#[test]
fn obsolete_errors_cannot_contaminate_any_of_the_six_language_switch_directions() {
    let mut checked = 0;
    for source in CASES {
        for target in CASES {
            if source == target {
                continue;
            }
            for obsolete_error in [
                PredictionError::Timeout,
                PredictionError::MissingCredentials,
                PredictionError::HttpStatus(429),
            ] {
                let mut controlled = Controlled::new(source);
                controlled.engine.seed(source.seed);
                let old = controlled.receive(source);
                controlled.switch_while_old_is_inflight(&old, target);
                let before = controlled.engine.snapshot();
                let local = controlled.engine.candidates().to_vec();
                controlled.replies.send(Err(obsolete_error)).unwrap();

                // Receipt of the next call on the SAME worker proves the old
                // Err has passed the worker's cancellation/revision barrier.
                // Neither sleeping nor response IDs can substitute for this.
                let fresh = controlled.receive(target);
                assert_eq!(old.thread, fresh.thread);
                assert_ne!(old.request.request_id, fresh.request.request_id);
                assert!(old.cancellation.is_cancelled());
                assert!(!fresh.cancellation.is_cancelled());
                assert!(!controlled.engine.poll_prediction());
                assert_eq!(
                    controlled.engine.prediction_status(),
                    PredictionStatus::Pending
                );
                assert_eq!(controlled.engine.prediction_error(), None);
                assert_eq!(controlled.engine.snapshot(), before);
                assert_eq!(controlled.engine.candidates(), local);
                assert!(controlled.entered.try_recv().is_err());

                controlled.settle_success(&fresh, target);
                assert!(!controlled.engine.candidates().iter().any(|candidate| {
                    candidate.text == source.sentence && candidate.source == CandidateSource::Model
                }));
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 18);
}

#[test]
fn an_obsolete_reply_cannot_replace_the_worker_revision_with_its_response_id() {
    let source = CASES[0];
    let target = CASES[2];
    let mut controlled = Controlled::new(source);
    controlled.engine.seed(source.seed);
    let old = controlled.receive(source);
    controlled.switch_while_old_is_inflight(&old, target);
    let mut obsolete = PredictionResponse::new(
        &old.request,
        vec![PredictionCandidate {
            text: source.sentence.into(),
            score_bias: 0.6,
            kind: Some(PredictionKind::Sentence),
        }],
    );
    // A deliberately foreign wire ID must not be assigned to the local
    // mailbox revision, even when the stale provider ignores cancellation.
    obsolete.request_id = u64::MAX;
    controlled.replies.send(Ok(obsolete)).unwrap();
    let fresh = controlled.receive(target);
    assert_eq!(old.thread, fresh.thread);
    assert_ne!(fresh.request.request_id, u64::MAX);
    assert!(!controlled.engine.poll_prediction());
    assert_eq!(
        controlled.engine.prediction_status(),
        PredictionStatus::Pending
    );
    assert_eq!(controlled.engine.prediction_error(), None);
    controlled.settle_success(&fresh, target);
    assert!(
        !controlled
            .engine
            .candidates()
            .iter()
            .any(|candidate| candidate.text == source.sentence)
    );
}
