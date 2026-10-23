//! Fixed authored shopping/price/payment fallback, not live store or price access.
//! Expectations precede the data append; every old token and index rank stays intact.
use std::{
    collections::{BTreeMap, HashSet},
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

const SHOPPING: &[(&str, [(&str, &str); 2])] = &[
    (
        "does this price include",
        [
            ("tax", "does this price include tax?"),
            ("delivery", "does this price include delivery?"),
        ],
    ),
    (
        "can i pay with my",
        [
            ("card", "can i pay with my card?"),
            ("phone", "can i pay with my phone?"),
        ],
    ),
    (
        "could i have the",
        [
            ("receipt", "could i have the receipt, please?"),
            ("bag", "could i have the bag, please?"),
        ],
    ),
    (
        "do you have this in",
        [
            ("blue", "do you have this in blue?"),
            ("black", "do you have this in black?"),
        ],
    ),
    (
        "could you check for a",
        [
            ("smaller", "could you check for a smaller size?"),
            ("larger", "could you check for a larger size?"),
        ],
    ),
    (
        "can i try this on",
        [
            ("here", "can i try this on here?"),
            ("first", "can i try this on first?"),
        ],
    ),
    (
        "could i exchange this for",
        [
            ("another", "could i exchange this for another one?"),
            ("different", "could i exchange this for different shoes?"),
        ],
    ),
    (
        "can i return this if",
        [
            ("it", "can i return this if it doesn't fit?"),
            ("the", "can i return this if the size is wrong?"),
        ],
    ),
    (
        "i'd like to buy the",
        [
            ("smaller", "i'd like to buy the smaller one."),
            ("larger", "i'd like to buy the larger one."),
        ],
    ),
    (
        "do you have any more",
        [
            ("black", "do you have any more black shoes?"),
            ("blue", "do you have any more blue shoes?"),
        ],
    ),
    (
        "is there a cheaper",
        [
            ("option", "is there a cheaper option?"),
            ("version", "is there a cheaper version?"),
        ],
    ),
    (
        "how much will this cost",
        [
            ("today", "how much will this cost today?"),
            ("with", "how much will this cost with delivery?"),
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

#[test]
fn shopping_word_and_sentence_branches_are_on_page_one_for_full_and_partial_contexts() {
    assert_eq!(SHOPPING.len(), 12);
    for (context, branches) in SHOPPING {
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            for seed in [format!("{context} "), format!("{context} {}", &next[..2])] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                index(&ime, &word, CandidateKind::Word);
                index(&ime, sentence, CandidateKind::Sentence);
            }
        }
    }
}

#[test]
fn every_shopping_branch_and_sentence_commits_once_and_undoes_once() {
    for (context, branches) in SHOPPING {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..2]);
            commit_once(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            // These two-letter tokens were already complete English words
            // before this batch; Literal is not a universal label for raw text.
            let raw_kind = if matches!(&next[..2], "he" | "an" | "it" | "to") {
                CandidateKind::Word
            } else {
                CandidateKind::Literal
            };
            commit_once(&mut engine(&seed), &seed, raw_kind);
        }
    }
}

#[test]
fn shopping_preserves_horizontal_spaces_case_apostrophes_and_literal_text() {
    for (seed, word, sentence, raw_kind) in [
        (
            "Can i pay with my CA",
            "Can i pay with my CARD",
            "Can i pay with my CARD?",
            CandidateKind::Literal,
        ),
        (
            "  does  this  price  include  ta",
            "  does  this  price  include  tax",
            "  does  this  price  include  tax?",
            CandidateKind::Literal,
        ),
        (
            "I’d like to buy the sm",
            "I’d like to buy the smaller",
            "I’d like to buy the smaller one.",
            CandidateKind::Literal,
        ),
        (
            "can i return this if it",
            "can i return this if it",
            "can i return this if it doesn't fit?",
            CandidateKind::Word,
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once(&mut engine(seed), seed, raw_kind);
    }
}

#[test]
fn long_shopping_drafts_and_adopted_words_keep_exact_prefixes_and_spacing() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀。  ".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}could you check for a sm");
            let word = format!("{prefix}{separator}could you check for a smaller");
            let sentence = format!("{word} size?");
            let mut ime = engine(&seed);
            assert!(seed.chars().count() > 256);
            draft(&ime, &seed);
            commit_once(&mut ime, &sentence, CandidateKind::Sentence);
            ime.seed(&seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            // Host-like adoption/reseeding only, not physical desktop acceptance.
            for tail in ["  ", "  si", "  size"] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(&ime, &format!("{word}  size?"), CandidateKind::Sentence);
            }
            commit_once(&mut ime, &format!("{word}  size?"), CandidateKind::Sentence);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn shopping_does_not_manufacture_authored_sentences_across_lines_or_protected_text() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("does this price{separator}include ta");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text
            == format!("does this price{separator}include tax?")
            && c.kind == CandidateKind::Sentence));
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
    for (seed, forbidden) in [
        (
            "https://('does this price include ta",
            "https://('does this price include tax?",
        ),
        (
            "file_name_('does this price include ta",
            "file_name_('does this price include tax?",
        ),
        (
            "unknownprefixdoes this price include ta",
            "unknownprefixdoes this price include tax?",
        ),
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == forbidden && c.kind == CandidateKind::Sentence)
        );
        commit_once(&mut ime, seed, CandidateKind::Literal);
    }
    commit_once(
        &mut engine("('does this price include ta"),
        "('does this price include tax?",
        CandidateKind::Sentence,
    );
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "shopping-synthetic-unavailable"
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
fn shopping_local_fallback_survives_missing_failed_timed_out_and_empty_providers() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in SHOPPING.iter().step_by(3) {
            let (next, sentence) = branches[0];
            let seed = format!("{context} {}", &next[..2]);
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

fn hash_pairs<'a>(pairs: impl Iterator<Item = (&'a str, &'a [String])>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for (context, values) in pairs {
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        hash = (hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    hash
}

fn word_positions(layers: &[WordLayer]) -> BTreeMap<String, usize> {
    let words = layers.iter().flat_map(|l| {
        l.words
            .iter()
            .map(String::as_str)
            .chain(
                l.next_words
                    .iter()
                    .flat_map(|(_, words)| words.iter().map(String::as_str)),
            )
            .chain(l.sentences.iter().flat_map(|s| {
                s.split(|c: char| !(c.is_ascii_alphabetic() || matches!(c, '\'' | '’')))
                    .filter(|w| !w.is_empty())
            }))
    });
    let mut positions = BTreeMap::new();
    for (rank, word) in words.enumerate() {
        positions
            .entry(word.replace('’', "'").to_ascii_lowercase())
            .or_insert(rank);
    }
    positions
}

#[test]
fn shopping_preserves_every_old_layer_context_sentence_and_index_rank() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..23];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0xa1b7b6731b5bf4c9
    );
    assert_eq!(hash_pairs(en.next_words().take(621)), 0xd33c62f35a0f3dae);
    assert_eq!(
        hash_pairs(en.sentences().take(1206).map(|t| (t, &[][..]))),
        0x091ee95ffed59d5c
    );
    let before = word_positions(previous);
    let current = word_positions(en.word_layers());
    assert_eq!(before.len(), 6139);
    for (word, rank) in before {
        assert_eq!(current[&word], rank, "changed old rank for {word}");
    }
    assert_eq!(current.len(), 6193);
}

#[test]
fn shopping_adds_twelve_fixed_contexts_twenty_four_sentences_without_index_growth() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(23)
        .expect("missing authored shopping layer");
    assert_eq!(layer.id, "daily_shopping");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 12);
    assert_eq!(layer.sentences.len(), 24);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(SHOPPING) {
        assert_eq!(context, expected);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            branches.map(|(word, _)| word)
        );
    }
    assert_eq!(
        layer
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        SHOPPING
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, s)| *s))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        word_positions(&en.word_layers()[..23]),
        word_positions(&en.word_layers()[..24])
    );
}
