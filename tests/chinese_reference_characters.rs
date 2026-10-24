//! Deictic/classifier data; defer 里 and 那 until their parsing/source risks are resolved.
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

const REFERENCES: &[(&str, &str, &str)] = &[("zhe", "这", "zhe4"), ("ben", "本", "ben3")];

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
fn two_independent_reference_readings_append_without_authored_contexts() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(zh.readings().len(), 3442);
    assert_eq!(zh.continuations().count(), 906);
    assert_eq!(
        zh.continuations().map(|(_, v)| v.len()).sum::<usize>(),
        1810
    );
    for (entry, &(reading, text, _)) in zh.readings()[3176..3178].iter().zip(REFERENCES) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(!zh.continuations().any(|(context, _)| context == text));
    }
}

#[test]
fn shorter_prefixes_prior_characters_and_pages_keep_thirty_real_before_vectors() {
    // Real pre-data samples: text, label, role, source, score bits and order.
    for (seed, expected) in [
        ("l", 0xad5bf787e7107897),
        ("b", 0x073741f1e79e26f7),
        ("be", 0xae0e59b7f21e95ba),
        ("beng", 0x58e80f278006d3dc),
        ("xian", 0x68030c8f5f2a1338),
        ("nian", 0xc3606f0d545c19cd),
        ("qian", 0xa7a8ed4d1caf176a),
        ("z", 0x428baa048a56fcc7),
        ("zh", 0x94cebaffe247bdf7),
        ("n", 0x19bf212c68d2c2e7),
        ("na", 0xe9be9c02bae62726),
        ("na guo lai", 0xc37e4a2218d764e5),
        ("lian", 0x893cb39af5230afb),
        ("lin", 0xf0b13c0ef2e3bfd2),
        ("zhen", 0x65b8a35200c955b2),
        ("zheng", 0x00d1d1964eacec4a),
        ("nai", 0x5f415a5703eadfe3),
        ("nan", 0x651be393c1ce3b32),
        ("nao", 0x809ceafa2f5caccb),
        ("wo jie ni", 0x101197e5eddca6ac),
        ("shi yong", 0xeb03afc31a106dd9),
        ("haimeishu", 0x08b8a281d7806afd),
        ("huan ge", 0x999081dd912ed081),
        ("dao", 0x797e682dcd20ac93),
        ("jia", 0x361af2db64232653),
        ("guo", 0x3072f706fe34dc50),
        ("wan", 0xe44186a987ecd304),
        ("hui", 0x9f40febe002c7d3a),
        ("zhao", 0x6d0f33d2d8a69f95),
        ("hai mei zhao da", 0xa43bd156c711841f),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn exact_case_tone_and_padding_keep_na_primary_raw_and_once_only_commit() {
    for &(reading, word, toned) in REFERENCES {
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
                assert!(index(&ime, word, CandidateKind::Word) < PAGE_SIZE);
                commit_once(&mut engine(&raw), word, CandidateKind::Word);
                commit_once(&mut engine(&raw), &raw, CandidateKind::Literal);
            }
        }
    }
    for seed in ["na2", "NA2", "na3", "NA3", "na4", "NA4"] {
        primary(&engine(seed), "拿", CandidateKind::Word);
        commit_once(&mut engine(seed), "拿", CandidateKind::Word);
    }
}

#[test]
fn exact_words_keep_decoder_four_order_and_separately_weighted_extra_choices() {
    for (seed, word, core, extra) in [
        (
            "ben",
            "本",
            ["本周", "崩溃", "本地", "本地模型"],
            "本地词库",
        ),
        ("zhe", "这", ["真的", "整理", "争议", "正文"], "正好"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        index(&ime, word, CandidateKind::Word);
        let positions = core.map(|old| index(&ime, old, CandidateKind::Word));
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        for old in core.into_iter().chain([extra]) {
            commit_once(&mut engine(seed), old, CandidateKind::Word);
        }
    }
}

#[test]
fn full_compounds_and_sensitive_partials_retain_existing_words_sentences_and_homophone_order() {
    // Do not add another na Exact until all four old Prefix paths retain sources.
    for sentence in ["闹钟已经设好了。", "闹钟需要提前十分钟。"] {
        commit_once(&mut engine("na"), sentence, CandidateKind::Sentence);
    }
    for (reading, word) in [
        ("ben'zhou", "本周"),
        ("ben'di", "本地"),
        ("ben'di'mo'xing", "本地模型"),
        ("ben'di'ci'ku", "本地词库"),
        ("li'ke", "立刻"),
        ("li'jie", "理解"),
        ("li'mian", "里面"),
        ("lian'xi", "联系"),
        ("jing'li", "经理"),
        ("zhen'de", "真的"),
        ("zheng'li", "整理"),
        ("zheng'wen", "正文"),
        ("zhe'ge", "这个"),
        ("zhe'bian", "这边"),
        ("na'bian", "那边"),
        ("na'tian'fang'bian", "哪天方便"),
        ("zai'na'li", "在哪里"),
        ("na'guo'lai", "拿过来"),
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
    for (seed, first, second) in [("lian xi", "联系", "练习"), ("jing li", "经理", "经历")]
    {
        let ime = engine(seed);
        assert!(index(&ime, first, CandidateKind::Word) < index(&ime, second, CandidateKind::Word));
    }
    for (seed, word, sentences) in [
        (
            "ben di",
            "本地模型",
            ["本地模型已经加载了。", "本地模型还在加载中。"],
        ),
        (
            "fang na l",
            "放哪了",
            ["放哪了，我刚才还看见了。", "放哪了，帮我一起找找吧。"],
        ),
        (
            "na guo lai",
            "拿过来",
            ["拿过来吧，我帮你看看。", "拿过来放在这里就好。"],
        ),
        (
            "zhe li",
            "这两个有什么区别",
            [
                "这两个有什么区别，可以具体说说吗？",
                "这两个有什么区别，我们对照着看一下。",
            ],
        ),
    ] {
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    commit_once(&mut engine("dao zhe li"), "到这里就好", CandidateKind::Word);
    commit_once(&mut engine("dao na li"), "到哪里了", CandidateKind::Word);
}

#[test]
fn known_readings_compose_references_without_new_fixed_phrases() {
    for (reading, text) in [
        ("zhe'ben'shu", "这本书"),
        ("wo'yong'zhe", "我用这"),
        ("wo'na'zhe'ben'shu", "我拿这本书"),
        ("zhe'ben'shu'gei'ni", "这本书给你"),
        ("wo'you'zhe'ben'shu", "我有这本书"),
    ] {
        for seed in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            draft(&engine(&seed), &seed);
            assert!(index(&engine(&seed), text, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(&mut engine(&seed), text, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_references_and_book_progress_preserve_spaces_and_unicode_frozen_prefixes() {
    for (reading, word, tail, suffix) in
        [("zhe", "这", "ben shu", "本书"), ("ben", "本", "shu", "书")]
    {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{word}{gap}{tail}");
            ime.seed(&seed);
            draft(&ime, &seed);
            commit_once(
                &mut engine(&seed),
                &format!("{word}{gap}{suffix}"),
                CandidateKind::Sentence,
            );
        }
    }
    let mut adopted = engine("zhe ben shu");
    adopted.select_candidate(index(&adopted, "这本书", CandidateKind::Sentence));
    assert_eq!(adopted.selected_completion_text(true), Some("这本书"));
    assert!(adopted.snapshot().committed_text.is_empty());
    for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let seed = format!("这本书{gap}gei ni");
        adopted.seed(&seed);
        draft(&adopted, &seed);
        commit_once(
            &mut engine(&seed),
            &format!("这本书{gap}给你"),
            CandidateKind::Sentence,
        );
    }
    for (seed, text) in [
        ("这ben shu", "这本书"),
        ("我拿zhe ben shu", "我拿这本书"),
        ("本shu", "本书"),
    ] {
        draft(&engine(seed), seed);
        commit_once(&mut engine(seed), text, CandidateKind::Sentence);
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for &(reading, word, _) in REFERENCES {
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
fn numeric_quotes_wrong_splits_and_protected_or_line_boundaries_remain_literal_safe() {
    for &(reading, word, _) in REFERENCES {
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
        "be n",
        "b en",
        "zh e",
        "z he",
        "n a",
        "https://example.invalid/ben",
        "person.zhe@example.invalid",
        "file_na",
        "本zzqvjy",
    ] {
        draft(&engine(seed), seed);
        assert!(
            !engine(seed)
                .candidates()
                .iter()
                .any(|c| REFERENCES.iter().any(|(_, w, _)| c.text == *w))
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "reference-characters-synthetic-error"
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
        for &(reading, word, _) in REFERENCES {
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
