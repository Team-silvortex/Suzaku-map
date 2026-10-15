//! File/photo sharing fallback; fixed expectations precede the resource append.
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

type SharingBranch = (&'static str, &'static str, &'static str);
const SHARING: &[(&str, [SharingBranch; 2])] = &[
    (
        "i've attached a",
        [
            (
                "photo",
                "pho",
                "i've attached a photo of the box for you to check.",
            ),
            (
                "document",
                "docu",
                "i've attached a document with the details you asked for.",
            ),
        ],
    ),
    (
        "could you open this",
        [
            (
                "attachment",
                "att",
                "could you open this attachment and check the details?",
            ),
            (
                "folder",
                "fold",
                "could you open this folder and find the photos?",
            ),
        ],
    ),
    (
        "please send the photos",
        [
            (
                "again",
                "aga",
                "please send the photos again when you have a moment.",
            ),
            (
                "tonight",
                "toni",
                "please send the photos tonight so i can save them.",
            ),
        ],
    ),
    (
        "the shared folder is",
        [
            (
                "empty",
                "emp",
                "the shared folder is empty, so i'll add the photos.",
            ),
            (
                "ready",
                "rea",
                "the shared folder is ready for the rest of the team.",
            ),
        ],
    ),
    (
        "i can't see the",
        [
            (
                "attachment",
                "att",
                "i can't see the attachment in your last message.",
            ),
            (
                "changes",
                "chan",
                "i can't see the changes in this document.",
            ),
        ],
    ),
    (
        "please check the file",
        [
            (
                "name",
                "nam",
                "please check the file name before sending it.",
            ),
            (
                "size",
                "siz",
                "please check the file size before you upload it.",
            ),
        ],
    ),
    (
        "could you add your",
        [
            (
                "notes",
                "not",
                "could you add your notes to the shared document?",
            ),
            (
                "name",
                "nam",
                "could you add your name to the file before sending it?",
            ),
        ],
    ),
    (
        "i'll send the final",
        [
            (
                "version",
                "vers",
                "i'll send the final version after checking the details.",
            ),
            (
                "photos",
                "phot",
                "i'll send the final photos once everyone is ready.",
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
fn sharing_resource_window_has_fixed_contexts_sentences_and_no_new_index_words() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(37)
        .expect("missing authored daily_sharing layer");
    assert_eq!(layer.id, "daily_sharing");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(SHARING) {
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
        SHARING
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, _, sentence)| *sentence))
            .collect::<Vec<_>>()
    );
    let previous_words = indexed_words(&en.word_layers()[..37]);
    assert_eq!(previous_words.len(), 6143);
    assert_eq!(indexed_words(&en.word_layers()[..38]), previous_words);
}

#[test]
fn sharing_each_explicit_branch_stays_on_first_page_and_commits_exactly_once() {
    for (context, branches) in SHARING {
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
fn sharing_preserves_typed_case_curly_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "I’ve attached a DOCU",
            "I’ve attached a DOCUMENT",
            "I’ve attached a DOCUMENT with the details you asked for.",
        ),
        (
            "i can’t see the att",
            "i can’t see the attachment",
            "i can’t see the attachment in your last message.",
        ),
        (
            "I'll send the final PHOT",
            "I'll send the final PHOTOS",
            "I'll send the final PHOTOS once everyone is ready.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_raw_once(&mut engine(seed), seed);
    }
    for (context, branches) in SHARING {
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
fn sharing_word_adoption_space_continuation_and_rewind_preserve_the_draft() {
    // The second branch in each fixed context continues directly after Space.
    let tails = [
        ("wit", "with"),
        ("an", "and"),
        ("s", "so"),
        ("fo", "for"),
        ("i", "in"),
        ("bef", "before"),
        ("t", "to"),
        ("onc", "once"),
    ];
    for ((context, branches), (partial_tail, full_tail)) in SHARING.iter().zip(tails) {
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
fn sharing_does_not_join_lines_or_reinterpret_protected_and_unknown_prefixes() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("i've attached{separator}a pho");
        let forbidden = format!("i've attached{separator}a photo of the box for you to check.");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for prefix in ["https://('", "file_name_('", "unknownprefix"] {
        for (context, branches) in SHARING {
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
        "sharing-synthetic-unavailable"
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
fn sharing_local_choices_survive_unavailable_timeout_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (context, branches) in SHARING {
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
