//! Fixed booking, arrival, queue and service-hours phrases, authored before the resource append.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{WordLayer, builtin},
};

type AppointmentBranch = (&'static str, &'static str, &'static str);
const APPOINTMENTS: &[(&str, [AppointmentBranch; 2])] = &[
    (
        "do i need to book",
        [
            ("ahead", "ahea", "do i need to book ahead before i come in?"),
            (
                "online",
                "onlin",
                "do i need to book online or can i call instead?",
            ),
        ],
    ),
    (
        "i have a booking for",
        [
            (
                "today",
                "toda",
                "i have a booking for today and would like to confirm the time.",
            ),
            (
                "tomorrow",
                "tomorr",
                "i have a booking for tomorrow and may arrive a little early.",
            ),
        ],
    ),
    (
        "could i move my appointment to",
        [
            (
                "tomorrow",
                "tomorr",
                "could i move my appointment to tomorrow afternoon?",
            ),
            (
                "friday",
                "frida",
                "could i move my appointment to friday at the same time?",
            ),
        ],
    ),
    (
        "i've arrived for my",
        [
            (
                "appointment",
                "appointmen",
                "i've arrived for my appointment and can wait here.",
            ),
            (
                "booking",
                "bookin",
                "i've arrived for my booking a little early.",
            ),
        ],
    ),
    (
        "is this the queue for",
        [
            (
                "tickets",
                "ticket",
                "is this the queue for tickets or should i go inside?",
            ),
            (
                "service",
                "servic",
                "is this the queue for service at the front desk?",
            ),
        ],
    ),
    (
        "how long is the wait",
        [
            (
                "today",
                "toda",
                "how long is the wait today without a booking?",
            ),
            (
                "usually",
                "usuall",
                "how long is the wait usually around this time?",
            ),
        ],
    ),
    (
        "what time do you",
        [
            ("open", "ope", "what time do you open tomorrow morning?"),
            ("close", "clos", "what time do you close on friday?"),
        ],
    ),
    (
        "can i come in",
        [
            ("today", "toda", "can i come in today without a booking?"),
            (
                "tomorrow",
                "tomorr",
                "can i come in tomorrow before you close?",
            ),
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.candidates().len() <= 12);
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.source == CandidateSource::Local && candidate.text.starts_with(seed)
    }));
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty());
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn commit_raw_once(ime: &mut XRTabletImeEngine, seed: &str) {
    let mut matching = ime
        .candidates()
        .iter()
        .filter(|candidate| candidate.text == seed);
    let raw = matching.next().expect("missing exact raw draft");
    assert!(matching.next().is_none(), "duplicate raw draft");
    assert_eq!(raw.source, CandidateSource::Local);
    // An already complete final word can own the deduplicated literal text.
    assert!(matches!(
        raw.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    let kind = raw.kind;
    commit_once(ime, seed, kind);
}

fn indexed_words(layers: &[WordLayer]) -> HashSet<String> {
    layers
        .iter()
        .flat_map(|layer| {
            layer
                .words
                .iter()
                .map(String::as_str)
                .chain(
                    layer
                        .next_words
                        .iter()
                        .flat_map(|(_, words)| words.iter().map(String::as_str)),
                )
                .chain(layer.sentences.iter().flat_map(|sentence| {
                    sentence
                        .split(|ch: char| !(ch.is_ascii_alphabetic() || matches!(ch, '\'' | '’')))
                        .filter(|word| !word.is_empty())
                }))
        })
        .map(|word| word.replace('’', "'").to_ascii_lowercase())
        .collect()
}

#[test]
fn appointments_layer_has_fixed_records_and_reuses_the_previous_index() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(41)
        .expect("missing authored daily_appointments layer");
    assert_eq!(layer.id, "daily_appointments");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(APPOINTMENTS) {
        assert_eq!(context, expected);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            branches.map(|(word, _, _)| word)
        );
        assert!(
            en.word_layers()[..41]
                .iter()
                .all(|old| old.next_words.iter().all(|(key, _)| key != context))
        );
    }
    assert_eq!(
        layer
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        APPOINTMENTS
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, _, sentence)| *sentence))
            .collect::<Vec<_>>()
    );
    let previous_words = indexed_words(&en.word_layers()[..41]);
    assert_eq!(previous_words.len(), 6143);
    assert_eq!(indexed_words(&en.word_layers()[..42]), previous_words);
}

#[test]
fn appointments_all_explicit_branches_keep_words_sentences_and_exact_once_only_commits() {
    for (context, branches) in APPOINTMENTS {
        for (next, partial, sentence) in branches {
            let word = format!("{context} {next}");
            for seed in [
                format!("{context} "),
                format!("{context} {partial}"),
                word.clone(),
            ] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                index(&ime, &word, CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
                commit_raw_once(&mut engine(&seed), &seed);
            }
            let seed = format!("{context} {partial}");
            commit_once(&mut engine(&seed), &word, CandidateKind::Word);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn appointments_preserves_case_curly_apostrophes_and_six_horizontal_gaps() {
    for (seed, word, sentence) in [
        (
            "I’ve arrived for my BOOKIN",
            "I’ve arrived for my BOOKING",
            "I’ve arrived for my BOOKING a little early.",
        ),
        (
            "Could I move my appointment to FRIDA",
            "Could I move my appointment to FRIDAY",
            "Could I move my appointment to FRIDAY at the same time?",
        ),
        (
            "What time do you OPE",
            "What time do you OPEN",
            "What time do you OPEN tomorrow morning?",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in APPOINTMENTS {
        for (next, partial, sentence) in branches {
            let ending = sentence.strip_prefix(&format!("{context} {next}")).unwrap();
            for gap in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let padded_context = context.replace(' ', gap);
                let seed = format!("{gap}{padded_context}{gap}{partial}");
                let word = format!("{gap}{padded_context}{gap}{next}");
                let ime = engine(&seed);
                draft(&ime, &seed);
                index(&ime, &word, CandidateKind::Word);
                commit_once(
                    &mut engine(&seed),
                    &format!("{word}{ending}"),
                    CandidateKind::Sentence,
                );
                commit_raw_once(&mut engine(&seed), &seed);
            }
        }
    }
}

#[test]
fn appointments_word_adoption_space_progress_and_reseeding_do_not_submit() {
    // Each selected branch has its own fixed next tail; none is inferred from output.
    let tails = [
        (1, "o", "or"),
        (1, "an", "and"),
        (1, "a", "at"),
        (0, "an", "and"),
        (1, "a", "at"),
        (1, "arou", "around"),
        (1, "o", "on"),
        (1, "bef", "before"),
    ];
    for ((context, branches), (branch_index, partial_tail, full_tail)) in
        APPOINTMENTS.iter().zip(tails)
    {
        let (next, partial, sentence) = branches[branch_index];
        let unprefixed_word = format!("{context} {next}");
        let ending = sentence.strip_prefix(&unprefixed_word).unwrap();
        assert!(ending.starts_with(&format!(" {full_tail} ")));
        for prefix in [
            String::new(),
            "Earlier note. ".repeat(40),
            "前文😀。  ".repeat(100),
        ] {
            let seed = format!("{prefix}{context} {partial}");
            let word = format!("{prefix}{unprefixed_word}");
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            // Host-like adoption/reseeding is an engine workflow, not physical key input.
            ime.seed(&word);
            draft(&ime, &word);
            let expected_sentence = format!("{word}  {}", ending.trim_start());
            for tail in [
                "  ".to_owned(),
                format!("  {partial_tail}"),
                format!("  {full_tail}"),
                format!("  {partial_tail}"),
                "  ".to_owned(),
            ] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(&ime, &expected_sentence, CandidateKind::Sentence);
            }
            commit_once(&mut ime, &expected_sentence, CandidateKind::Sentence);
            commit_raw_once(&mut engine(&seed), &seed);
        }
    }
}

#[test]
fn appointments_keeps_old_controls_and_respects_line_and_protected_token_boundaries() {
    // Existing authored workflows are controls, not newly claimed appointment support.
    for (seed, word, sentence) in [
        (
            "can i book an appointment onlin",
            "can i book an appointment online",
            "can i book an appointment online?",
        ),
        (
            "could you check the opening hou",
            "could you check the opening hours",
            "could you check the opening hours?",
        ),
        (
            "is there a waiting ar",
            "is there a waiting area",
            "is there a waiting area?",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in APPOINTMENTS {
        let (left, right) = context.rsplit_once(' ').unwrap();
        for (_, partial, sentence) in branches {
            for separator in [
                "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                let seed = format!("{left}{separator}{right} {partial}");
                let forbidden = format!(
                    "{left}{separator}{}",
                    sentence.strip_prefix(&format!("{left} ")).unwrap()
                );
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|candidate| candidate.text == forbidden)
                );
                commit_raw_once(&mut engine(&seed), &seed);
            }
            for prefix in ["https://('", "file_name_('", "unknownprefix"] {
                let seed = format!("{prefix}{context} {partial}");
                let forbidden = format!("{prefix}{sentence}");
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    !ime.candidates()
                        .iter()
                        .any(|candidate| candidate.text == forbidden)
                );
                commit_raw_once(&mut engine(&seed), &seed);
            }
        }
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "appointments-synthetic-unavailable"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}

#[test]
fn appointments_complete_local_pools_survive_unavailable_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, branches) in APPOINTMENTS {
            for (next, partial, sentence) in branches {
                let seed = format!("{context} {partial}");
                let mut ime = engine(&seed);
                let local = ime.candidates().to_vec();
                ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
                ime.seed(&seed);
                draft(&ime, &seed);
                index(&ime, &format!("{context} {next}"), CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
                assert_eq!(ime.candidates(), local);
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(
                    ime.prediction_error(),
                    Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                );
                draft(&ime, &seed);
                assert_eq!(ime.candidates(), local);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}
