//! Finite arrival/home readings, not arbitrary Pinyin or semantic repair.
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

const ARRIVALS: &[(&str, &str, &str)] = &[("dao", "到", "dao4"), ("jia", "家", "jia1")];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
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
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn signature(ime: &XRTabletImeEngine) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for c in ime.candidates() {
        for byte in c
            .text
            .bytes()
            .chain([0])
            .chain(c.label.bytes())
            .chain([0])
            .chain(c.kind.id().bytes())
            .chain([0])
            .chain(c.source.id().bytes())
            .chain([0])
            .chain(c.score.to_bits().to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[test]
fn two_independent_readings_append_after_result_window_without_authored_contexts() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(zh.readings().len(), 3330);
    assert_eq!(zh.continuations().count(), 821);
    assert_eq!(
        zh.continuations().map(|(_, v)| v.len()).sum::<usize>(),
        1640
    );
    for (entry, &(reading, text, _)) in zh.readings()[3174..3176].iter().zip(ARRIVALS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(!zh.continuations().any(|(context, _)| context == text));
    }
}

#[test]
fn shorter_prefixes_previous_characters_and_sensitive_pages_keep_sixteen_before_vectors() {
    // Real pre-data samples include every text, label, role, source, score bit and position.
    for (seed, expected) in [
        ("d", 0x8074bc065012ca87),
        ("da", 0x5aee63e89579a451),
        ("j", 0x66d9f31de656eb47),
        ("ji", 0xd47726af584226a8),
        ("jian", 0xd86991b897fd7146),
        ("jiao", 0x4b809c26791f04ab),
        ("wo jie ni", 0x101197e5eddca6ac),
        ("shi yong", 0xeb03afc31a106dd9),
        ("haimeishu", 0x08b8a281d7806afd),
        ("huan ge", 0x999081dd912ed081),
        ("guo", 0x3072f706fe34dc50),
        ("wan", 0xe44186a987ecd304),
        ("hui", 0x9f40febe002c7d3a),
        ("zhao", 0x6d0f33d2d8a69f95),
        ("hai mei zhao da", 0xa43bd156c711841f),
        ("到家hou", 0xfb3d7624bb464fb1),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn exact_readings_case_tone_and_horizontal_padding_keep_raw_and_once_only_commit() {
    for &(reading, word, toned) in ARRIVALS {
        for seed in [
            reading.to_owned(),
            reading.to_ascii_uppercase(),
            toned.to_owned(),
            toned.to_ascii_uppercase(),
        ] {
            for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let raw = format!("{seed}{gap}");
                let ime = engine(&raw);
                draft(&ime, &raw);
                primary(&ime, word, CandidateKind::Word);
                commit_once(&mut engine(&raw), word, CandidateKind::Word);
                commit_once(&mut engine(&raw), &raw, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn new_exact_characters_keep_old_decoder_order_and_separately_weighted_extra_choices() {
    for (seed, word, core, extra) in [
        ("dao", "到", ["到达", "导航", "导入", "导出"], "到期"),
        ("jia", "家", ["健康", "建议", "交流", "检查"], "假期"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        primary(&ime, word, CandidateKind::Word);
        let positions = core.map(|old| index(&ime, old, CandidateKind::Word));
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        for old in core.into_iter().chain([extra]) {
            commit_once(&mut engine(seed), old, CandidateKind::Word);
        }
    }
}

#[test]
fn full_compounds_keep_primary_and_sensitive_partials_keep_original_words_and_sentences() {
    for (reading, word) in [
        ("dao'da", "到达"),
        ("dao'hang", "导航"),
        ("dao'ru", "导入"),
        ("dao'chu", "导出"),
        ("dao'qi", "到期"),
        ("dao'jia", "到家"),
        ("dao'le", "到了"),
        ("jia'qi", "假期"),
        ("jia'ge", "价格"),
        ("jia'gou", "架构"),
        ("jia'ban", "加班"),
        ("jia'she", "假设"),
        ("jia'ren", "家人"),
        ("hui'jia", "回家"),
        ("wo'jia", "我家"),
        ("hai'mei'zhao'dao", "还没找到"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            draft(&engine(&seed), &seed);
            primary(&engine(&seed), word, CandidateKind::Word);
        }
    }
    for (seed, word, sentences) in [
        (
            "wo dao",
            "我到了",
            ["我到了，你在哪儿？", "我到了，在门口等你。"],
        ),
        (
            "zhao dao",
            "找到了就在桌上",
            [
                "找到了就在桌上，刚才被纸挡住了。",
                "找到了就在桌上，我这就收起来。",
            ],
        ),
        (
            "hai mei zhao dao l",
            "还没找到了就在桌上",
            [
                "还没找到了就在桌上，刚才被纸挡住了。",
                "还没找到了就在桌上，我这就收起来。",
            ],
        ),
    ] {
        index(&engine(seed), word, CandidateKind::Word);
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    // These Sentence spans existed before this batch via whole-word readings.
    for (seed, sentence) in [
        ("zhao dao le", "找到了"),
        ("wo hui jia", "我回家"),
        ("hui dao jia", "回到家"),
        ("dai dao jia", "带到家"),
        ("wo dao jia le", "我到家了"),
    ] {
        primary(&engine(seed), sentence, CandidateKind::Sentence);
    }
}

#[test]
fn new_known_reading_combinations_convert_without_new_fixed_phrase_or_sentence_records() {
    for (reading, sentence) in [
        ("zhao'dao", "找到"),
        ("wo'zhao'dao", "我找到"),
        ("wo'zhao'dao'shu'le", "我找到书了"),
        ("na'dao'shu", "拿到书"),
        ("wo'de'jia", "我的家"),
        ("ni'de'jia", "你的家"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
    }
    // Preserve the real pre-data, differently interpreted completion too.
    commit_once(&mut engine("na dao shu"), "拿倒水", CandidateKind::Word);
}

#[test]
fn adopting_each_character_keeps_dictionary_roles_spacing_and_mixed_authored_progress() {
    for (reading, word, tail, suffix) in [("dao", "到", "jia", "家"), ("jia", "家", "ren", "人")]
    {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{word}{gap}{tail}");
            let expected = format!("{word}{gap}{suffix}");
            ime.seed(&seed);
            draft(&ime, &seed);
            let kind = if gap.is_empty() {
                CandidateKind::Word
            } else {
                CandidateKind::Sentence
            };
            commit_once(&mut engine(&seed), &expected, kind);
        }
    }
    for (seed, text, kind) in [
        ("拿dao", "拿到", CandidateKind::Sentence),
        ("回到jia", "回到家", CandidateKind::Sentence),
        ("到jia", "到家", CandidateKind::Word),
    ] {
        draft(&engine(seed), seed);
        commit_once(&mut engine(seed), text, kind);
    }
    for text in ["到家后给我发个消息。", "到家了，我先收拾一下。"] {
        commit_once(&mut engine("到jia"), text, CandidateKind::Sentence);
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for &(reading, word, _) in ARRIVALS {
            let seed = format!("{prefix}{reading}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
            commit_once(
                &mut engine(&seed),
                &format!("{prefix}{word}"),
                CandidateKind::Word,
            );
        }
    }
}

#[test]
fn quotes_numeric_punctuation_wrong_splits_and_token_or_line_boundaries_are_lossless() {
    for &(reading, word, _) in ARRIVALS {
        for ending in ["'", "4.5", "2:30", "40"] {
            let seed = format!("{reading}{ending}");
            draft(&engine(&seed), &seed);
            commit_once(
                &mut engine(&seed),
                &format!("{word}{ending}"),
                CandidateKind::Sentence,
            );
            assert!(!engine(&seed).candidates().iter().any(|c| c.text == word));
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("{reading}{boundary}shu");
            draft(&engine(&seed), &seed);
            assert!(
                !engine(&seed)
                    .candidates()
                    .iter()
                    .any(|c| c.text == format!("{word}书"))
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        let ime = engine(word);
        draft(&ime, word);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == word && c.kind == CandidateKind::Word)
        );
    }
    for seed in [
        "da o",
        "ji a",
        "d ao",
        "j ia",
        "https://example.invalid/dao",
        "person.jia@example.invalid",
        "file_dao",
        "家zzqvjy",
    ] {
        draft(&engine(seed), seed);
        assert!(
            !engine(seed)
                .candidates()
                .iter()
                .any(|c| ARRIVALS.iter().any(|(_, w, _)| c.text == *w))
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "arrival-characters-synthetic-error"
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
fn unavailable_timeout_and_empty_model_responses_keep_entire_local_vectors() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for &(reading, word, _) in ARRIVALS {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedProvider(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            primary(&ime, word, CandidateKind::Word);
            index(&ime, word, CandidateKind::Word);
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
