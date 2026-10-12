//! Controlled provider-neutral races; no network, native keys or installed IME.
//! Fresh request receipt proves that the single worker has drained an older
//! response before a negative assertion. Provider return alone is not that proof.
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use suzaku_map::ime::candidate_mix::{CandidateKind, CandidateSource};
use suzaku_map::ime::{CommitOptions, EngineConfig, PredictionStatus, Snapshot, XRTabletImeEngine};
use suzaku_map::prediction::{
    MAX_CANDIDATES, MAX_PREDICTION_SEED_CHARS, PredictionCancellation, PredictionCandidate,
    PredictionError, PredictionKind, PredictionProvider, PredictionRequest, PredictionResponse,
};

type Reply = Result<PredictionResponse, PredictionError>;
type ObservedRequest = (PredictionRequest, PredictionCancellation);

struct ControlledProvider {
    requests: mpsc::Sender<ObservedRequest>,
    replies: Mutex<mpsc::Receiver<Reply>>,
}

impl PredictionProvider for ControlledProvider {
    fn predict(&self, request: &PredictionRequest, cancellation: &PredictionCancellation) -> Reply {
        self.requests
            .send((request.clone(), cancellation.clone()))
            .map_err(|_| PredictionError::Unavailable)?;
        // Deliberately return a valid response even after cancellation. The
        // worker/engine must reject it without relying on provider cooperation.
        self.replies
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or(Err(PredictionError::Timeout))
    }
}

struct Controlled {
    engine: XRTabletImeEngine,
    requests: mpsc::Receiver<ObservedRequest>,
    replies: mpsc::Sender<Reply>,
}

fn controlled(language: &str, mixed: bool, context: Option<&str>) -> Controlled {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    if mixed {
        engine.enable_ibus_candidate_mix();
    }
    if let Some(context) = context {
        // Create genuine engine history before installing the blocking
        // provider. Select exact literal text, not an inferred conversion.
        engine.seed(context);
        let index = engine
            .candidates()
            .iter()
            .position(|row| row.text == context && row.source == CandidateSource::Local)
            .expect("committed context must have an exact local choice");
        engine.select_candidate(index);
        let committed = engine.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(context));
        assert!(engine.snapshot().seed_text.is_empty());
    }
    let (requests_tx, requests) = mpsc::channel();
    let (replies, replies_rx) = mpsc::channel();
    engine.configure_prediction_provider(Some(Arc::new(ControlledProvider {
        requests: requests_tx,
        replies: Mutex::new(replies_rx),
    })));
    assert_eq!(engine.prediction_status(), PredictionStatus::Idle);
    Controlled {
        engine,
        requests,
        replies,
    }
}

fn long_seed(language: &str) -> String {
    let seed = match language {
        "en" => format!("{}hel", "Earlier café 😀.  ".repeat(12)),
        "zh-Hans" => format!("{}ni hao", "你".repeat(170)),
        _ => unreachable!(),
    };
    assert!((161..MAX_PREDICTION_SEED_CHARS).contains(&seed.chars().count()));
    assert!(seed.len() > seed.chars().count(), "use a multibyte prefix");
    seed
}

fn budget_seed(language: &str) -> String {
    let seed = match language {
        "en" => format!("note {}z hel", "界😀é".repeat(82)),
        "zh-Hans" => format!("{}ni hao", "你".repeat(250)),
        _ => unreachable!(),
    };
    assert_eq!(seed.chars().count(), MAX_PREDICTION_SEED_CHARS);
    assert!(seed.len() > MAX_PREDICTION_SEED_CHARS);
    seed
}

fn context(language: &str) -> &'static str {
    match language {
        "en" => "Earlier confirmed note.",
        "zh-Hans" => "前文已确认。",
        _ => unreachable!(),
    }
}

fn receive(control: &Controlled, seed: &str, context: &str) -> ObservedRequest {
    let observed = control
        .requests
        .recv_timeout(Duration::from_secs(2))
        .expect("controlled prediction did not enter the provider");
    observed.0.validate().unwrap();
    assert_eq!(observed.0.input.seed_text, seed);
    assert_eq!(observed.0.input.context_before_cursor, context);
    assert_eq!(observed.0.limits.max_candidates, MAX_CANDIDATES);
    assert_eq!(
        observed.0.input.language_id,
        control.engine.snapshot().active_language
    );
    assert!(!observed.1.is_cancelled());
    observed
}

fn response(request: &PredictionRequest, label: &str) -> (PredictionResponse, String) {
    let text = if request.input.language_id == "en" {
        // Complete the authored `hel` rather than leaving a known incomplete
        // word followed by a separator; this is a semantically usable reply.
        format!("{}lo {label}.", request.input.seed_text)
    } else {
        format!("{}，{label}。", request.input.normalized_phrase)
    };
    let reply = PredictionResponse::new(
        request,
        vec![PredictionCandidate {
            text: text.clone(),
            score_bias: 1.0,
            kind: Some(PredictionKind::Sentence),
        }],
    );
    reply.validate_for(request).unwrap();
    (reply, text)
}

fn release_old_and_receive_fresh(
    control: &mut Controlled,
    old: &PredictionRequest,
    seed: &str,
    context: &str,
) -> (PredictionRequest, String) {
    let before = control.engine.snapshot();
    let local = control.engine.candidates().to_vec();
    let (reply, obsolete) = response(old, "obsolete reply");
    control.replies.send(Ok(reply)).unwrap();
    // A single worker cannot call the fresh provider until it has processed
    // the old provider result. This is stronger than a provider-return marker.
    let (fresh, _) = receive(control, seed, context);
    assert_ne!(fresh.request_id, old.request_id);
    assert!(!control.engine.poll_prediction());
    assert_eq!(
        control.engine.prediction_status(),
        PredictionStatus::Pending
    );
    assert_eq!(control.engine.prediction_error(), None);
    assert_eq!(control.engine.snapshot(), before);
    assert_eq!(control.engine.candidates(), local);
    assert!(local.iter().all(|row| row.source == CandidateSource::Local));
    assert!(!local.iter().any(|row| row.text == obsolete));
    (fresh, obsolete)
}

fn finish_fresh(control: &mut Controlled, fresh: &PredictionRequest, obsolete: &str) -> String {
    let before = control.engine.snapshot();
    let first = control.engine.candidates()[0].clone();
    let (reply, expected) = response(fresh, "fresh usable reply");
    assert_ne!(expected, obsolete);
    control.replies.send(Ok(reply)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while control.engine.prediction_pending() && Instant::now() < deadline {
        control.engine.poll_prediction();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(control.engine.prediction_status(), PredictionStatus::Ready);
    assert_eq!(control.engine.prediction_error(), None);
    assert_eq!(control.engine.candidates()[0], first);
    assert_eq!(control.engine.snapshot().seed_text, before.seed_text);
    assert_eq!(
        control.engine.snapshot().committed_text,
        before.committed_text
    );
    assert_eq!(
        control.engine.snapshot().selected_index,
        before.selected_index
    );
    assert!(control.engine.candidates().iter().any(|row| {
        row.text == expected
            && row.kind == CandidateKind::Sentence
            && row.source == CandidateSource::Model
    }));
    assert!(
        !control
            .engine
            .candidates()
            .iter()
            .any(|row| row.text == obsolete)
    );
    assert!(
        control.requests.try_recv().is_err(),
        "no implicit extra request"
    );
    expected
}

fn expect_only_context_removed(before: &Snapshot, after: &Snapshot) {
    let mut expected = before.clone();
    assert!(!expected.committed_text.is_empty());
    expected.committed_text.clear();
    assert_eq!(after, &expected);
}

#[test]
fn long_edit_restore_aba_discards_an_old_valid_reply_before_a_fresh_one() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (old, cancelled) = receive(&control, &raw, "");
            control.engine.seed(format!("{raw}x"));
            assert!(cancelled.is_cancelled());
            control.engine.seed(&raw);
            assert_eq!(control.engine.snapshot().seed_text, raw);
            assert_eq!(control.engine.selected_completion_text(true), None);
            let (fresh, obsolete) = release_old_and_receive_fresh(&mut control, &old, &raw, "");
            assert_eq!(
                fresh.input, old.input,
                "the old reply is still text-compatible"
            );
            finish_fresh(&mut control, &fresh, &obsolete);
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn long_selection_cancels_synchronously_and_manual_edit_unlocks_a_fresh_prediction() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (old, cancelled) = receive(&control, &raw, "");
            let pool = control.engine.candidates().to_vec();
            let index = pool.iter().position(|row| row.text != raw).unwrap();
            control.engine.select_candidate(index);
            assert!(
                cancelled.is_cancelled(),
                "selection must signal cancellation immediately"
            );
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert_eq!(control.engine.candidates(), pool);
            assert_eq!(control.engine.snapshot().selected_index, index);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(pool[index].text.as_str())
            );
            assert_eq!(control.engine.snapshot().seed_text, raw);
            assert!(control.engine.snapshot().committed_text.is_empty());

            // This negative is synchronous cancellation/selection preservation,
            // not a claim that the blocked old callback has already drained.
            let selected = control.engine.snapshot();
            assert!(!control.engine.poll_prediction());
            assert_eq!(control.engine.snapshot(), selected);
            let edited = format!("{raw}x");
            control.engine.seed(&edited);
            assert_eq!(control.engine.selected_completion_text(true), None);
            assert_eq!(control.engine.snapshot().selected_index, 0);
            let (fresh, obsolete) = release_old_and_receive_fresh(&mut control, &old, &edited, "");
            finish_fresh(&mut control, &fresh, &obsolete);
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn multibyte_256_257_256_budget_aba_keeps_the_entire_draft_and_rejects_old_replies() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            let raw = budget_seed(language);
            control.engine.seed(&raw);
            let (old, cancelled) = receive(&control, &raw, "");
            let longer = format!("{raw}😀");
            assert_eq!(longer.chars().count(), MAX_PREDICTION_SEED_CHARS + 1);
            control.engine.seed(&longer);
            assert!(cancelled.is_cancelled());
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert_eq!(control.engine.snapshot().seed_text, longer);
            assert!(
                control
                    .engine
                    .candidates()
                    .iter()
                    .any(|row| row.text == longer)
            );
            assert!(
                control
                    .engine
                    .candidates()
                    .iter()
                    .all(|row| row.source == CandidateSource::Local)
            );
            assert!(control.engine.snapshot().committed_text.is_empty());

            control.engine.seed(&raw);
            let (fresh, obsolete) = release_old_and_receive_fresh(&mut control, &old, &raw, "");
            assert_eq!(
                fresh.input, old.input,
                "never crop a long request to its tail"
            );
            finish_fresh(&mut control, &fresh, &obsolete);
            assert_eq!(control.engine.snapshot().seed_text, raw);
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn cleared_context_same_long_draft_recreation_never_accepts_the_old_field_reply() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let committed = context(language);
            let mut control = controlled(language, mixed, Some(committed));
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (old, cancelled) = receive(&control, &raw, committed);
            control.engine.clear_session_context();
            assert!(cancelled.is_cancelled());
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert!(control.engine.snapshot().seed_text.is_empty());
            assert!(control.engine.snapshot().draft_text.is_empty());
            assert!(control.engine.snapshot().committed_text.is_empty());
            assert!(control.engine.candidates().is_empty());
            assert_eq!(control.engine.snapshot().selected_index, 0);
            assert_eq!(
                control.engine.undo(),
                None,
                "old field history must be gone"
            );

            control.engine.seed(&raw);
            let (fresh, obsolete) = release_old_and_receive_fresh(&mut control, &old, &raw, "");
            assert_eq!(fresh.input.seed_text, old.input.seed_text);
            assert_eq!(fresh.input.normalized_phrase, old.input.normalized_phrase);
            assert!(!old.input.context_before_cursor.is_empty());
            finish_fresh(&mut control, &fresh, &obsolete);
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn successful_context_undo_cancels_a_pending_long_prediction_without_rebuilding_the_draft() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let committed = context(language);
            let mut control = controlled(language, mixed, Some(committed));
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (old, cancelled) = receive(&control, &raw, committed);
            let before = control.engine.snapshot();
            let local = control.engine.candidates().to_vec();
            let after = control
                .engine
                .undo()
                .expect("literal context created real history");
            assert!(
                cancelled.is_cancelled(),
                "context undo must cancel the obsolete request immediately"
            );
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert_eq!(control.engine.prediction_error(), None);
            expect_only_context_removed(&before, &after);
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), local);
            assert_eq!(control.engine.selected_completion_text(true), None);

            // A deliberate subsequent edit, not undo itself, requests the same
            // whole draft using the now-empty committed context.
            control.engine.seed(&raw);
            let (fresh, obsolete) = release_old_and_receive_fresh(&mut control, &old, &raw, "");
            assert_eq!(fresh.input.seed_text, old.input.seed_text);
            assert_eq!(fresh.input.normalized_phrase, old.input.normalized_phrase);
            finish_fresh(&mut control, &fresh, &obsolete);
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn no_history_undo_is_a_strict_noop_and_preserves_a_pending_long_prediction() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (live, token) = receive(&control, &raw, "");
            let before = control.engine.snapshot();
            let local = control.engine.candidates().to_vec();
            assert_eq!(control.engine.undo(), None);
            assert!(!token.is_cancelled());
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Pending
            );
            assert_eq!(control.engine.prediction_error(), None);
            assert_eq!(control.engine.snapshot(), before);
            assert_eq!(control.engine.candidates(), local);
            // Positive acceptance proves undo neither cancelled nor restarted
            // the existing blocked request; channel silence alone cannot.
            finish_fresh(&mut control, &live, "no obsolete row in this workflow");
            assert!(!token.is_cancelled());
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn successful_context_undo_preserves_the_ready_model_pool_and_explicit_selection() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let committed = context(language);
            let mut control = controlled(language, mixed, Some(committed));
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (request, token) = receive(&control, &raw, committed);
            let expected = finish_fresh(&mut control, &request, "no obsolete row in this workflow");
            assert!(
                !token.is_cancelled(),
                "ready request was never cancelled early"
            );
            let index = control
                .engine
                .candidates()
                .iter()
                .position(|row| row.text == expected && row.source == CandidateSource::Model)
                .unwrap();
            control.engine.select_candidate(index);
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Ready);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(expected.as_str())
            );
            let before = control.engine.snapshot();
            let ready = control.engine.candidates().to_vec();
            let after = control
                .engine
                .undo()
                .expect("literal context created real history");
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert_eq!(control.engine.prediction_error(), None);
            expect_only_context_removed(&before, &after);
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), ready);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(expected.as_str())
            );
            assert!(!control.engine.poll_prediction());
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), ready);
            assert!(
                control.requests.try_recv().is_err(),
                "undo must not rebuild or restart prediction"
            );
            let committed = control.engine.commit(CommitOptions { force: true });
            assert!(committed.ok);
            assert_eq!(committed.text.as_deref(), Some(expected.as_str()));
            assert!(control.engine.snapshot().seed_text.is_empty());
            let duplicate = control.engine.commit(CommitOptions { force: true });
            assert!(!duplicate.ok);
            assert_eq!(duplicate.text, None);
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

// Appended error/diagnostic paths deliberately leave the original seven tests
// and their reply barriers unchanged. An obsolete error is a result too: it
// must not change a newer draft's status or diagnostic merely because it has
// no candidate text to compare.
fn drain_obsolete_error_and_receive_fresh(
    control: &mut Controlled,
    old: &PredictionRequest,
    seed: &str,
    context: &str,
) -> PredictionRequest {
    let before = control.engine.snapshot();
    let local = control.engine.candidates().to_vec();
    assert_eq!(
        control.engine.prediction_status(),
        PredictionStatus::Pending
    );
    assert_eq!(control.engine.prediction_error(), None);
    assert_eq!(control.engine.selected_completion_text(true), None);
    control
        .replies
        .send(Err(PredictionError::HttpStatus(429)))
        .unwrap();
    // The same single-worker receipt barrier applies to Err and Ok. The
    // fresh provider is still blocked, so no fresh result can mask an obsolete
    // failure that was incorrectly published while draining the old request.
    let (fresh, _) = receive(control, seed, context);
    assert_ne!(fresh.request_id, old.request_id);
    assert!(!control.engine.poll_prediction());
    assert_eq!(
        control.engine.prediction_status(),
        PredictionStatus::Pending
    );
    assert_eq!(control.engine.prediction_error(), None);
    assert_eq!(control.engine.snapshot(), before);
    assert_eq!(control.engine.candidates(), local);
    assert!(local.iter().all(|row| row.source == CandidateSource::Local));
    fresh
}

fn finish_active_error(control: &mut Controlled, request: &PredictionRequest) {
    let before = control.engine.snapshot();
    let local = control.engine.candidates().to_vec();
    assert_eq!(request.input.seed_text, before.seed_text);
    assert_eq!(
        control.engine.prediction_status(),
        PredictionStatus::Pending
    );
    assert_eq!(control.engine.prediction_error(), None);
    control
        .replies
        .send(Err(PredictionError::HttpStatus(404)))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while control.engine.prediction_pending() && Instant::now() < deadline {
        control.engine.poll_prediction();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        control.engine.prediction_status(),
        PredictionStatus::Unavailable
    );
    assert_eq!(
        control.engine.prediction_error(),
        Some(&PredictionError::HttpStatus(404))
    );
    assert_eq!(control.engine.snapshot(), before);
    assert_eq!(control.engine.candidates(), local);
    assert!(local.iter().all(|row| row.source == CandidateSource::Local));
    assert!(control.requests.try_recv().is_err());
}

fn select_exact_local_draft(control: &mut Controlled, seed: &str) {
    let local = control.engine.candidates().to_vec();
    let index = local
        .iter()
        .position(|row| row.text == seed && row.source == CandidateSource::Local)
        .expect("the full literal draft must remain an exact local choice");
    control.engine.select_candidate(index);
    assert_eq!(control.engine.snapshot().seed_text, seed);
    assert_eq!(control.engine.snapshot().selected_index, index);
    assert_eq!(control.engine.candidates(), local);
    assert_eq!(control.engine.selected_completion_text(true), Some(seed));
}

#[test]
fn obsolete_errors_do_not_change_newer_long_drafts_or_pending_diagnostics() {
    let mut checked = 0;
    for boundary in [
        "edit-restore",
        "budget-restore",
        "context-clear",
        "context-undo",
    ] {
        for language in ["en", "zh-Hans"] {
            for mixed in [false, true] {
                let has_context = boundary.starts_with("context-");
                let committed = if has_context { context(language) } else { "" };
                let mut control = controlled(language, mixed, has_context.then_some(committed));
                let raw = if boundary == "budget-restore" {
                    budget_seed(language)
                } else {
                    long_seed(language)
                };
                control.engine.seed(&raw);
                let (old, cancelled) = receive(&control, &raw, committed);
                let before = control.engine.snapshot();
                let local = control.engine.candidates().to_vec();
                match boundary {
                    "edit-restore" => {
                        let edited = format!("{raw}x");
                        control.engine.seed(&edited);
                        assert_eq!(control.engine.snapshot().seed_text, edited);
                        assert!(cancelled.is_cancelled());
                    }
                    "budget-restore" => {
                        let longer = format!("{raw}😀");
                        assert_eq!(longer.chars().count(), MAX_PREDICTION_SEED_CHARS + 1);
                        control.engine.seed(&longer);
                        assert!(cancelled.is_cancelled());
                        assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
                        assert_eq!(control.engine.prediction_error(), None);
                        assert_eq!(control.engine.snapshot().seed_text, longer);
                        assert!(
                            control
                                .engine
                                .candidates()
                                .iter()
                                .any(|row| row.text == longer)
                        );
                        assert!(
                            control
                                .engine
                                .candidates()
                                .iter()
                                .all(|row| row.source == CandidateSource::Local)
                        );
                        assert!(control.engine.snapshot().committed_text.is_empty());
                    }
                    "context-clear" => {
                        control.engine.clear_session_context();
                        assert!(cancelled.is_cancelled());
                        assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
                        assert_eq!(control.engine.prediction_error(), None);
                        assert!(control.engine.snapshot().seed_text.is_empty());
                        assert!(control.engine.snapshot().draft_text.is_empty());
                        assert!(control.engine.snapshot().committed_text.is_empty());
                        assert!(control.engine.candidates().is_empty());
                        assert_eq!(control.engine.snapshot().selected_index, 0);
                        let cleared = control.engine.snapshot();
                        assert_eq!(control.engine.undo(), None);
                        assert_eq!(control.engine.snapshot(), cleared);
                    }
                    "context-undo" => {
                        let after = control
                            .engine
                            .undo()
                            .expect("literal context created history");
                        assert!(cancelled.is_cancelled());
                        assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
                        assert_eq!(control.engine.prediction_error(), None);
                        expect_only_context_removed(&before, &after);
                        assert_eq!(control.engine.snapshot(), after);
                        assert_eq!(control.engine.candidates(), local);
                        assert_eq!(control.engine.selected_completion_text(true), None);
                    }
                    _ => unreachable!(),
                }
                // Recreate the byte-identical full draft. Neither its text nor
                // normalized conversion can identify the obsolete failure.
                control.engine.seed(&raw);
                assert!(cancelled.is_cancelled());
                let fresh = drain_obsolete_error_and_receive_fresh(&mut control, &old, &raw, "");
                assert_eq!(fresh.input.seed_text, old.input.seed_text);
                assert_eq!(fresh.input.normalized_phrase, old.input.normalized_phrase);
                if has_context {
                    assert!(!old.input.context_before_cursor.is_empty());
                    assert!(fresh.input.context_before_cursor.is_empty());
                } else {
                    assert_eq!(fresh.input, old.input);
                }
                finish_fresh(
                    &mut control,
                    &fresh,
                    "no obsolete candidate in this error workflow",
                );
                assert!(control.engine.snapshot().committed_text.is_empty());
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 16);
}

#[test]
fn successful_undo_clears_current_failure_but_preserves_the_long_draft_and_local_choice() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let committed = context(language);
            let mut control = controlled(language, mixed, Some(committed));
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (failed, token) = receive(&control, &raw, committed);
            finish_active_error(&mut control, &failed);
            assert!(
                !token.is_cancelled(),
                "the current failure was actually accepted"
            );
            select_exact_local_draft(&mut control, &raw);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Unavailable
            );
            assert_eq!(
                control.engine.prediction_error(),
                Some(&PredictionError::HttpStatus(404))
            );
            let before = control.engine.snapshot();
            let local = control.engine.candidates().to_vec();
            let after = control
                .engine
                .undo()
                .expect("literal context created history");
            expect_only_context_removed(&before, &after);
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), local);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(raw.as_str())
            );
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert_eq!(control.engine.prediction_error(), None);
            assert!(!control.engine.poll_prediction());
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), local);

            // Only the deliberate follow-up seed may unlock the choice and
            // request again. It carries the entire draft and empty context.
            control.engine.seed(&raw);
            assert_eq!(control.engine.selected_completion_text(true), None);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Pending
            );
            assert_eq!(control.engine.prediction_error(), None);
            let (fresh, _) = receive(&control, &raw, "");
            assert_ne!(fresh.request_id, failed.request_id);
            assert_eq!(fresh.input.seed_text, failed.input.seed_text);
            assert_eq!(
                fresh.input.normalized_phrase,
                failed.input.normalized_phrase
            );
            finish_fresh(
                &mut control,
                &fresh,
                "no obsolete candidate after current failure",
            );
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn no_history_undo_preserves_current_failure_and_explicit_local_choice_until_an_edit() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (failed, token) = receive(&control, &raw, "");
            finish_active_error(&mut control, &failed);
            assert!(
                !token.is_cancelled(),
                "the current failure was actually accepted"
            );
            select_exact_local_draft(&mut control, &raw);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Unavailable
            );
            assert_eq!(
                control.engine.prediction_error(),
                Some(&PredictionError::HttpStatus(404))
            );
            let before = control.engine.snapshot();
            let local = control.engine.candidates().to_vec();
            assert_eq!(control.engine.undo(), None);
            assert_eq!(control.engine.snapshot(), before);
            assert_eq!(control.engine.candidates(), local);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(raw.as_str())
            );
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Unavailable
            );
            assert_eq!(
                control.engine.prediction_error(),
                Some(&PredictionError::HttpStatus(404))
            );
            assert!(!control.engine.poll_prediction());
            assert_eq!(control.engine.snapshot(), before);
            assert_eq!(control.engine.candidates(), local);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Unavailable
            );
            assert_eq!(
                control.engine.prediction_error(),
                Some(&PredictionError::HttpStatus(404))
            );
            assert!(!token.is_cancelled());

            control.engine.seed(&raw);
            assert_eq!(control.engine.selected_completion_text(true), None);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Pending
            );
            assert_eq!(control.engine.prediction_error(), None);
            let (fresh, _) = receive(&control, &raw, "");
            assert_ne!(fresh.request_id, failed.request_id);
            assert_eq!(fresh.input, failed.input);
            finish_fresh(
                &mut control,
                &fresh,
                "no obsolete candidate after no-history undo",
            );
            assert!(control.engine.snapshot().committed_text.is_empty());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn provider_none_undo_stays_disabled_and_preserves_local_choice_commit_and_reset_boundaries() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let committed = context(language);
            let mut control = controlled(language, mixed, Some(committed));
            control.engine.configure_prediction_provider(None);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Disabled
            );
            assert_eq!(control.engine.prediction_error(), None);
            let raw = long_seed(language);
            control.engine.seed(&raw);
            select_exact_local_draft(&mut control, &raw);
            let before = control.engine.snapshot();
            let local = control.engine.candidates().to_vec();
            assert!(local.iter().all(|row| row.source == CandidateSource::Local));
            let after = control
                .engine
                .undo()
                .expect("literal context created history");
            expect_only_context_removed(&before, &after);
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), local);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(raw.as_str())
            );
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Disabled
            );
            assert_eq!(control.engine.prediction_error(), None);
            assert_eq!(control.engine.undo(), None);
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), local);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(raw.as_str())
            );
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Disabled
            );
            assert_eq!(control.engine.prediction_error(), None);
            assert!(!control.engine.poll_prediction());
            assert_eq!(control.engine.snapshot(), after);
            assert_eq!(control.engine.candidates(), local);
            assert!(control.requests.try_recv().is_err());

            let committed = control.engine.commit(CommitOptions { force: true });
            assert!(committed.ok);
            assert_eq!(committed.text.as_deref(), Some(raw.as_str()));
            let committed_frame = control.engine.snapshot();
            assert_eq!(committed_frame.committed_text, raw);
            assert!(committed_frame.seed_text.is_empty());
            assert!(committed_frame.draft_text.is_empty());
            assert!(control.engine.candidates().is_empty());
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Disabled
            );
            assert_eq!(control.engine.prediction_error(), None);
            let duplicate = control.engine.commit(CommitOptions { force: true });
            assert!(!duplicate.ok);
            assert_eq!(duplicate.text, None);
            assert_eq!(control.engine.snapshot(), committed_frame);

            // This is an engine context reset, not a simulated native Reset.
            // It must erase the real commit history as well as visible text.
            control.engine.clear_session_context();
            let cleared = control.engine.snapshot();
            assert!(cleared.seed_text.is_empty());
            assert!(cleared.draft_text.is_empty());
            assert!(cleared.committed_text.is_empty());
            assert_eq!(cleared.selected_index, 0);
            assert!(control.engine.candidates().is_empty());
            assert_eq!(control.engine.undo(), None);
            assert_eq!(control.engine.snapshot(), cleared);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Disabled
            );
            assert_eq!(control.engine.prediction_error(), None);
            assert!(control.requests.try_recv().is_err());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

fn repeat_current_controls(engine: &mut XRTabletImeEngine) {
    let before = engine.snapshot();
    let pool = engine.candidates().to_vec();
    let selected = engine.selected_completion_text(true).map(str::to_owned);
    let status = engine.prediction_status();
    let error = engine.prediction_error().cloned();
    let alias = match before.active_language.as_str() {
        "en" => "en_GB",
        "zh-Hans" => "ZH_cn",
        _ => unreachable!(),
    };
    for language in [before.active_language.as_str(), alias] {
        assert_eq!(engine.set_language(language), before);
        assert_eq!(engine.candidates(), pool);
        assert_eq!(engine.selected_completion_text(true), selected.as_deref());
        assert_eq!(engine.prediction_status(), status);
        assert_eq!(engine.prediction_error(), error.as_ref());
        assert_eq!(
            engine.update_signal(suzaku_map::ime::SignalState::default()),
            before
        );
        assert_eq!(engine.candidates(), pool);
        assert_eq!(engine.selected_completion_text(true), selected.as_deref());
        assert_eq!(engine.prediction_status(), status);
        assert_eq!(engine.prediction_error(), error.as_ref());
    }
}

fn finish_unchanged_request(control: &mut Controlled, request: &PredictionRequest) -> String {
    let before = control.engine.snapshot();
    let (reply, expected) = response(request, "usable unchanged request");
    control.replies.send(Ok(reply)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while control.engine.prediction_pending() && Instant::now() < deadline {
        control.engine.poll_prediction();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(control.engine.prediction_status(), PredictionStatus::Ready);
    assert_eq!(control.engine.prediction_error(), None);
    assert!(control.engine.candidates().iter().any(|row| {
        row.text == expected
            && row.kind == CandidateKind::Sentence
            && row.source == CandidateSource::Model
    }));
    assert_eq!(control.engine.snapshot().seed_text, before.seed_text);
    assert_eq!(
        control.engine.snapshot().committed_text,
        before.committed_text
    );
    assert!(control.requests.try_recv().is_err());
    expected
}

#[test]
fn repeated_controls_preserve_the_exact_pending_request_and_its_usable_reply() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let committed = context(language);
            let mut control = controlled(language, mixed, Some(committed));
            control
                .engine
                .update_signal(suzaku_map::ime::SignalState::default());
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (request, token) = receive(&control, &raw, committed);
            repeat_current_controls(&mut control.engine);
            assert!(
                !token.is_cancelled(),
                "an equal control canceled the active request"
            );
            finish_unchanged_request(&mut control, &request);
            assert!(!token.is_cancelled());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn repeated_controls_preserve_ready_model_choices_and_one_exact_commit() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            control
                .engine
                .update_signal(suzaku_map::ime::SignalState::default());
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (request, _) = receive(&control, &raw, "");
            let expected = finish_unchanged_request(&mut control, &request);
            let index = control
                .engine
                .candidates()
                .iter()
                .position(|row| row.text == expected && row.source == CandidateSource::Model)
                .unwrap();
            control.engine.select_candidate(index);
            repeat_current_controls(&mut control.engine);
            assert_eq!(
                control.engine.selected_completion_text(true),
                Some(expected.as_str())
            );
            let result = control.engine.commit(CommitOptions { force: true });
            assert!(result.ok);
            assert_eq!(result.text.as_deref(), Some(expected.as_str()));
            assert_eq!(control.engine.prediction_status(), PredictionStatus::Idle);
            assert!(!control.engine.commit(CommitOptions { force: true }).ok);
            assert_eq!(control.engine.snapshot().committed_text, expected);
            assert!(control.engine.undo().unwrap().committed_text.is_empty());
            assert!(control.engine.undo().is_none());
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn repeated_controls_keep_current_errors_until_an_actual_edit_retries() {
    let mut checked = 0;
    for language in ["en", "zh-Hans"] {
        for mixed in [false, true] {
            let mut control = controlled(language, mixed, None);
            control
                .engine
                .update_signal(suzaku_map::ime::SignalState::default());
            let raw = long_seed(language);
            control.engine.seed(&raw);
            let (failed, token) = receive(&control, &raw, "");
            finish_active_error(&mut control, &failed);
            select_exact_local_draft(&mut control, &raw);
            repeat_current_controls(&mut control.engine);
            assert_eq!(
                control.engine.prediction_error(),
                Some(&PredictionError::HttpStatus(404))
            );
            assert!(!token.is_cancelled());
            assert!(control.requests.try_recv().is_err());

            control.engine.seed(&raw);
            assert_eq!(control.engine.selected_completion_text(true), None);
            assert_eq!(control.engine.prediction_error(), None);
            assert_eq!(
                control.engine.prediction_status(),
                PredictionStatus::Pending
            );
            let (fresh, _) = receive(&control, &raw, "");
            assert_ne!(fresh.request_id, failed.request_id);
            finish_unchanged_request(&mut control, &fresh);
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}
