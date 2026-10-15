//! Fixed expectations for two basic readings, not an unrestricted character dictionary.
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
    lexicon::{self, EntryKind},
};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn signature(ime: &XRTabletImeEngine) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for candidate in ime.candidates() {
        for byte in candidate
            .text
            .bytes()
            .chain([0])
            .chain(candidate.label.bytes())
            .chain([0])
            .chain(candidate.kind.id().bytes())
            .chain([0])
            .chain(candidate.source.id().bytes())
            .chain([0])
            .chain(candidate.score.to_bits().to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
    index(ime, seed, CandidateKind::Literal);
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}

fn primary(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert_eq!(index(ime, text, kind), 0);
    assert_eq!(ime.candidates()[0].score, 100.0);
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
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn only_two_readings_are_appended_without_context_or_sentence_growth() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(zh.readings().len(), 3178);
    assert_eq!(zh.continuations().count(), 767);
    assert_eq!(
        zh.continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1532
    );
    for (entry, (reading, text)) in zh.readings()[3162..3164]
        .iter()
        .zip([("jie", "借"), ("huan", "还")])
    {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let old_alias = &zh.readings()[2920];
    assert_eq!((&*old_alias.reading, &*old_alias.text), ("hai", "还"));
}

#[test]
fn shorter_prefixes_and_old_hai_alias_keep_the_entire_frozen_candidate_vector() {
    // Captured from the exact pre-data runtime, not recomputed from the new data.
    // All texts, labels, roles, sources, score bits and positions participate.
    for (seed, expected) in [
        ("j", 0x66d9f31de656eb47),
        ("ji", 0xd47726af584226a8),
        ("hu", 0x81d739caaaa52cf4),
        ("hua", 0xb712264db5b762ee),
        ("ha", 0x171e3f070d01a8de),
        ("hai", 0x43081278a2a4c99c),
        ("HAI", 0x093e94dce1320d5c),
        ("hai2", 0x0c90dc582e61a748),
        ("hai  ", 0xf6fa9d4cd36d6d8c),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn full_basic_readings_tones_case_and_horizontal_endings_offer_exact_words() {
    for (variants, word) in [
        (["jie", "JIE", "jie4", "JIE4"], "借"),
        (["huan", "HUAN", "huan2", "HUAN2"], "还"),
    ] {
        for reading in variants {
            for ending in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let seed = format!("{reading}{ending}");
                let ime = engine(&seed);
                draft(&ime, &seed);
                primary(&ime, word, CandidateKind::Word);
                commit_once(&mut engine(&seed), word, CandidateKind::Word);
                commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn old_broad_words_remain_selectable_but_exact_characters_intentionally_become_primary() {
    for (seed, word, old_decoder, old_extra) in [
        ("jie", "借", ["介绍", "解释", "结束", "解决"], "结果"),
        (
            "huan",
            "还",
            ["缓存", "换乘", "欢迎回来", "换货"],
            "换乘时间",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        primary(&ime, word, CandidateKind::Word);
        let positions = old_decoder.map(|old| index(&ime, old, CandidateKind::Word));
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        index(&ime, old_extra, CandidateKind::Word);
        for old in old_decoder.into_iter().chain([old_extra]) {
            commit_once(&mut engine(seed), old, CandidateKind::Word);
        }
    }
}

#[test]
fn complete_compounds_and_existing_homophones_keep_their_precise_primary() {
    for (reading, word) in [
        ("jie'shu", "结束"),
        ("jie'shao", "介绍"),
        ("jie'shi", "解释"),
        ("jie'guo", "结果"),
        ("huan'cun", "缓存"),
        ("huan'cheng", "换乘"),
        ("huan'ying'hui'lai", "欢迎回来"),
        ("huan'huo", "换货"),
        ("huan'shu", "还书"),
        ("hai'mei", "还没"),
        ("hai'shi", "还是"),
        ("hai'xian", "海鲜"),
        ("hai'ke'yi", "还可以"),
        ("jie'wo'yi'xia", "借我一下"),
        ("wo'ming'tian'dai'lai'huan'ni", "我明天带来还你"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            primary(&ime, word, CandidateKind::Word);
            if word == "结束" {
                assert!(index(&ime, "借书", CandidateKind::Word) < PAGE_SIZE);
                assert!(
                    index(&ime, "结束", CandidateKind::Word)
                        < index(&ime, "借书", CandidateKind::Word)
                );
            }
        }
    }
    // The existing normalizer accepts tone numbers, but does not disambiguate tones.
    primary(&engine("jie4shu1"), "结束", CandidateKind::Word);
    for (seed, word) in [
        ("huange", "换个说法"),
        ("huan ge", "换个说法"),
        ("jie sh", "介绍"),
        ("huan sh", "还书"),
    ] {
        primary(&engine(seed), word, CandidateKind::Word);
    }
}

#[test]
fn new_characters_compose_with_known_readings_without_new_canned_phrases() {
    for (reading, text) in [
        ("jie'huan", "借还"),
        ("wo'huan'ni", "我还你"),
        ("wo'jie'ni", "我借你"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            index(&ime, text, CandidateKind::Sentence);
            if text == "我借你" {
                primary(&ime, "我接你", CandidateKind::Sentence);
            }
            commit_once(&mut engine(&seed), text, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_words_space_and_mixed_prefixes_remain_editable_until_explicit_commit() {
    for (reading, word, next, expected, kind) in [
        ("jie", "借", "wo yi xia", "借我一下", CandidateKind::Word),
        ("huan", "还", "ni", "还你", CandidateKind::Sentence),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        let seed = format!("{word}{next}");
        ime.seed(&seed);
        draft(&ime, &seed);
        index(&ime, expected, kind);
        ime.seed(reading);
        primary(&ime, word, CandidateKind::Word);
        for gap in [" ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{word}{gap}{next}");
            let spaced = format!("{word}{gap}{}", &expected[word.len()..]);
            ime.seed(&seed);
            draft(&ime, &seed);
            index(&ime, &spaced, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), &spaced, CandidateKind::Sentence);
        }
    }
    for (seed, word, sentence) in [
        ("归huan", "归还", None),
        ("归还tu shu", "归还图书", Some("归还图书可以放进还书箱。")),
        (
            "这个能jie我两天吗",
            "这个能借我两天吗",
            Some("这个能借我两天吗？周末我就还给你。"),
        ),
        (
            "我明天带来huan你",
            "我明天带来还你",
            Some("我明天带来还你，谢谢你借给我用。"),
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        if let Some(sentence) = sentence {
            index(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(80)] {
        for (reading, word) in [("jie", "借"), ("huan", "还")] {
            let seed = format!("{prefix}{reading}");
            let text = format!("{prefix}{word}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
            index(&ime, &text, CandidateKind::Word);
            commit_once(&mut engine(&seed), &text, CandidateKind::Word);
        }
    }
}

#[test]
fn quotes_numbers_wrong_splits_and_protected_boundaries_remain_literal() {
    for (seed, converted) in [
        ("jie'", "借'"),
        ("huan'", "还'"),
        ("jie4.5", "借4.5"),
        ("huan2:30", "还2:30"),
        ("jie40", "借40"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, converted, CandidateKind::Sentence);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == "借" || c.text == "还")
        );
        commit_once(&mut engine(seed), converted, CandidateKind::Sentence);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for seed in [
        "j ie",
        "ji e",
        "h uan",
        "hu an",
        "hua n",
        "https://example.invalid/jie",
        "person+huan@example.invalid",
        "file_jie",
        "借zzqvjy",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == "借" || c.text == "还")
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("jie{boundary}huan");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(!ime.candidates().iter().any(|c| c.text == "借还"));
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
    for seed in ["借", "还"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.kind == CandidateKind::Word && c.text == seed)
        );
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "lending-characters-synthetic-error"
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
fn unavailable_timeout_and_empty_model_responses_keep_immediate_local_characters() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (reading, word) in [("jie", "借"), ("huan", "还")] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedProvider(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            primary(&ime, word, CandidateKind::Word);
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
            commit_once(&mut ime, word, CandidateKind::Word);
        }
    }
}
