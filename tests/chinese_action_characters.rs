//! Finite data-driven everyday actions, not unrestricted Pinyin or tone disambiguation.
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

const ACTIONS: &[(&str, &str, &str)] = &[
    ("na", "拿", "na2"),
    ("fang", "放", "fang4"),
    ("dai", "带", "dai4"),
    ("yong", "用", "yong4"),
];

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
fn four_action_readings_append_without_rewriting_previous_words_or_adding_canned_sentences() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(zh.readings().len(), 3442);
    assert_eq!(zh.continuations().count(), 959);
    assert_eq!(
        zh.continuations().map(|(_, v)| v.len()).sum::<usize>(),
        1916
    );
    for (entry, &(reading, text, _)) in zh.readings()[3166..3170].iter().zip(ACTIONS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(!zh.continuations().any(|(context, _)| context == text));
    }
}

#[test]
fn shorter_prefixes_and_prior_pagination_keep_ten_real_before_data_vectors() {
    // Before-data samples include every text, label, kind, source, score bit and position.
    for (seed, expected) in [
        ("n", 0x19bf212c68d2c2e7),
        ("f", 0x96e754cee503b597),
        ("fa", 0x34c3a49d2e7fb179),
        ("fan", 0xa29ffbf823783339),
        ("d", 0x8074bc065012ca87),
        ("da", 0x5aee63e89579a451),
        ("y", 0x03e8c56a3a7f2b8f),
        ("yo", 0x8870ed2bb625873c),
        ("huan ge", 0x999081dd912ed081),
        ("haimeishu", 0x08b8a281d7806afd),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn exact_actions_case_tone_and_horizontal_padding_keep_raw_and_once_only_commit() {
    for &(reading, word, toned) in ACTIONS {
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
fn exact_actions_precede_broad_words_but_keep_old_decoder_order_and_all_old_choices() {
    for (seed, word, core, extra) in [
        ("na", "拿", ["奶茶", "耐心", "难怪", "闹钟"], "耐用"),
        ("fang", "放", ["放心", "方便", "方法", "方案"], "房间"),
        ("dai", "带", ["代码", "待办", "待办事项", "代收"], "待会儿"),
        (
            "yong",
            "用",
            ["用户名", "用法", "用词", "用完还给我"],
            "用完了",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        primary(&ime, word, CandidateKind::Word);
        // Decoder four and separately weighted mixed extra are distinct sources.
        let positions = core.map(|old| index(&ime, old, CandidateKind::Word));
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        for old in core.into_iter().chain([extra]) {
            commit_once(&mut engine(seed), old, CandidateKind::Word);
        }
    }
}

#[test]
fn full_homophones_keep_old_primary_and_sensitive_partials_remain_selectable() {
    for (reading, word) in [
        ("na'bian", "那边"),
        ("na'ge'geng'hao", "哪个更好"),
        ("na'tian'fang'bian", "哪天方便"),
        ("na'guo'lai", "拿过来"),
        ("na'guo'qu", "拿过去"),
        ("fang'xin", "放心"),
        ("fang'bian", "方便"),
        ("fang'fa", "方法"),
        ("fang'an", "方案"),
        ("fang'jian", "房间"),
        ("fang'shui", "防水"),
        ("dai'ma", "代码"),
        ("dai'ban", "待办"),
        ("dai'shou", "代收"),
        ("dai'shang", "带上"),
        ("yong'fa", "用法"),
        ("yong'ci", "用词"),
        ("yong'hu'ming", "用户名"),
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
        }
    }
    for (seed, word, sentences) in [
        (
            "na guo",
            "拿过来",
            ["拿过来吧，我帮你看看。", "拿过来放在这里就好。"],
        ),
        (
            "na tian",
            "哪天方便",
            ["哪天方便，我们约个时间见面。", "哪天方便，提前告诉我一声。"],
        ),
        (
            "fang na",
            "放哪了",
            ["放哪了，我刚才还看见了。", "放哪了，帮我一起找找吧。"],
        ),
        (
            "fang jin chou t",
            "放进抽屉",
            [
                "放进抽屉里，别放在桌子边上。",
                "放进抽屉之前，先把东西分类。",
            ],
        ),
        (
            "yong wan huan gei w",
            "用完还给我",
            ["用完还给我就行，不用着急。", "用完还给我，我明天还要用。"],
        ),
    ] {
        draft(&engine(seed), seed);
        index(&engine(seed), word, CandidateKind::Word);
        for sentence in sentences {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
    }
    for (seed, word) in [("fang xi", "放心"), ("dai zo", "带走")] {
        assert!(index(&engine(seed), word, CandidateKind::Word) < PAGE_SIZE);
    }
    let ime = engine("shi yong");
    primary(&ime, "使用", CandidateKind::Word);
    let positions =
        ["使用", "食用", "实用", "适用"].map(|word| index(&ime, word, CandidateKind::Word));
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
}

#[test]
fn new_actions_compose_with_known_words_without_fixed_phrase_entries_or_semantic_repair() {
    for (reading, sentence) in [
        ("wo'na'shu", "我拿书"),
        ("wo'fang'shu", "我放书"),
        ("wo'dai'shu", "我带书"),
        ("wo'yong'shu", "我用书"),
        ("na'gei'ni", "拿给你"),
        ("yong'yi'xia", "用一下"),
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
    // Before data, this was the only local completion; do not erase it.
    commit_once(&mut engine("wo fang shu"), "我防水", CandidateKind::Word);
    commit_once(
        &mut engine("yong yi xia"),
        "用一下就还",
        CandidateKind::Word,
    );
}

#[test]
fn adopted_actions_mixed_han_and_long_prefixes_stay_editable_and_lossless() {
    for (reading, word, tail, sentence) in [
        ("na", "拿", "gei wo", "拿给我"),
        ("fang", "放", "zhe ge", "放这个"),
        ("dai", "带", "zhe ge", "带这个"),
        ("yong", "用", "zhe ge", "用这个"),
    ] {
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
            let seed = format!("{word}{gap}{tail}");
            let expected = format!("{word}{gap}{}", &sentence[word.len()..]);
            ime.seed(&seed);
            draft(&ime, &seed);
            index(&ime, &expected, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), &expected, CandidateKind::Sentence);
        }
        ime.seed(reading);
        primary(&ime, word, CandidateKind::Word);
    }
    for (seed, word, sentence) in [
        (
            "我明天dai lai huan ni",
            "我明天带来还你",
            "我明天带来还你，谢谢你借给我用。",
        ),
        (
            "先fang我这里保管",
            "先放我这里保管",
            "先放我这里保管，你走的时候再拿。",
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut engine(seed), word, CandidateKind::Word);
        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for &(reading, word, _) in ACTIONS {
            let seed = format!("{prefix}{reading}");
            let converted = format!("{prefix}{word}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
            commit_once(&mut engine(&seed), &converted, CandidateKind::Word);
        }
    }
}

#[test]
fn actions_preserve_quotes_numeric_punctuation_bad_splits_and_line_or_token_boundaries() {
    for &(reading, word, _) in ACTIONS {
        for (ending, kept) in [("'", "'"), ("4.5", "4.5"), ("2:30", "2:30"), ("40", "40")] {
            let seed = format!("{reading}{ending}");
            let text = format!("{word}{kept}");
            draft(&engine(&seed), &seed);
            commit_once(&mut engine(&seed), &text, CandidateKind::Sentence);
            assert!(!engine(&seed).candidates().iter().any(|c| c.text == word));
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            let seed = format!("{reading}{boundary}shu");
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(
                !ime.candidates()
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
        "n a",
        "f ang",
        "fa ng",
        "da i",
        "yo ng",
        "https://example.invalid/na",
        "person.fang@example.invalid",
        "file_dai",
        "用zzqvjy",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| ACTIONS.iter().any(|(_, word, _)| c.text == *word))
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "action-characters-synthetic-error"
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
fn unavailable_timeout_and_empty_model_responses_preserve_all_immediate_local_actions() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for &(reading, word, _) in ACTIONS {
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
