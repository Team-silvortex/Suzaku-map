//! Authored laundry fallback, preserving the existing vocabulary and index order.
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

// Keep the authored branches and explicit partials independent of the resource.
type LaundryBranch = (&'static str, &'static str, &'static str);
const LAUNDRY: &[(&str, [LaundryBranch; 2])] = &[
    (
        "are these towels",
        [
            ("clean", "cle", "are these towels clean enough to use?"),
            ("dry", "dr", "are these towels dry enough to put away?"),
        ],
    ),
    (
        "this shirt is still",
        [
            (
                "wet",
                "we",
                "this shirt is still wet, so i'll wear another one.",
            ),
            ("dirty", "dir", "this shirt is still dirty after washing."),
        ],
    ),
    (
        "i'll bring the laundry",
        [
            (
                "inside",
                "ins",
                "i'll bring the laundry inside before the rain starts.",
            ),
            (
                "downstairs",
                "down",
                "i'll bring the laundry downstairs after breakfast.",
            ),
        ],
    ),
    (
        "please check your pockets",
        [
            (
                "first",
                "fir",
                "please check your pockets first, then put your jacket in the basket.",
            ),
            (
                "again",
                "ag",
                "please check your pockets again in case your keys are there.",
            ),
        ],
    ),
    (
        "i can't find my",
        [
            (
                "socks",
                "so",
                "i can't find my socks after doing the laundry.",
            ),
            ("gloves", "glo", "i can't find my gloves in the drawer."),
        ],
    ),
    (
        "these socks don't",
        [
            (
                "match",
                "mat",
                "these socks don't match, so i'll look for another pair.",
            ),
            ("fit", "fi", "these socks don't fit me anymore."),
        ],
    ),
    (
        "i need a clean",
        [
            ("shirt", "shi", "i need a clean shirt for tomorrow."),
            ("towel", "tow", "i need a clean towel for the bathroom."),
        ],
    ),
    (
        "i'll put the folded laundry",
        [
            (
                "away",
                "aw",
                "i'll put the folded laundry away after dinner.",
            ),
            (
                "here",
                "he",
                "i'll put the folded laundry here on your bed.",
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
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
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
    let candidate = matching.next().expect("missing exact raw draft candidate");
    assert!(matching.next().is_none(), "duplicate raw draft candidate");
    assert_eq!(candidate.source, CandidateSource::Local);
    // Existing whole words (for example `we`) share the raw candidate after
    // deduplication; its kind may be Word without changing the committed text.
    assert!(matches!(
        candidate.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    let kind = candidate.kind;
    commit_once(ime, seed, kind);
}

#[test]
fn laundry_contexts_and_explicit_partials_offer_each_word_and_sentence_locally() {
    for (context, branches) in LAUNDRY {
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
        }
    }
}

#[test]
fn laundry_word_sentence_and_literal_commit_once_and_undo() {
    for (context, branches) in LAUNDRY {
        for (next, partial, sentence) in branches {
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
fn laundry_preserve_case_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "Are these towels CLE",
            "Are these towels CLEAN",
            "Are these towels CLEAN enough to use?",
        ),
        (
            "i’ll bring the laundry ins",
            "i’ll bring the laundry inside",
            "i’ll bring the laundry inside before the rain starts.",
        ),
        (
            "i can’t find my glo",
            "i can’t find my gloves",
            "i can’t find my gloves in the drawer.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{pad}are{pad}these{pad}towels{pad}cle");
        let word = format!("{pad}are{pad}these{pad}towels{pad}clean");
        let ime = engine(&seed);
        draft(&ime, &seed);
        index(&ime, &word, CandidateKind::Word);
        commit_once(
            &mut engine(&seed),
            &format!("{word} enough to use?"),
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn laundry_adoption_then_spaces_and_typing_keep_the_complete_draft() {
    let mut ime = engine("");
    let mut raw = String::new();
    for fragment in ["i", " ", "need", " ", "a", " ", "clean", " ", "shi"] {
        raw.push_str(fragment);
        ime.seed(&raw);
        draft(&ime, &raw);
    }
    index(&ime, "i need a clean shirt", CandidateKind::Word);
    for prefix in [
        String::new(),
        "Earlier note. ".repeat(40),
        "前文😀。  ".repeat(100),
    ] {
        let seed = format!("{prefix}i need a clean shi");
        let word = format!("{prefix}i need a clean shirt");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        ime.select_candidate(index(&ime, &word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
        // Host-like adoption/reseed; native keyboard dispatch is a separate suite.
        for tail in ["  ", "  fo", "  for"] {
            let continued = format!("{word}{tail}");
            ime.seed(&continued);
            draft(&ime, &continued);
            index(
                &ime,
                &format!("{word}  for tomorrow."),
                CandidateKind::Sentence,
            );
        }
        commit_once(
            &mut ime,
            &format!("{word}  for tomorrow."),
            CandidateKind::Sentence,
        );
        commit_raw_once(&mut engine(&seed), &seed);
    }
}

#[test]
fn laundry_do_not_complete_across_lines_or_inside_protected_tokens() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("are these{separator}towels cle");
        let forbidden = format!("are these{separator}towels clean enough to use?");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for prefix in ["https://('", "file_name_('", "unknownprefix"] {
        let seed = format!("{prefix}are these towels cle");
        let forbidden = format!("{prefix}are these towels clean enough to use?");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "laundry-synthetic-unavailable"
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
fn laundry_local_fallback_survives_missing_failed_timed_out_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in LAUNDRY {
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
                    .flat_map(|(_, ws)| ws.iter().map(String::as_str)),
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
fn laundry_append_fixed_data_preserving_all_legacy_records_and_6143_positions() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..35];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0xa6069f6383be8d12
    );
    assert_eq!(hash_pairs(en.next_words().take(733)), 0x13a5e3f5f5cbcddd);
    assert_eq!(
        hash_pairs(en.sentences().take(1430).map(|s| (s, &[][..]))),
        0xd335c280ce594e81
    );
    assert_eq!(hash_pairs(en.phrase_endings()), 0x6d41391645ab94f6);
    let before = word_positions(previous);
    assert_eq!(before.len(), 6143);
    let mut rank_hash = 0xcbf29ce484222325_u64;
    for (word, rank) in &before {
        for byte in word.bytes().chain([0]).chain((*rank as u64).to_le_bytes()) {
            rank_hash = (rank_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(rank_hash, 0x18269557c5329a0e);
    let layer = en
        .word_layers()
        .get(35)
        .expect("missing authored daily_laundry layer");
    assert_eq!(layer.id, "daily_laundry");
    assert_eq!(LAUNDRY.len(), 8);
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(LAUNDRY) {
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
        LAUNDRY
            .iter()
            .flat_map(|(_, bs)| bs.iter().map(|(_, _, s)| *s))
            .collect::<Vec<_>>()
    );
    assert_eq!(word_positions(&en.word_layers()[..36]), before);
    let after = word_positions(en.word_layers());
    for (word, rank) in &before {
        assert_eq!(
            after.get(word),
            Some(rank),
            "old index position changed: {word}"
        );
    }
    assert_eq!(after.len(), 6213);
}
