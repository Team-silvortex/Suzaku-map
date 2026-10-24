//! Authored washing/tidying fallback; fixed expectations precede the data append.
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

const CARE: &[(&str, [(&str, &str); 2])] = &[
    (
        "i need to wash my",
        [
            ("face", "i need to wash my face before going out."),
            ("hands", "i need to wash my hands before we leave."),
        ],
    ),
    (
        "i'll dry my",
        [
            ("hair", "i'll dry my hair with a towel before going out."),
            ("face", "i'll dry my face with a towel."),
        ],
    ),
    (
        "please leave the towel",
        [
            ("here", "please leave the towel here by the sink."),
            ("there", "please leave the towel there until it's dry."),
        ],
    ),
    (
        "could you pass me a",
        [
            ("towel", "could you pass me a towel?"),
            ("bag", "could you pass me a bag for the wet shoes?"),
        ],
    ),
    (
        "let me tidy the",
        [
            ("room", "let me tidy the room first."),
            ("table", "let me tidy the table before we leave."),
        ],
    ),
    (
        "please hang it",
        [
            ("up", "please hang it up when you're done."),
            ("here", "please hang it here to dry."),
        ],
    ),
    (
        "i'll check the",
        [
            ("door", "i'll check the door before we leave."),
            ("lights", "i'll check the lights before going out."),
        ],
    ),
    (
        "let me fold this",
        [
            ("shirt", "let me fold this shirt before going out."),
            ("towel", "let me fold this towel and put it away."),
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
    commit_index_once(ime, index(ime, text, kind), text);
}
fn second_page_index(ime: &XRTabletImeEngine, text: &str) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == CandidateKind::Sentence)
        .unwrap_or_else(|| {
            panic!(
                "missing later-page sentence {text:?}: {:?}",
                ime.candidates()
            )
        });
    assert!((PAGE_SIZE..2 * PAGE_SIZE).contains(&index));
    index
}
fn commit_index_once(ime: &mut XRTabletImeEngine, index: usize, text: &str) {
    ime.select_candidate(index);
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
fn care_specific_partials_keep_both_branches_and_broad_contexts_preserve_old_choices() {
    for (context, branches) in CARE {
        for (next, sentence) in branches {
            for seed in [format!("{context} "), format!("{context} {}", &next[..2])] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
                index(&ime, &format!("{context} {next}"), CandidateKind::Word);
                if *context == "i'll check the" && *next == "lights" && seed.ends_with(' ') {
                    // The broader old latest-version context keeps its sentence slot.
                    // Only this full generic context needs the second page.
                    second_page_index(&ime, sentence);
                } else {
                    index(&ime, sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}
#[test]
fn care_broad_check_context_keeps_old_latest_and_selects_the_second_page_once() {
    let seed = "i'll check the ";
    let mut ime = engine(seed);
    draft(&ime, seed);
    index(&ime, "i'll check the latest", CandidateKind::Word);
    index(
        &ime,
        "i'll check the latest version.",
        CandidateKind::Sentence,
    );
    index(
        &ime,
        "i'll check the door before we leave.",
        CandidateKind::Sentence,
    );
    let text = "i'll check the lights before going out.";
    let index = second_page_index(&ime, text);
    commit_index_once(&mut ime, index, text);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    commit_once(
        &mut engine("i'll check the li"),
        text,
        CandidateKind::Sentence,
    );
}
#[test]
fn care_each_branch_and_raw_draft_commits_and_undoes_exactly_once() {
    for (context, branches) in CARE {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..2]);
            commit_once(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            let mut raw = engine(&seed);
            let kind = raw.candidates()[0].kind;
            commit_once(&mut raw, &seed, kind);
        }
    }
}
#[test]
fn care_preserves_case_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "I need to wash my HA",
            "I need to wash my HANDS",
            "I need to wash my HANDS before we leave.",
        ),
        (
            "i’ll dry my ha",
            "i’ll dry my hair",
            "i’ll dry my hair with a towel before going out.",
        ),
        (
            "let me fold this to",
            "let me fold this towel",
            "let me fold this towel and put it away.",
        ),
    ] {
        draft(&engine(seed), seed);
        index(&engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{pad}i{pad}need{pad}to{pad}wash{pad}my{pad}ha");
        let word = format!("{pad}i{pad}need{pad}to{pad}wash{pad}my{pad}hands");
        draft(&engine(&seed), &seed);
        index(&engine(&seed), &word, CandidateKind::Word);
        commit_once(
            &mut engine(&seed),
            &format!("{word} before we leave."),
            CandidateKind::Sentence,
        );
    }
}
#[test]
fn care_spaces_and_adoption_preserve_long_unicode_drafts_without_implicit_commit() {
    let mut raw = String::new();
    let mut ime = engine("");
    for fragment in [
        "i", " ", "need", " ", "to", " ", "wash", " ", "my", " ", "ha",
    ] {
        raw.push_str(fragment);
        ime.seed(&raw);
        draft(&ime, &raw);
    }
    index(&ime, "i need to wash my hands", CandidateKind::Word);
    index(
        &ime,
        "i need to wash my hands before we leave.",
        CandidateKind::Sentence,
    );
    for prefix in ["Earlier note. ".repeat(40), "前文😀。  ".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}i need to wash my ha");
            let word = format!("{prefix}{separator}i need to wash my hands");
            let mut ime = engine(&seed);
            assert!(seed.chars().count() > 256);
            draft(&ime, &seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            // Host-like adoption/reseed, not an assertion of physical Backspace.
            for tail in ["  ", "  be", "  before"] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(
                    &ime,
                    &format!("{word}  before we leave."),
                    CandidateKind::Sentence,
                );
            }
            commit_once(
                &mut ime,
                &format!("{word}  before we leave."),
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}
#[test]
fn care_common_words_are_reachable_without_relabelling_old_hand_and_washing() {
    for word in ["wash", "hair", "hands", "hand", "washing"] {
        let ime = engine(word);
        draft(&ime, word);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
        commit_once(&mut engine(word), word, CandidateKind::Word);
    }
    // Broad was/han prefixes keep their old choices; exact new forms and
    // context-specific partials do not require replacing those old slots.
    index(&engine("wash"), "wash", CandidateKind::Word);
    index(&engine("hai"), "hair", CandidateKind::Word);
    index(&engine("hands"), "hands", CandidateKind::Word);
    commit_once(&mut engine("hai"), "hai", CandidateKind::Literal);
    for (seed, sentence) in [
        (
            "i need to bring my gla",
            "i need to bring my glasses so i can read at work.",
        ),
        (
            "could i borrow your ch",
            "could i borrow your charger until tonight?",
        ),
        ("i'm getting ready for wo", "i'm getting ready for work."),
    ] {
        index(&engine(seed), sentence, CandidateKind::Sentence);
    }
}
#[test]
fn care_does_not_invent_sentences_across_lines_or_protected_tokens() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("i need to wash{separator}my ha");
        let forbidden = format!("i need to wash{separator}my hands before we leave.");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == forbidden && c.kind == CandidateKind::Sentence)
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
    for prefix in ["https://('", "file_name_('", "unknownprefix"] {
        let seed = format!("{prefix}i need to wash my ha");
        let forbidden = format!("{prefix}i need to wash my hands before we leave.");
        draft(&engine(&seed), &seed);
        assert!(
            !engine(&seed)
                .candidates()
                .iter()
                .any(|c| c.text == forbidden && c.kind == CandidateKind::Sentence)
        );
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}
struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "care-synthetic-unavailable"
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
fn care_fallback_survives_missing_failed_timed_out_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in CARE {
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
fn care_preserves_every_old_layer_context_sentence_and_index_rank() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..29];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0x3db8d02b851575bb
    );
    assert_eq!(hash_pairs(en.next_words().take(685)), 0xd70a69d4c597ffc8);
    assert_eq!(
        hash_pairs(en.sentences().take(1334).map(|s| (s, &[][..]))),
        0x53acacc40f6af5f1
    );
    let before = word_positions(previous);
    assert_eq!(before.len(), 6140);
    let after = word_positions(en.word_layers());
    for (word, rank) in &before {
        assert_eq!(
            after.get(word),
            Some(rank),
            "old index rank changed: {word}"
        );
    }
    assert_eq!(
        word_positions(&en.word_layers()[..30])
            .keys()
            .filter(|w| !before.contains_key(*w))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["hair", "hands", "wash"]
    );
    assert_eq!(word_positions(&en.word_layers()[..30]).len(), 6143);
    assert_eq!(after.len(), 6216);
}
#[test]
fn care_adds_three_words_eight_contexts_and_sixteen_fixed_sentences() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(29)
        .expect("missing authored daily_care layer");
    assert_eq!(layer.id, "daily_care");
    assert_eq!(layer.words, ["wash", "hair", "hands"]);
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(CARE) {
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
        CARE.iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, s)| *s))
            .collect::<Vec<_>>()
    );
}
