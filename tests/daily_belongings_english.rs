//! Everyday lost-and-found belongings fallback; fixed expectations precede the resource append.
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

type BelongingBranch = (&'static str, &'static str, &'static str);
const BELONGINGS: &[(&str, [BelongingBranch; 2])] = &[
    (
        "i found your",
        [
            ("wallet", "wall", "i found your wallet under the chair."),
            ("glasses", "glas", "i found your glasses next to the sofa."),
        ],
    ),
    (
        "is this your",
        [
            (
                "scarf",
                "scar",
                "is this your scarf on the back of the chair?",
            ),
            ("notebook", "noteb", "is this your notebook by the window?"),
        ],
    ),
    (
        "my bag might be",
        [
            (
                "upstairs",
                "upst",
                "my bag might be upstairs in the bedroom.",
            ),
            ("outside", "outs", "my bag might be outside near the door."),
        ],
    ),
    (
        "please look under the",
        [
            (
                "table",
                "tabl",
                "please look under the table before we leave.",
            ),
            (
                "sofa",
                "sof",
                "please look under the sofa for the missing keys.",
            ),
        ],
    ),
    (
        "i may have left my",
        [
            ("jacket", "jack", "i may have left my jacket in your car."),
            ("bottle", "bott", "i may have left my bottle at your house."),
        ],
    ),
    (
        "i'll keep it",
        [
            ("safe", "saf", "i'll keep it safe until you come back."),
            (
                "here",
                "her",
                "i'll keep it here so you can collect it later.",
            ),
        ],
    ),
    (
        "could you bring back my",
        [
            (
                "umbrella",
                "umbr",
                "could you bring back my umbrella when you visit?",
            ),
            (
                "notebook",
                "noteb",
                "could you bring back my notebook after work?",
            ),
        ],
    ),
    (
        "i can collect my",
        [
            ("bag", "ba", "i can collect my bag tomorrow morning."),
            ("coat", "coa", "i can collect my coat on the way home."),
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
fn belongings_resource_window_has_fixed_contexts_sentences_and_no_new_index_words() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(38)
        .expect("missing authored daily_belongings layer");
    assert_eq!(layer.id, "daily_belongings");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(BELONGINGS) {
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
        BELONGINGS
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, _, sentence)| *sentence))
            .collect::<Vec<_>>()
    );
    let previous_words = indexed_words(&en.word_layers()[..38]);
    assert_eq!(previous_words.len(), 6143);
    assert_eq!(indexed_words(&en.word_layers()[..39]), previous_words);
}

#[test]
fn belongings_each_explicit_branch_stays_on_first_page_and_commits_exactly_once() {
    for (context, branches) in BELONGINGS {
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
fn belongings_preserves_typed_case_curly_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "I’ll keep it HER",
            "I’ll keep it HERE",
            "I’ll keep it HERE so you can collect it later.",
        ),
        (
            "I found your GLAS",
            "I found your GLASSES",
            "I found your GLASSES next to the sofa.",
        ),
        (
            "My bag might be OUTS",
            "My bag might be OUTSIDE",
            "My bag might be OUTSIDE near the door.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in BELONGINGS {
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
fn belongings_word_adoption_space_continuation_and_rewind_preserve_the_draft() {
    // The second branch in each fixed context continues directly after Space.
    let tails = [
        ("nex", "next"),
        ("b", "by"),
        ("nea", "near"),
        ("fo", "for"),
        ("a", "at"),
        ("s", "so"),
        ("aft", "after"),
        ("o", "on"),
    ];
    for ((context, branches), (partial_tail, full_tail)) in BELONGINGS.iter().zip(tails) {
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
fn belongings_does_not_join_lines_or_reinterpret_protected_and_unknown_prefixes() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("i found{separator}your wall");
        let forbidden = format!("i found{separator}your wallet under the chair.");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for prefix in ["https://('", "file_name_('", "unknownprefix"] {
        for (context, branches) in BELONGINGS {
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
        "belongings-synthetic-unavailable"
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
fn belongings_local_choices_survive_unavailable_timeout_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, branches) in BELONGINGS {
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
