//! Authored delivery fallback, preserving the existing vocabulary and index order.
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
type DeliveryBranch = (&'static str, &'static str, &'static str);
const DELIVERY: &[(&str, [DeliveryBranch; 2])] = &[
    (
        "please leave the package",
        [
            (
                "inside",
                "ins",
                "please leave the package inside by the door.",
            ),
            (
                "outside",
                "out",
                "please leave the package outside if no one is home.",
            ),
        ],
    ),
    (
        "i'll collect the parcel",
        [
            ("today", "tod", "i'll collect the parcel today after work."),
            (
                "tomorrow",
                "tom",
                "i'll collect the parcel tomorrow morning.",
            ),
        ],
    ),
    (
        "the delivery should arrive",
        [
            (
                "today",
                "tod",
                "the delivery should arrive today before dinner.",
            ),
            (
                "tomorrow",
                "tom",
                "the delivery should arrive tomorrow afternoon.",
            ),
        ],
    ),
    (
        "could you deliver it",
        [
            ("later", "lat", "could you deliver it later this evening?"),
            ("tomorrow", "tom", "could you deliver it tomorrow instead?"),
        ],
    ),
    (
        "the box looks",
        [
            ("damaged", "dam", "the box looks damaged on one side."),
            ("wet", "we", "the box looks wet near the bottom."),
        ],
    ),
    (
        "please keep the package",
        [
            ("dry", "dr", "please keep the package dry until i get home."),
            (
                "safe",
                "sa",
                "please keep the package safe until i collect it.",
            ),
        ],
    ),
    (
        "i need to pack this",
        [
            (
                "carefully",
                "care",
                "i need to pack this carefully before sending it.",
            ),
            (
                "first",
                "fir",
                "i need to pack this first, then take it to the post office.",
            ),
        ],
    ),
    (
        "i'll send the package",
        [
            ("today", "tod", "i'll send the package today after lunch."),
            ("tomorrow", "tom", "i'll send the package tomorrow morning."),
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
fn delivery_contexts_and_explicit_partials_offer_each_word_and_sentence_locally() {
    for (context, branches) in DELIVERY {
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
fn delivery_word_sentence_and_literal_commit_once_and_undo() {
    for (context, branches) in DELIVERY {
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
fn delivery_preserve_case_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "Please leave the package INS",
            "Please leave the package INSIDE",
            "Please leave the package INSIDE by the door.",
        ),
        (
            "i’ll collect the parcel tod",
            "i’ll collect the parcel today",
            "i’ll collect the parcel today after work.",
        ),
        (
            "i’ll send the package tod",
            "i’ll send the package today",
            "i’ll send the package today after lunch.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{pad}please{pad}leave{pad}the{pad}package{pad}ins");
        let word = format!("{pad}please{pad}leave{pad}the{pad}package{pad}inside");
        let ime = engine(&seed);
        draft(&ime, &seed);
        index(&ime, &word, CandidateKind::Word);
        commit_once(
            &mut engine(&seed),
            &format!("{word} by the door."),
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn delivery_adoption_then_spaces_and_typing_keep_the_complete_draft() {
    let mut ime = engine("");
    let mut raw = String::new();
    for fragment in [
        "i'll", " ", "collect", " ", "the", " ", "parcel", " ", "tod",
    ] {
        raw.push_str(fragment);
        ime.seed(&raw);
        draft(&ime, &raw);
    }
    index(&ime, "i'll collect the parcel today", CandidateKind::Word);
    for prefix in [
        String::new(),
        "Earlier note. ".repeat(40),
        "前文😀。  ".repeat(100),
    ] {
        let seed = format!("{prefix}i'll collect the parcel tod");
        let word = format!("{prefix}i'll collect the parcel today");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        ime.select_candidate(index(&ime, &word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
        // Host-like adoption/reseed; native keyboard dispatch is a separate suite.
        for tail in ["  ", "  af", "  after"] {
            let continued = format!("{word}{tail}");
            ime.seed(&continued);
            draft(&ime, &continued);
            index(
                &ime,
                &format!("{word}  after work."),
                CandidateKind::Sentence,
            );
        }
        commit_once(
            &mut ime,
            &format!("{word}  after work."),
            CandidateKind::Sentence,
        );
        commit_raw_once(&mut engine(&seed), &seed);
    }
}

#[test]
fn delivery_do_not_complete_across_lines_or_inside_protected_tokens() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("please leave the{separator}package ins");
        let forbidden = format!("please leave the{separator}package inside by the door.");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
    for prefix in ["https://('", "file_name_('", "unknownprefix"] {
        let seed = format!("{prefix}please leave the package ins");
        let forbidden = format!("{prefix}please leave the package inside by the door.");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == forbidden));
        commit_raw_once(&mut engine(&seed), &seed);
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "delivery-synthetic-unavailable"
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
fn delivery_local_fallback_survives_missing_failed_timed_out_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in DELIVERY {
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
fn delivery_append_fixed_data_preserving_all_legacy_records_and_6143_positions() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..33];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0x601c3074eabdad8a
    );
    assert_eq!(hash_pairs(en.next_words().take(717)), 0x3852d5f348e70e47);
    assert_eq!(
        hash_pairs(en.sentences().take(1398).map(|s| (s, &[][..]))),
        0xe760c3b1301abb9a
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
        .get(33)
        .expect("missing authored daily_delivery layer");
    assert_eq!(layer.id, "daily_delivery");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(DELIVERY) {
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
        DELIVERY
            .iter()
            .flat_map(|(_, bs)| bs.iter().map(|(_, _, s)| *s))
            .collect::<Vec<_>>()
    );
    assert_eq!(word_positions(&en.word_layers()[..34]), before);
    let after = word_positions(en.word_layers());
    for (word, rank) in &before {
        assert_eq!(
            after.get(word),
            Some(rank),
            "old index position changed: {word}"
        );
    }
    assert_eq!(after.len(), 6216);
}
