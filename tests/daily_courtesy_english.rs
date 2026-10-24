//! Authored courtesy fallback with independent expectations and old-rank guards.
//! These are finite offline examples, not model-quality or desktop acceptance.
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

const COURTESY: &[(&str, [(&str, &str); 2])] = &[
    (
        "thanks for taking the",
        [
            ("time", "thanks for taking the time to help me."),
            ("trouble", "thanks for taking the trouble to explain this."),
        ],
    ),
    (
        "thank you for being so",
        [
            ("patient", "thank you for being so patient with me."),
            ("kind", "thank you for being so kind to us."),
        ],
    ),
    (
        "i really appreciate your",
        [
            ("help", "i really appreciate your help with this."),
            ("time", "i really appreciate your time today."),
        ],
    ),
    (
        "it was kind of you to",
        [
            ("help", "it was kind of you to help us."),
            ("ask", "it was kind of you to ask."),
        ],
    ),
    (
        "sorry to keep you",
        [
            ("waiting", "sorry to keep you waiting."),
            ("busy", "sorry to keep you busy so late."),
        ],
    ),
    (
        "i'm sorry i forgot to",
        [
            ("reply", "i'm sorry i forgot to reply."),
            ("call", "i'm sorry i forgot to call you."),
        ],
    ),
    (
        "i'm sorry about the",
        [
            ("delay", "i'm sorry about the delay."),
            ("change", "i'm sorry about the change of plans."),
        ],
    ),
    (
        "i really didn't mean to",
        [
            ("interrupt", "i really didn't mean to interrupt you."),
            ("upset", "i really didn't mean to upset you."),
        ],
    ),
    (
        "could i ask you to",
        [
            ("help", "could i ask you to help me with this?"),
            ("wait", "could i ask you to wait a moment?"),
        ],
    ),
    (
        "would it be okay to",
        [
            ("sit", "would it be okay to sit here?"),
            ("join", "would it be okay to join you?"),
        ],
    ),
    (
        "may i have a",
        [
            ("moment", "may i have a moment of your time?"),
            ("seat", "may i have a seat here?"),
        ],
    ),
    (
        "please let me know if you",
        [
            ("mind", "please let me know if you mind."),
            ("need", "please let me know if you need anything."),
        ],
    ),
    (
        "that's very",
        [
            ("kind", "that's very kind of you."),
            ("helpful", "that's very helpful, thank you."),
        ],
    ),
    (
        "please don't feel you have to",
        [
            ("reply", "please don't feel you have to reply right away."),
            ("help", "please don't feel you have to help if you're busy."),
        ],
    ),
    (
        "i'm glad i could",
        [
            ("help", "i'm glad i could help."),
            ("come", "i'm glad i could come along."),
        ],
    ),
    (
        "thank you, that",
        [
            ("helps", "thank you, that helps a lot."),
            ("means", "thank you, that means a lot to me."),
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
            .all(|c| { c.source == CandidateSource::Local && c.text.starts_with(seed) })
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
fn courtesy_contexts_offer_both_word_branches_and_sentences_on_page_one() {
    assert_eq!(COURTESY.len(), 16);
    for (context, branches) in COURTESY {
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
fn every_courtesy_word_and_sentence_commits_once_with_one_undo() {
    for (context, branches) in COURTESY {
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
fn courtesy_appends_exactly_sixteen_contexts_and_thirty_two_sentences() {
    let en = builtin("en").unwrap();
    let added = en
        .word_layers()
        .get(20)
        .expect("missing authored courtesy layer");
    assert_eq!(added.id, "daily_courtesy");
    assert!(added.words.is_empty());
    assert_eq!(added.next_words.len(), 16);
    assert_eq!(added.sentences.len(), 32);
    for ((context, words), (expected_context, branches)) in added.next_words.iter().zip(COURTESY) {
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
        COURTESY
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, sentence)| *sentence))
            .collect::<Vec<_>>()
    );
}

#[test]
fn courtesy_keeps_the_older_need_help_sentence_ahead_of_both_new_choices() {
    let seed = "please let me know if you ";
    let ime = engine(seed);
    check_draft(&ime, seed);
    let old = index(
        &ime,
        "please let me know if you need help.",
        CandidateKind::Sentence,
    );
    for next in ["mind", "need"] {
        index(&ime, &format!("{seed}{next}"), CandidateKind::Word);
    }
    for sentence in [
        "please let me know if you mind.",
        "please let me know if you need anything.",
    ] {
        assert!(old < index(&ime, sentence, CandidateKind::Sentence));
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    commit_once_and_undo(
        &mut engine(seed),
        "please let me know if you need help.",
        CandidateKind::Sentence,
    );
}

#[test]
fn courtesy_preserves_case_spacing_apostrophes_and_raw_choices() {
    for (seed, word, sentence) in [
        (
            "Thank you for being so PA",
            "Thank you for being so PATIENT",
            "Thank you for being so PATIENT with me.",
        ),
        (
            "  thanks  for  taking  the  ti",
            "  thanks  for  taking  the  time",
            "  thanks  for  taking  the  time to help me.",
        ),
        (
            "I’m sorry about the de",
            "I’m sorry about the delay",
            "I’m sorry about the delay.",
        ),
        (
            "Please don’t feel you have to re",
            "Please don’t feel you have to reply",
            "Please don’t feel you have to reply right away.",
        ),
        (
            "That’s very he",
            "That’s very helpful",
            "That’s very helpful, thank you.",
        ),
        (
            "i really didn’t mean to in",
            "i really didn’t mean to interrupt",
            "i really didn’t mean to interrupt you.",
        ),
    ] {
        let ime = engine(seed);
        check_draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        index(&ime, sentence, CandidateKind::Sentence);
        commit_once_and_undo(&mut engine(seed), sentence, CandidateKind::Sentence);
        let raw_kind = ime.candidates()[0].kind;
        commit_once_and_undo(&mut engine(seed), seed, raw_kind);
    }
}

#[test]
fn courtesy_long_mixed_drafts_preserve_the_entire_prefix_and_literal() {
    assert!(!suzaku_map::languages::english::is_known_english_word(
        "patie"
    ));
    for prefix in ["Earlier note. ".repeat(40), "前文😀".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}thank you for being so patie");
            assert!(seed.chars().count() > 256);
            let mut ime = engine(&seed);
            check_draft(&ime, &seed);
            index(
                &ime,
                &format!("{prefix}{separator}thank you for being so patient"),
                CandidateKind::Word,
            );
            commit_once_and_undo(
                &mut ime,
                &format!("{prefix}{separator}thank you for being so patient with me."),
                CandidateKind::Sentence,
            );
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn adopted_courtesy_words_keep_the_draft_open_for_partial_followup() {
    let prefix = "Earlier note. ".repeat(40);
    let seed = format!("{prefix}thank you for being so patie");
    let word = format!("{prefix}thank you for being so patient");
    let mut ime = engine(&seed);
    ime.select_candidate(index(&ime, &word, CandidateKind::Word));
    let adopted = ime.selected_completion_text(true).unwrap().to_owned();
    assert_eq!(adopted, word);
    assert!(ime.snapshot().committed_text.is_empty());
    // Engine-level reseeding mirrors a host draft replacement. Real key
    // adoption/Space/Backspace still needs the private native input gate.
    for suffix in ["  ", "  wi", "  with "] {
        let continued = format!("{adopted}{suffix}");
        ime.seed(&continued);
        check_draft(&ime, &continued);
        index(&ime, &format!("{word}  with me."), CandidateKind::Sentence);
    }
    commit_once_and_undo(
        &mut ime,
        &format!("{word}  with me."),
        CandidateKind::Sentence,
    );
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "courtesy-synthetic-unavailable"
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
fn courtesy_is_immediate_and_unchanged_when_models_fail_or_return_empty() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in COURTESY.iter().step_by(4) {
            let (next, sentence) = branches[0];
            let seed = format!("{context} {}", &next[..2]);
            let word = format!("{context} {next}");
            let mut ime = engine(&seed);
            ime.configure_prediction(Some(Arc::new(MissingProvider(error.clone()))));
            ime.seed(&seed);
            check_draft(&ime, &seed);
            index(&ime, &word, CandidateKind::Word);
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
fn courtesy_keeps_all_twenty_previous_layers_full_wording_and_every_word_rank() {
    // Captured from clean v0.8.5 before adding courtesy; never refreshed to
    // make a data regression pass. All older independent snapshots stay intact.
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..20];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0x166c05bbbdb3fdbf
    );
    assert_eq!(hash_pairs(en.next_words().take(577)), 0x9e2f47416160c3ea);
    assert_eq!(
        hash_pairs(en.sentences().take(1118).map(|text| (text, &[][..]))),
        0x7bc8783ccc96df9a
    );
    let previous_positions = word_positions(previous);
    let current = word_positions(en.word_layers());
    assert_eq!(previous_positions.len(), 6134);
    let mut hash = 0xcbf29ce484222325_u64;
    for (word, rank) in &previous_positions {
        assert_eq!(current[word], *rank, "changed rank for {word}");
        for byte in word.bytes().chain([0]).chain((*rank as u64).to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(hash, 0xefe0b415ade6b8b5);
    // Courtesy itself reuses only old indexed words. Later independent layers
    // may append new forms, but must retain every old rank and the same budget.
    assert_eq!(word_positions(&en.word_layers()[..21]), previous_positions);
    assert_eq!(current.len(), 6213);
}
