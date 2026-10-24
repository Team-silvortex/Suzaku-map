//! Independently authored location/direction fallback, not live navigation.
//! Fixed expectations precede data changes; all prior words/text/ranks remain protected.
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

const WORDS: &[&str] = &["across", "corner", "corners", "street", "streets"];
const LOCATION: &[(&str, [(&str, &str); 2])] = &[
    (
        "which way is the",
        [
            ("entrance", "which way is the entrance from here?"),
            ("exit", "which way is the exit?"),
        ],
    ),
    (
        "am i going in the",
        [
            ("right", "am i going in the right direction?"),
            ("wrong", "am i going in the wrong direction?"),
        ],
    ),
    (
        "do i need to turn",
        [
            ("left", "do i need to turn left here?"),
            ("right", "do i need to turn right there?"),
        ],
    ),
    (
        "should i keep going",
        [
            ("straight", "should i keep going straight?"),
            ("forward", "should i keep going forward?"),
        ],
    ),
    (
        "can we walk across the",
        [
            ("street", "can we walk across the street here?"),
            ("bridge", "can we walk across the bridge?"),
        ],
    ),
    (
        "is it near the",
        [
            ("station", "is it near the station?"),
            ("bridge", "is it near the bridge?"),
        ],
    ),
    (
        "you can get there through the",
        [
            ("entrance", "you can get there through the entrance."),
            ("gate", "you can get there through the gate."),
        ],
    ),
    (
        "you'll find it next to the",
        [
            ("library", "you'll find it next to the library."),
            ("park", "you'll find it next to the park."),
        ],
    ),
    (
        "the entrance is behind the",
        [
            ("building", "the entrance is behind the building."),
            ("gate", "the entrance is behind the gate."),
        ],
    ),
    (
        "it's between the",
        [
            ("station", "it's between the station and the library."),
            ("park", "it's between the park and the school."),
        ],
    ),
    (
        "i'm standing in front of the",
        [
            ("cafe", "i'm standing in front of the cafe."),
            ("building", "i'm standing in front of the building."),
        ],
    ),
    (
        "i'll wait on the",
        [
            ("corner", "i'll wait on the corner by the cafe."),
            ("street", "i'll wait on the street outside the station."),
        ],
    ),
    (
        "can you meet me outside the",
        [
            ("station", "can you meet me outside the station?"),
            ("shop", "can you meet me outside the shop?"),
        ],
    ),
    (
        "let's wait near the",
        [
            ("entrance", "let's wait near the entrance."),
            ("bridge", "let's wait near the bridge."),
        ],
    ),
    (
        "i can't find the right",
        [
            ("building", "i can't find the right building."),
            ("entrance", "i can't find the right entrance."),
        ],
    ),
    (
        "could you tell me how to get",
        [
            ("there", "could you tell me how to get there?"),
            ("back", "could you tell me how to get back to the station?"),
        ],
    ),
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig::default());
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn check_draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.candidates()[0].text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
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

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
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
fn location_contexts_offer_both_word_and_sentence_branches_on_page_one() {
    assert_eq!(LOCATION.len(), 16);
    for (context, branches) in LOCATION {
        let seed = format!("{context} ");
        let ime = engine(&seed);
        check_draft(&ime, &seed);
        assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        for (next, sentence) in branches {
            let word = format!("{context} {next}");
            index(&ime, &word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
            let partial = format!("{context} {}", &next[..2]);
            let ime = engine(&partial);
            check_draft(&ime, &partial);
            index(&ime, &word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn location_common_nouns_prepositions_and_explicit_plurals_are_selectable() {
    for (seed, word) in [
        ("acros", "across"),
        ("corne", "corner"),
        ("corner", "corners"),
        ("stree", "street"),
        ("street", "streets"),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once_and_undo(&mut engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn every_location_word_branch_and_sentence_commits_exactly_once_and_undoes() {
    for (context, branches) in LOCATION {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..2]);
            commit_once_and_undo(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
}

#[test]
fn location_preserves_case_horizontal_spaces_apostrophes_and_literal_choices() {
    for (seed, word, sentence) in [
        (
            "Which way is the EN",
            "Which way is the ENTRANCE",
            "Which way is the ENTRANCE from here?",
        ),
        (
            "  can  we  walk  across  the  st",
            "  can  we  walk  across  the  street",
            "  can  we  walk  across  the  street here?",
        ),
        (
            "I’ll wait on the corne",
            "I’ll wait on the corner",
            "I’ll wait on the corner by the cafe.",
        ),
        (
            "you’ll find it next to the li",
            "you’ll find it next to the library",
            "you’ll find it next to the library.",
        ),
        (
            "it’s between the sta",
            "it’s between the station",
            "it’s between the station and the library.",
        ),
        (
            "I’m standing in front of the ca",
            "I’m standing in front of the cafe",
            "I’m standing in front of the cafe.",
        ),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), seed, ime.candidates()[0].kind);
    }
}

#[test]
fn long_location_drafts_and_adopted_words_preserve_all_text_and_followup_spacing() {
    for prefix in ["Earlier note. ".repeat(40), "前文😀。  ".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}i'll wait on the corne");
            let word = format!("{prefix}{separator}i'll wait on the corner");
            let sentence = format!("{word} by the cafe.");
            let mut ime = engine(&seed);
            assert!(seed.chars().count() > 256);
            check_draft(&ime, &seed);
            index(&ime, &word, CandidateKind::Word);
            commit_once_and_undo(&mut ime, &sentence, CandidateKind::Sentence);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            ime.seed(&seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            // Host-like reseeding only, not a physical key-event acceptance test.
            for tail in ["  ", "  by", "  by "] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                check_draft(&ime, &continued);
                index(
                    &ime,
                    &format!("{word}  by the cafe."),
                    CandidateKind::Sentence,
                );
            }
            commit_once_and_undo(
                &mut ime,
                &format!("{word}  by the cafe."),
                CandidateKind::Sentence,
            );
        }
    }
}

#[test]
fn location_does_not_join_line_boundaries_or_rewrite_protected_unknown_text() {
    for separator in ["\n", "\r", "\u{85}", "\u{2028}", "\u{2029}"] {
        let seed = format!("can we walk across{separator}the st");
        let mut ime = engine(&seed);
        check_draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text
            == format!("can we walk across{separator}the street here?")
            && c.kind == CandidateKind::Sentence));
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for seed in [
        "https://whichwayistheen",
        "file_name_corne",
        "unknownprefixam i going in the ri",
    ] {
        let mut ime = engine(seed);
        check_draft(&ime, seed);
        assert!(!ime.candidates().iter().any(
            |c| c.text == format!("{seed}ght direction?") && c.kind == CandidateKind::Sentence
        ));
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "location-synthetic-unavailable"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        match &self.0 {
            Some(error) => Err(error.clone()),
            None => Ok(Vec::new()),
        }
    }
}

#[test]
fn location_fallback_is_immediate_and_unchanged_with_missing_failed_or_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in LOCATION.iter().step_by(4) {
            let (next, sentence) = branches[0];
            let seed = format!("{context} {}", &next[..2]);
            let mut ime = engine(&seed);
            ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
            ime.seed(&seed);
            check_draft(&ime, &seed);
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
            commit_once_and_undo(&mut ime, sentence, CandidateKind::Sentence);
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
    let words = layers.iter().flat_map(|layer| {
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
fn location_keeps_all_twenty_one_earlier_layers_full_wording_and_every_old_rank() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..21];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0xd395c67ce83c6fca
    );
    assert_eq!(hash_pairs(en.next_words().take(593)), 0x7552cfc074205be6);
    assert_eq!(
        hash_pairs(en.sentences().take(1150).map(|text| (text, &[][..]))),
        0x5cfec0416a786e54
    );
    let previous_positions = word_positions(previous);
    let current = word_positions(en.word_layers());
    assert_eq!(previous_positions.len(), 6134);
    let mut hash = 0xcbf29ce484222325_u64;
    for (word, rank) in &previous_positions {
        assert_eq!(current[word], *rank, "changed old rank for {word}");
        for byte in word.bytes().chain([0]).chain((*rank as u64).to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(hash, 0xefe0b415ade6b8b5);
    assert_eq!(current.len(), 6213);
}

#[test]
fn location_appends_exactly_five_new_forms_sixteen_contexts_and_thirty_two_sentences() {
    let en = builtin("en").unwrap();
    let added = en
        .word_layers()
        .get(21)
        .expect("missing authored location layer");
    assert_eq!(added.id, "daily_location");
    assert_eq!(
        added.words.iter().map(String::as_str).collect::<Vec<_>>(),
        WORDS
    );
    assert_eq!(added.next_words.len(), 16);
    assert_eq!(added.sentences.len(), 32);
    for ((context, words), (expected_context, branches)) in added.next_words.iter().zip(LOCATION) {
        assert_eq!(context, expected_context);
        assert_eq!(
            words.iter().map(String::as_str).collect::<Vec<_>>(),
            branches.map(|(word, _)| word)
        );
    }
    assert_eq!(
        added
            .sentences
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        LOCATION
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, sentence)| *sentence))
            .collect::<Vec<_>>()
    );
    let before = word_positions(&en.word_layers()[..21]);
    let after = word_positions(&en.word_layers()[..22]);
    assert_eq!(after.len(), 6139);
    assert_eq!(
        after
            .keys()
            .filter(|word| !before.contains_key(*word))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        WORDS
    );
}
