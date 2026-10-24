//! Authored clothing/carrying fallback; expectations precede the resource append.
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

const CARRY: &[(&str, [(&str, &str); 2])] = &[
    (
        "i need to bring my",
        [
            (
                "glasses",
                "i need to bring my glasses so i can read at work.",
            ),
            ("charger", "i need to bring my charger before we leave."),
        ],
    ),
    (
        "don't forget to take your",
        [
            ("keys", "don't forget to take your keys with you."),
            (
                "wallet",
                "don't forget to take your wallet before going out.",
            ),
        ],
    ),
    (
        "please put on your",
        [
            ("jacket", "please put on your jacket before we leave."),
            ("shoes", "please put on your shoes before going outside."),
        ],
    ),
    (
        "you might need a",
        [
            ("sweater", "you might need a sweater tonight."),
            ("scarf", "you might need a scarf outside."),
        ],
    ),
    (
        "i'll wear my",
        [
            ("coat", "i'll wear my coat because it's cold."),
            ("hat", "i'll wear my hat on the walk."),
        ],
    ),
    (
        "do i need to bring an",
        [
            ("umbrella", "do i need to bring an umbrella today?"),
            ("adapter", "do i need to bring an adapter for the trip?"),
        ],
    ),
    (
        "i've packed my",
        [
            ("bag", "i've packed my bag for the trip."),
            ("notebook", "i've packed my notebook for work."),
        ],
    ),
    (
        "i'll keep my",
        [
            ("phone", "i'll keep my phone in my bag."),
            ("wallet", "i'll keep my wallet in my jacket."),
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
fn carry_word_and_sentence_branches_are_on_page_one_at_full_and_partial_contexts() {
    for (context, branches) in CARRY {
        for (next, sentence) in branches {
            for seed in [format!("{context} "), format!("{context} {}", &next[..2])] {
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
fn carry_each_branch_and_raw_draft_commits_and_undoes_exactly_once() {
    for (context, branches) in CARRY {
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
fn carry_preserves_case_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "I need to bring my GL",
            "I need to bring my GLASSES",
            "I need to bring my GLASSES so i can read at work.",
        ),
        (
            "don’t forget to take your ke",
            "don’t forget to take your keys",
            "don’t forget to take your keys with you.",
        ),
        (
            "i’ve packed my no",
            "i’ve packed my notebook",
            "i’ve packed my notebook for work.",
        ),
    ] {
        draft(&engine(seed), seed);
        index(&engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for pad in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{pad}i{pad}need{pad}to{pad}bring{pad}my{pad}gla");
        let word = format!("{pad}i{pad}need{pad}to{pad}bring{pad}my{pad}glasses");
        draft(&engine(&seed), &seed);
        index(&engine(&seed), &word, CandidateKind::Word);
        commit_once(
            &mut engine(&seed),
            &format!("{word} so i can read at work."),
            CandidateKind::Sentence,
        );
    }
}
#[test]
fn carry_spaces_and_adoption_preserve_long_unicode_drafts_without_implicit_commit() {
    let mut raw = String::new();
    let mut ime = engine("");
    for fragment in [
        "i", " ", "need", " ", "to", " ", "bring", " ", "my", " ", "gla",
    ] {
        raw.push_str(fragment);
        ime.seed(&raw);
        draft(&ime, &raw);
    }
    index(&ime, "i need to bring my glasses", CandidateKind::Word);
    index(
        &ime,
        "i need to bring my glasses so i can read at work.",
        CandidateKind::Sentence,
    );
    for prefix in ["Earlier note. ".repeat(40), "前文😀。  ".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}i need to bring my gla");
            let word = format!("{prefix}{separator}i need to bring my glasses");
            let mut ime = engine(&seed);
            assert!(seed.chars().count() > 256);
            draft(&ime, &seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            // Host-like adoption/reseed, not an assertion of physical Backspace.
            for tail in ["  ", "  so", "  so i"] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(
                    &ime,
                    &format!("{word}  so i can read at work."),
                    CandidateKind::Sentence,
                );
            }
            commit_once(
                &mut ime,
                &format!("{word}  so i can read at work."),
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}
#[test]
fn carry_glasses_is_reachable_without_relabelling_the_existing_word_glass() {
    let ime = engine("glass");
    draft(&ime, "glass");
    assert_eq!(ime.candidates()[0].kind, CandidateKind::Word);
    index(&ime, "glass", CandidateKind::Word);
    index(&engine("glasse"), "glasses", CandidateKind::Word);
    commit_once(&mut engine("glasse"), "glasses", CandidateKind::Word);
    commit_once(&mut engine("glasse"), "glasse", CandidateKind::Literal);
    commit_once(&mut engine("glasses"), "glasses", CandidateKind::Word);
    for (seed, sentence) in [
        (
            "could i borrow your ch",
            "could i borrow your charger until tonight?",
        ),
        ("i can return it toda", "i can return it today after work."),
        ("i'm getting ready for wo", "i'm getting ready for work."),
    ] {
        index(&engine(seed), sentence, CandidateKind::Sentence);
    }
}
#[test]
fn carry_does_not_invent_sentences_across_lines_or_protected_tokens() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("i need to bring{separator}my gla");
        let forbidden = format!("i need to bring{separator}my glasses so i can read at work.");
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
        let seed = format!("{prefix}i need to bring my gla");
        let forbidden = format!("{prefix}i need to bring my glasses so i can read at work.");
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
        "carry-synthetic-unavailable"
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
fn carry_fallback_survives_missing_failed_timed_out_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in CARRY {
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
fn carry_preserves_every_old_layer_context_sentence_and_index_rank() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..28];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0x62d0c5c72eecf7d9
    );
    assert_eq!(hash_pairs(en.next_words().take(677)), 0x7940170096f4dfdf);
    assert_eq!(
        hash_pairs(en.sentences().take(1318).map(|s| (s, &[][..]))),
        0xa235928551537357
    );
    let before = word_positions(previous);
    assert_eq!(before.len(), 6139);
    let after = word_positions(en.word_layers());
    for (word, rank) in &before {
        assert_eq!(
            after.get(word),
            Some(rank),
            "old index rank changed: {word}"
        );
    }
    assert_eq!(
        word_positions(&en.word_layers()[..29])
            .keys()
            .filter(|w| !before.contains_key(*w))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["glasses"]
    );
    assert_eq!(word_positions(&en.word_layers()[..29]).len(), 6140);
    assert_eq!(after.len(), 6213);
}
#[test]
fn carry_adds_one_word_eight_contexts_and_sixteen_fixed_sentences() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(28)
        .expect("missing authored daily_carry layer");
    assert_eq!(layer.id, "daily_carry");
    assert_eq!(layer.words, ["glasses"]);
    assert_eq!(layer.next_words.len(), 8);
    assert_eq!(layer.sentences.len(), 16);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(CARRY) {
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
        CARRY
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, s)| *s))
            .collect::<Vec<_>>()
    );
}
