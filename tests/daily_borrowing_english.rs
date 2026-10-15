//! Everyday borrowing, returning and storage fallback; fixed expectations precede the resource append.
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

type BorrowingBranch = (&'static str, &'static str, &'static str);
const BORROWING: &[(&str, [BorrowingBranch; 2])] = &[
    (
        "may i use your",
        [
            (
                "notebook",
                "noteb",
                "may i use your notebook to take a few notes?",
            ),
            (
                "scissors",
                "scis",
                "may i use your scissors to open this package?",
            ),
        ],
    ),
    (
        "do you have a spare",
        [
            (
                "charger",
                "charg",
                "do you have a spare charger i could use?",
            ),
            (
                "umbrella",
                "umbr",
                "do you have a spare umbrella for the walk home?",
            ),
        ],
    ),
    (
        "i only need it for",
        [
            (
                "today",
                "toda",
                "i only need it for today and i'll return it tomorrow.",
            ),
            (
                "tonight",
                "toni",
                "i only need it for tonight so you can have it tomorrow.",
            ),
        ],
    ),
    (
        "please return it to the",
        [
            (
                "shelf",
                "shel",
                "please return it to the shelf when you finish.",
            ),
            (
                "drawer",
                "draw",
                "please return it to the drawer after using it.",
            ),
        ],
    ),
    (
        "you can leave it",
        [
            (
                "here",
                "her",
                "you can leave it here until tomorrow morning.",
            ),
            (
                "upstairs",
                "upst",
                "you can leave it upstairs next to the desk.",
            ),
        ],
    ),
    (
        "i'll put it back in the",
        [
            ("box", "bo", "i'll put it back in the box when i'm done."),
            (
                "cupboard",
                "cupb",
                "i'll put it back in the cupboard after dinner.",
            ),
        ],
    ),
    (
        "can you keep this",
        [
            ("safe", "saf", "can you keep this safe while i'm away?"),
            ("dry", "dr", "can you keep this dry until i get back?"),
        ],
    ),
    (
        "thanks for letting me use your",
        [
            (
                "book",
                "boo",
                "thanks for letting me use your book for the weekend.",
            ),
            (
                "jacket",
                "jack",
                "thanks for letting me use your jacket on the way home.",
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
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn commit_raw_once(ime: &mut XRTabletImeEngine, seed: &str) {
    let mut matching = ime.candidates().iter().filter(|c| c.text == seed);
    let candidate = matching.next().expect("missing exact raw draft");
    assert!(matching.next().is_none(), "duplicate raw draft");
    assert_eq!(candidate.source, CandidateSource::Local);
    // A known final word can already own the deduplicated raw text.
    assert!(matches!(
        candidate.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    let kind = candidate.kind;
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
fn borrowing_resource_window_has_fixed_contexts_sentences_and_no_new_index_words() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(39)
        .expect("missing authored daily_borrowing layer");
    assert_eq!(layer.id, "daily_borrowing");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(BORROWING) {
        assert_eq!(context, expected);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            branches.map(|(word, _, _)| word)
        );
    }
    assert_eq!(
        layer
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        BORROWING
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, _, sentence)| *sentence))
            .collect::<Vec<_>>()
    );
    let previous_words = indexed_words(&en.word_layers()[..39]);
    assert_eq!(previous_words.len(), 6143);
    assert_eq!(indexed_words(&en.word_layers()[..40]), previous_words);
}

#[test]
fn borrowing_each_explicit_branch_stays_on_first_page_and_commits_exactly_once() {
    for (context, branches) in BORROWING {
        for (next, partial, sentence) in branches {
            for seed in [
                format!("{context} "),
                format!("{context} {partial}"),
                format!("{context} {next}"),
            ] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                index(&ime, &format!("{context} {next}"), CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
            }
            let seed = format!("{context} {partial}");
            commit_once(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            commit_raw_once(&mut engine(&seed), &seed);
        }
    }
}

#[test]
fn borrowing_preserves_typed_case_curly_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "I’ll put it back in the CUPB",
            "I’ll put it back in the CUPBOARD",
            "I’ll put it back in the CUPBOARD after dinner.",
        ),
        (
            "May I use your SCIS",
            "May I use your SCISSORS",
            "May I use your SCISSORS to open this package?",
        ),
        (
            "i only need it for TONI",
            "i only need it for TONIGHT",
            "i only need it for TONIGHT so you can have it tomorrow.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in BORROWING {
        for (next, partial, sentence) in branches {
            let unpadded_word = format!("{context} {next}");
            let ending = sentence.strip_prefix(&unpadded_word).unwrap();
            for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let padded_context = context.replace(' ', pad);
                let seed = format!("{pad}{padded_context}{pad}{partial}");
                let word = format!("{pad}{padded_context}{pad}{next}");
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
fn borrowing_word_adoption_space_continuation_and_rewind_preserve_the_draft() {
    // The second branch in each fixed context continues directly after Space.
    let tails = [
        ("t", "to"),
        ("fo", "for"),
        ("s", "so"),
        ("aft", "after"),
        ("nex", "next"),
        ("aft", "after"),
        ("unt", "until"),
        ("o", "on"),
    ];
    for ((context, branches), (partial_tail, full_tail)) in BORROWING.iter().zip(tails) {
        let (next, partial, sentence) = branches[1];
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
            // Host-like adoption and reseeding; selection and Space do not commit.
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
fn borrowing_does_not_join_lines_or_reinterpret_protected_and_unknown_prefixes() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("may i use{separator}your noteb");
        let forbidden = format!("may i use{separator}your notebook to take a few notes?");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for prefix in ["https://('", "file_name_('", "unknownprefix"] {
        for (context, branches) in BORROWING {
            for (_, partial, sentence) in branches {
                let seed = format!("{prefix}{context} {partial}");
                let forbidden = format!("{prefix}{sentence}");
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
                commit_raw_once(&mut engine(&seed), &seed);
            }
        }
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "borrowing-synthetic-unavailable"
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
fn borrowing_local_choices_survive_unavailable_timeout_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, branches) in BORROWING {
            for (next, partial, sentence) in branches {
                let seed = format!("{context} {partial}");
                let mut ime = engine(&seed);
                ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
                ime.seed(&seed);
                draft(&ime, &seed);
                index(&ime, &format!("{context} {next}"), CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
                let local = ime.candidates().to_vec();
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
                assert_eq!(ime.candidates(), local);
                commit_once(&mut ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}
