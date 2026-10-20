//! Fixed authored routine/status fallback, independent of model and personal history.
//! Expectations are written before data; every prior index position is protected.
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

const ROUTINE: &[(&str, [(&str, &str); 2])] = &[
    (
        "i just woke",
        [
            ("up", "i just woke up."),
            ("early", "i just woke early this morning."),
        ],
    ),
    (
        "i'm getting ready for",
        [
            ("work", "i'm getting ready for work."),
            ("bed", "i'm getting ready for bed."),
        ],
    ),
    (
        "i'm still at",
        [
            ("work", "i'm still at work, so i'll reply later."),
            ("home", "i'm still at home, but i'll leave soon."),
        ],
    ),
    (
        "i've finished my",
        [
            ("work", "i've finished my work for today."),
            ("homework", "i've finished my homework, so i'm free now."),
        ],
    ),
    (
        "i'm taking a short",
        [
            ("break", "i'm taking a short break."),
            ("walk", "i'm taking a short walk before dinner."),
        ],
    ),
    (
        "i need to get some",
        [
            ("sleep", "i need to get some sleep tonight."),
            ("rest", "i need to get some rest after work."),
        ],
    ),
    (
        "i didn't sleep very",
        [
            ("well", "i didn't sleep very well last night."),
            ("much", "i didn't sleep very much, so i'm tired."),
        ],
    ),
    (
        "i'm feeling a bit",
        [
            ("tired", "i'm feeling a bit tired today."),
            ("cold", "i'm feeling a bit cold, so i'll stay inside."),
        ],
    ),
    (
        "i'll be home after",
        [
            ("work", "i'll be home after work."),
            ("lunch", "i'll be home after lunch today."),
        ],
    ),
    (
        "i'm going to",
        [
            ("bed", "i'm going to bed soon."),
            ("work", "i'm going to work now."),
        ],
    ),
    (
        "i'll take care of it",
        [
            ("later", "i'll take care of it later today."),
            ("tomorrow", "i'll take care of it tomorrow morning."),
        ],
    ),
    (
        "we can take it",
        [
            ("slowly", "we can take it slowly today."),
            ("easy", "we can take it easy this evening."),
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
fn routine_word_and_sentence_branches_are_on_page_one_at_full_and_partial_contexts() {
    for (context, branches) in ROUTINE {
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
fn every_routine_branch_sentence_and_exact_raw_draft_commits_and_undoes_once() {
    for (context, branches) in ROUTINE {
        for (next, sentence) in branches {
            let seed = format!("{context} {}", &next[..2]);
            commit_once(
                &mut engine(&seed),
                &format!("{context} {next}"),
                CandidateKind::Word,
            );
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            let mut raw = engine(&seed);
            // Exact existing short words (e.g. up/we) must retain their Word
            // label; raw text preservation does not require relabelling them.
            let kind = raw.candidates()[0].kind;
            commit_once(&mut raw, &seed, kind);
        }
    }
    let seed = "i need to get some sl";
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
}

#[test]
fn routine_preserves_case_apostrophes_and_horizontal_spacing() {
    for (seed, word, sentence) in [
        (
            "I just woke EA",
            "I just woke EARLY",
            "I just woke EARLY this morning.",
        ),
        (
            "I’m getting ready for wo",
            "I’m getting ready for work",
            "I’m getting ready for work.",
        ),
        (
            "i’ve finished my ho",
            "i’ve finished my homework",
            "i’ve finished my homework, so i'm free now.",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for padding in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("{padding}i{padding}just{padding}woke{padding}ea");
        let word = format!("{padding}i{padding}just{padding}woke{padding}early");
        let sentence = format!("{word} this morning.");
        draft(&engine(&seed), &seed);
        index(&engine(&seed), &word, CandidateKind::Word);
        commit_once(&mut engine(&seed), &sentence, CandidateKind::Sentence);
    }
}

#[test]
fn routine_cumulative_spaces_and_adopted_words_preserve_long_drafts_without_implicit_commit() {
    let mut raw = String::new();
    let mut ime = engine("");
    for fragment in [
        "i", " ", "need", " ", "to", " ", "get", " ", "some", " ", "sl",
    ] {
        raw.push_str(fragment);
        // Complete host draft, not append_seed's trimmed word-fragment API.
        ime.seed(&raw);
        draft(&ime, &raw);
    }
    index(&ime, "i need to get some sleep", CandidateKind::Word);
    index(
        &ime,
        "i need to get some sleep tonight.",
        CandidateKind::Sentence,
    );
    for prefix in ["Earlier note. ".repeat(40), "前文😀。  ".repeat(100)] {
        for separator in ["—", "，", "。", "！", "？", "；", "：", "、"] {
            let seed = format!("{prefix}{separator}i need to get some sl");
            let word = format!("{prefix}{separator}i need to get some sleep");
            let mut ime = engine(&seed);
            assert!(seed.chars().count() > 256);
            draft(&ime, &seed);
            ime.select_candidate(index(&ime, &word, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(word.as_str()));
            // Host-like adoption/reseed; physical adoption and Backspace are
            // validated separately in the private GTK vocabulary gate.
            for tail in ["  ", "  to", "  tonight"] {
                let continued = format!("{word}{tail}");
                ime.seed(&continued);
                draft(&ime, &continued);
                index(&ime, &format!("{word}  tonight."), CandidateKind::Sentence);
            }
            commit_once(
                &mut ime,
                &format!("{word}  tonight."),
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn routine_keeps_old_short_context_choices_on_page_one() {
    for (seed, words, sentences) in [
        (
            "i'm getting ",
            ["i'm getting tired", "i'm getting hungry"],
            ["i'm getting tired.", "i'm getting hungry."],
        ),
        (
            "i'll be home ",
            ["i'll be home soon", "i'll be home late"],
            ["i'll be home soon.", "i'll be home late tonight."],
        ),
        (
            "i'll take care of ",
            ["i'll take care of it", "i'll take care of this"],
            ["i'll take care of it.", "i'll take care of this."],
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        for word in words {
            index(&ime, word, CandidateKind::Word);
        }
        for sentence in sentences {
            index(&ime, sentence, CandidateKind::Sentence);
        }
    }
    index(
        &engine("i'll take care of it"),
        "i'll take care of it.",
        CandidateKind::Sentence,
    );
}

#[test]
fn routine_does_not_invent_sentences_across_lines_or_protected_tokens() {
    for separator in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("i need to get{separator}some sl");
        let forbidden = format!("i need to get{separator}some sleep tonight.");
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
        let seed = format!("{prefix}i need to get some sl");
        let forbidden = format!("{prefix}i need to get some sleep tonight.");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == forbidden && c.kind == CandidateKind::Sentence)
        );
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

struct MissingProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for MissingProvider {
    fn provider_id(&self) -> &str {
        "routine-synthetic-unavailable"
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
fn routine_local_candidates_survive_missing_failed_timed_out_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (context, branches) in ROUTINE {
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
fn routine_preserves_every_old_layer_context_sentence_and_index_rank() {
    let en = builtin("en").unwrap();
    let previous = &en.word_layers()[..26];
    assert_eq!(
        hash_pairs(previous.iter().map(|l| (l.id.as_str(), l.words.as_slice()))),
        0x076e6556469d48e9
    );
    assert_eq!(hash_pairs(en.next_words().take(657)), 0x74823218bb49ff0d);
    assert_eq!(
        hash_pairs(en.sentences().take(1278).map(|s| (s, &[][..]))),
        0x346a20bbb3039904
    );
    let before = word_positions(previous);
    assert_eq!(before.len(), 6139);
    let after = word_positions(en.word_layers());
    assert_eq!(
        before,
        word_positions(&en.word_layers()[..27]),
        "routine's own append must reuse old forms without changing any index rank"
    );
    for (word, rank) in &before {
        assert_eq!(
            after.get(word),
            Some(rank),
            "old index rank changed: {word}"
        );
    }
    assert_eq!(after.len(), 6151);
}
#[test]
fn routine_adds_twelve_fixed_contexts_and_twenty_four_sentences_without_index_growth() {
    let en = builtin("en").unwrap();
    let layer = en
        .word_layers()
        .get(26)
        .expect("missing authored daily_routine layer");
    assert_eq!(layer.id, "daily_routine");
    assert!(layer.words.is_empty());
    assert_eq!(layer.next_words.len(), 12);
    assert_eq!(layer.sentences.len(), 24);
    for ((context, words), (expected, branches)) in layer.next_words.iter().zip(ROUTINE) {
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
        ROUTINE
            .iter()
            .flat_map(|(_, branches)| branches.iter().map(|(_, s)| *s))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        word_positions(&en.word_layers()[..26]),
        word_positions(&en.word_layers()[..27])
    );
}
