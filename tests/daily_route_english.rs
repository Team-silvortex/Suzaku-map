//! Frozen meetup/travel-status utterances, not navigation, live ETAs or location sharing.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{WordLayer, builtin},
};

const CONTEXTS: [&str; 8] = [
    "i'm leaving in",
    "i'm taking a different",
    "i'm at the other",
    "let's meet on the",
    "i can't see you from",
    "please go ahead without",
    "i just got off the",
    "please stay where you",
];
const WORDS: [[&str; 2]; 8] = [
    ["two", "ten"],
    ["route", "bus"],
    ["entrance", "exit"],
    ["same", "other"],
    ["here", "inside"],
    ["me", "us"],
    ["bus", "train"],
    ["are", "can"],
];
const PARTIALS: [[&str; 2]; 8] = [
    ["tw", "te"],
    ["rout", "bu"],
    ["entranc", "exi"],
    ["sam", "othe"],
    ["her", "insid"],
    ["m", "u"],
    ["bu", "trai"],
    ["ar", "ca"],
];
const SENTENCES: [[&str; 2]; 8] = [
    [
        "i'm leaving in two minutes so i'll message you when i'm outside.",
        "i'm leaving in ten minutes and should be there before lunch.",
    ],
    [
        "i'm taking a different route because the road is closed.",
        "i'm taking a different bus so i may arrive a little later.",
    ],
    [
        "i'm at the other entrance on the opposite side of the building.",
        "i'm at the other exit so i'll walk around to meet you.",
    ],
    [
        "let's meet on the same side of the street as the cafe.",
        "let's meet on the other side of the station near the shop.",
    ],
    [
        "i can't see you from here so could you come to the gate?",
        "i can't see you from inside so i'll come outside to find you.",
    ],
    [
        "please go ahead without me if the bus is leaving.",
        "please go ahead without us and we'll meet you there.",
    ],
    [
        "i just got off the bus and will walk to the entrance.",
        "i just got off the train so i'll meet you by the exit.",
    ],
    [
        "please stay where you are and i'll come to you.",
        "please stay where you can see the main entrance.",
    ],
];
type Case = (&'static str, &'static str, &'static str, &'static str);
fn cases() -> impl Iterator<Item = Case> {
    CONTEXTS.iter().enumerate().flat_map(|(i, &context)| {
        (0..2).map(move |j| (context, WORDS[i][j], PARTIALS[i][j], SENTENCES[i][j]))
    })
}
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
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local && c.text.starts_with(seed))
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .take(PAGE_SIZE)
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()))
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let seed = ime.snapshot().seed_text;
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn raw_once(seed: &str) {
    let mut ime = engine(seed);
    draft(&ime, seed);
    let kind = ime.candidates()[0].kind;
    assert!(matches!(kind, CandidateKind::Literal | CandidateKind::Word));
    commit_once(&mut ime, seed, kind);
}
fn choices(seed: &str, word: &str, sentence: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    index(&ime, word, CandidateKind::Word);
    index(&ime, sentence, CandidateKind::Sentence);
}
fn indexed_words(layers: &[WordLayer]) -> HashSet<String> {
    layers
        .iter()
        .flat_map(|l| {
            l.words
                .iter()
                .map(String::as_str)
                .chain(
                    l.next_words
                        .iter()
                        .flat_map(|(_, ws)| ws.iter().map(String::as_str)),
                )
                .chain(
                    l.sentences
                        .iter()
                        .flat_map(|s| {
                            s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                        })
                        .filter(|s| !s.is_empty()),
                )
        })
        .map(|s| s.replace('’', "'").to_ascii_lowercase())
        .collect()
}

#[test]
fn route_layer_has_fixed_records_and_preserves_all_previous_index_forms() {
    let en = builtin("en").unwrap();
    assert_eq!(en.word_layers().len(), 53);
    assert_eq!(en.next_words().count(), 857);
    assert_eq!(en.sentences().count(), 1678);
    let layer = &en.word_layers()[43];
    assert_eq!(layer.id, "daily_route");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    for (i, (context, words)) in layer.next_words.iter().enumerate() {
        assert_eq!(context, CONTEXTS[i]);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            WORDS[i]
        );
        assert!(
            en.word_layers()[..43]
                .iter()
                .all(|l| l.next_words.iter().all(|(old, _)| old != context))
        );
    }
    assert_eq!(
        layer
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        SENTENCES.into_iter().flatten().collect::<Vec<_>>()
    );
    assert_eq!(
        en.word_layers()[..43]
            .iter()
            .map(|l| l.next_words.len())
            .sum::<usize>(),
        795
    );
    assert_eq!(
        en.word_layers()[..43]
            .iter()
            .map(|l| l.sentences.len())
            .sum::<usize>(),
        1554
    );
    let old = indexed_words(&en.word_layers()[..43]);
    assert_eq!(old.len(), 6143);
    assert_eq!(indexed_words(en.word_layers()).len(), 6179);
    assert_eq!(indexed_words(&en.word_layers()[..47]), old);
}

#[test]
fn route_every_fixed_branch_keeps_word_sentence_raw_and_explicit_once_only_commit() {
    for (context, next, partial, sentence) in cases() {
        assert!(next.starts_with(partial) && next != partial);
        let word = format!("{context} {next}");
        for seed in [
            format!("{context} "),
            format!("{context} {partial}"),
            word.clone(),
        ] {
            choices(&seed, &word, sentence);
            assert_eq!(
                engine(&seed).prediction_status(),
                PredictionStatus::Disabled
            );
            raw_once(&seed);
        }
        let seed = format!("{context} {partial}");
        commit_once(&mut engine(&seed), &word, CandidateKind::Word);
        commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
    }
}

#[test]
fn route_preserves_case_curly_apostrophes_and_six_horizontal_gaps() {
    for (seed, word, sentence) in [
        (
            "I’m taking a different ROUT",
            "I’m taking a different ROUTE",
            "I’m taking a different ROUTE because the road is closed.",
        ),
        (
            "Let’s meet on the OTHE",
            "Let’s meet on the OTHER",
            "Let’s meet on the OTHER side of the station near the shop.",
        ),
        (
            "I JUST GOT OFF THE TRAI",
            "I JUST GOT OFF THE TRAIN",
            "I JUST GOT OFF THE TRAIN so i'll meet you by the exit.",
        ),
    ] {
        choices(seed, word, sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        raw_once(seed);
    }
    for (context, next, partial, sentence) in cases() {
        let tail = sentence.strip_prefix(&format!("{context} {next}")).unwrap();
        for gap in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let context = context.replace(' ', gap);
            let seed = format!("{gap}{context}{gap}{partial}");
            let word = format!("{gap}{context}{gap}{next}");
            choices(&seed, &word, &format!("{word}{tail}"));
            commit_once(
                &mut engine(&seed),
                &format!("{word}{tail}"),
                CandidateKind::Sentence,
            );
            raw_once(&seed);
        }
    }
}

#[test]
fn route_word_adoption_space_progress_and_reseeding_never_submit_implicitly() {
    // One concrete authored branch/tail per context, fixed before data or runtime output.
    let tails = [
        (0, "minu", "minutes"),
        (0, "becau", "because"),
        (1, "s", "so"),
        (0, "sid", "side"),
        (1, "s", "so"),
        (0, "i", "if"),
        (1, "s", "so"),
        (0, "an", "and"),
    ];
    for (i, (branch, partial_tail, full_tail)) in tails.into_iter().enumerate() {
        let (context, next, partial, sentence) = (
            CONTEXTS[i],
            WORDS[i][branch],
            PARTIALS[i][branch],
            SENTENCES[i][branch],
        );
        let word = format!("{context} {next}");
        let tail = sentence.strip_prefix(&word).unwrap();
        assert!(tail.starts_with(&format!(" {full_tail} ")));
        for prefix in [
            String::new(),
            "Earlier note. ".repeat(40),
            "前文😀。  ".repeat(100),
        ] {
            let seed = format!("{prefix}{context} {partial}");
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert_eq!(ime.snapshot().seed_text, seed);
            assert!(ime.snapshot().committed_text.is_empty());
            // Host-like reseeding/adoption, not simulated physical keyboard input.
            ime.seed(&adopted);
            draft(&ime, &adopted);
            let expected = format!("{adopted}  {}", tail.trim_start());
            for progress in [
                "  ".to_owned(),
                format!("  {partial_tail}"),
                format!("  {full_tail}"),
                format!("  {partial_tail}"),
                "  ".to_owned(),
            ] {
                let continued = format!("{adopted}{progress}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(&ime, &expected, CandidateKind::Sentence);
            }
            commit_once(&mut ime, &expected, CandidateKind::Sentence);
            raw_once(&seed);
        }
    }
}

#[test]
fn route_keeps_old_controls_and_rejects_cross_line_or_protected_context() {
    // These are old controls, not new almost-there, lateness or meeting-location support.
    for (seed, word, sentence) in [
        ("i'm almost ther", "i'm almost there", "i'm almost there."),
        (
            "i may be a few minutes lat",
            "i may be a few minutes late",
            "i may be a few minutes late.",
        ),
        (
            "can you meet me outside the stat",
            "can you meet me outside the station",
            "can you meet me outside the station?",
        ),
    ] {
        choices(seed, word, sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        raw_once(seed);
    }
    for (context, _, partial, sentence) in cases() {
        let (left, right) = context.rsplit_once(' ').unwrap();
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
            assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
            raw_once(&seed);
        }
        for prefix in ["https://('", "file_name_('", "unknownprefix"] {
            let seed = format!("{prefix}{context} {partial}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|c| c.text == format!("{prefix}{sentence}"))
            );
            raw_once(&seed);
        }
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "route-synthetic-missing"
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
fn route_local_pools_survive_unavailable_timeout_and_empty_model_results_unchanged() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, next, partial, sentence) in cases() {
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
