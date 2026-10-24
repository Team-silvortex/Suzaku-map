//! Finite desk-accessory coordination, not electrical-operation advice.
use ring::digest::{SHA256, digest};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{self, EntryKind},
};

const WORDS: &[(&str, &str)] = &[
    ("cha'xian'ban'xian'fang'zhe'bian", "插线板先放这边"),
    ("yan'chang'xian'shou'zai'na'le", "延长线收在哪了"),
    ("zhuan'jie'tou'bang'wo'liu'zhe", "转接头帮我留着"),
    ("dian'yuan'xian'bie'na'hun'le", "电源线别拿混了"),
    ("er'ji'he'fang'zai'zhuo'bian", "耳机盒放在桌边"),
    ("zhe'xie'xian'xian'fen'kai'fang", "这些线先分开放"),
    ("cha'xian'ban", "插线板"),
    ("yan'chang'xian", "延长线"),
    ("dian'yuan'xian", "电源线"),
    ("li'xian'jia", "理线夹"),
];
const TONES: &[&str] = &[
    "cha1'xian4'ban3'xian1'fang4'zhe4'bian1",
    "yan2'chang2'xian4'shou1'zai4'na3'le5",
    "zhuan3'jie1'tou2'bang1'wo3'liu2'zhe5",
    "dian4'yuan2'xian4'bie2'na2'hun4'le5",
    "er3'ji1'he2'fang4'zai4'zhuo1'bian1",
    "zhe4'xie1'xian4'xian1'fen1'kai1'fang4",
    "cha1'xian4'ban3",
    "yan2'chang2'xian4",
    "dian4'yuan2'xian4",
    "li3'xian4'jia1",
];
const PARTIALS: &[&str] = &[
    "cha'xian'ban'xian'fang'zhe'bia",
    "yan'chang'xian'shou'zai'na'l",
    "zhuan'jie'tou'bang'wo'liu'zh",
    "dian'yuan'xian'bie'na'hun'l",
    "er'ji'he'fang'zai'zhuo'bia",
    "zhe'xie'xian'xian'fen'kai'fan",
];
const AUTHORS: [[&str; 2]; 6] = [
    [
        "插线板先放这边，我来腾出一点位置。",
        "插线板先放这边，等整理好再收起来。",
    ],
    [
        "延长线收在哪了？我只找到旁边的袋子。",
        "延长线收在哪了？你看到后告诉我。",
    ],
    [
        "转接头帮我留着，我等会儿来拿。",
        "转接头帮我留着，放在小盒子里就行。",
    ],
    [
        "电源线别拿混了，我再看一下标签。",
        "电源线别拿混了，这根上面贴了标签。",
    ],
    [
        "耳机盒放在桌边，和钥匙放在一起。",
        "耳机盒放在桌边，我出门前会拿走。",
    ],
    [
        "这些线先分开放，我来找几个收纳袋。",
        "这些线先分开放，标签朝外比较好找。",
    ],
];
const GAPS: &[&str] = &[" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"];

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.into(),
        reading.replace('\'', " "),
        reading.replace('\'', ""),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn pool(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local
                && c.label == c.text
                && c.score.is_finite()
                && (0.0..=100.0).contains(&c.score))
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
    let at = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if matches!(kind, CandidateKind::Word | CandidateKind::Sentence) {
        assert!(at < PAGE_SIZE, "specific choice left page one: {text:?}");
    }
    at
}
fn commit_from(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let seed = ime.snapshot().seed_text;
    pool(ime, &seed);
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn commit(seed: &str, text: &str, kind: CandidateKind) {
    commit_from(&mut engine(seed), text, kind);
}
#[test]
fn accessories_preserves_3422_readings_867_contexts_flags_order_and_exact_resource_bytes() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert!(zh.readings().len() >= 3422 && zh.continuations().count() >= 867);
    assert_eq!(
        zh.continuations()
            .take(867)
            .map(|(_, v)| v.len())
            .sum::<usize>(),
        1732
    );
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if zh.readings().len() > 3422 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"cha'xian'ban'xian'fang'zhe'bian\", \"text\": \"插线板先放这边\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"插线板先放这边\",", "\n  ]\n}"),
        ] {
            assert_eq!(raw.matches(first).count(), 1);
            assert_eq!(raw.matches(after).count(), 1);
            let (start, end) = (raw.find(first).unwrap(), raw.find(after).unwrap());
            assert!(start < end);
            raw.replace_range(start..end, "");
        }
    }
    assert_eq!(
        digest(&SHA256, raw.as_bytes())
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "95263b4549158cafedb2b625467195021bd6837692ec1267eb4e2a08a0f5fcf8"
    );
    for &(_, word) in WORDS {
        assert!(!zh.readings()[..3422].iter().any(|e| e.text == word));
    }
}

#[test]
fn accessories_adds_ten_words_six_contexts_and_twelve_authors_without_duplicating_old_nouns() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(
        (zh.readings().len(), zh.continuations().count()),
        (3442, 906)
    );
    assert_eq!(
        zh.continuations().map(|(_, vs)| vs.len()).sum::<usize>(),
        1810
    );
    assert_eq!((WORDS.len(), TONES.len(), PARTIALS.len()), (10, 10, 6));
    for (e, &(reading, text)) in zh.readings()[3422..3432].iter().zip(WORDS) {
        assert_eq!(
            (&*e.reading, &*e.text, e.kind, e.require_separators),
            (reading, text, EntryKind::Word, false)
        );
        assert_eq!(
            zh.readings()
                .iter()
                .filter(|e| &*e.reading == reading || &*e.text == text)
                .count(),
            1
        );
    }
    for (i, (key, vs)) in zh.continuations().skip(867).take(6).enumerate() {
        assert_eq!(key, WORDS[i].1);
        assert_eq!(
            vs.iter().map(String::as_str).collect::<Vec<_>>(),
            AUTHORS[i]
        );
    }
    for text in ["转接头", "耳机盒", "充电器", "充电线", "收纳袋"] {
        assert_eq!(zh.readings().iter().filter(|e| e.text == text).count(), 1);
    }
}

#[test]
fn accessories_specific_full_and_partial_pinyin_offer_words_authors_and_literal_commits() {
    for (i, &(reading, word)) in WORDS.iter().enumerate() {
        for seed in spellings(reading).into_iter().chain([TONES[i].into()]) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
            if i < 6 {
                for author in AUTHORS[i] {
                    commit(&seed, author, CandidateKind::Sentence);
                }
            }
        }
    }
    for (i, partial) in PARTIALS.iter().enumerate() {
        for seed in spellings(partial) {
            commit(&seed, WORDS[i].1, CandidateKind::Word);
            for author in AUTHORS[i] {
                commit(&seed, author, CandidateKind::Sentence);
            }
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn accessories_horizontal_spaces_adopted_tails_and_long_unicode_drafts_are_lossless() {
    for gap in GAPS {
        for (i, &(reading, word)) in WORDS.iter().take(6).enumerate() {
            let raw = format!("{}{gap}", reading.replace('\'', gap));
            commit(&raw, word, CandidateKind::Word);
            commit(&raw, &raw, CandidateKind::Literal);
            let adopted = format!("{word}{gap}");
            for author in AUTHORS[i] {
                commit(
                    &adopted,
                    &format!("{adopted}{}", author.strip_prefix(word).unwrap()),
                    CandidateKind::Sentence,
                );
            }
        }
    }
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for (i, &(reading, word)) in WORDS.iter().take(6).enumerate() {
            let original = format!("{prefix}{}", reading.replace('\'', " "));
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&original);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.snapshot().seed_text, original);
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert!(ime.snapshot().committed_text.is_empty());
            let punctuation = if i == 1 { "？" } else { "，" };
            let (han, pinyin) = if i == 4 { ("和", "he") } else { ("我", "wo") };
            for gap in ["", " ", "\u{3000}"] {
                for tail in [han.to_owned(), pinyin.to_ascii_uppercase()] {
                    let raw = format!("{adopted}{gap}{punctuation}{tail}");
                    let expected = format!(
                        "{adopted}{gap}{}",
                        AUTHORS[i][0].strip_prefix(word).unwrap()
                    );
                    ime.seed(&raw);
                    pool(&ime, &raw);
                    assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
                    index(&ime, &expected, CandidateKind::Sentence);
                    ime.seed(&original);
                    index(&ime, &adopted, CandidateKind::Word);
                    commit(&raw, &expected, CandidateKind::Sentence);
                    commit(&raw, &raw, CandidateKind::Literal);
                }
            }
        }
    }
}

#[test]
fn accessories_preserves_old_complete_choices_and_does_not_cross_protected_boundaries() {
    for (reading, word, authors) in [
        (
            "chong'dian'qi",
            "充电器",
            ["充电器可以借用一下吗？", "充电器放在包里了。"],
        ),
        (
            "chong'dian'xian",
            "充电线",
            ["充电线已经插好了。", "充电线可以借我用一下吗？"],
        ),
    ] {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            for author in authors {
                commit(&seed, author, CandidateKind::Sentence);
            }
        }
    }
    for raw in [
        "zzqvjy",
        "accessories@example.invalid",
        "https://example.invalid/connectors",
        "file_cha_xian_ban",
        "原文 123\t???",
    ] {
        assert!(
            !engine(raw)
                .candidates()
                .iter()
                .any(|c| AUTHORS.iter().flatten().any(|s| c.text.contains(s)))
        );
        commit(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("cha{line}xian ban xian fang zhe bian");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == WORDS[0].1 || AUTHORS[0].contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        for (i, &(_, word)) in WORDS.iter().take(6).enumerate() {
            let raw = format!("{word}{line}，wo");
            let crossed = format!("{word}{line}{}", AUTHORS[i][0].strip_prefix(word).unwrap());
            assert!(
                !engine(&raw)
                    .candidates()
                    .iter()
                    .any(|c| c.text == crossed || c.text == AUTHORS[i][0])
            );
            commit(&raw, &raw, CandidateKind::Literal);
        }
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "accessories-chinese-synthetic-fallback"
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
fn accessories_synthetic_pending_missing_error_timeout_and_empty_models_keep_local_pool() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (i, &seed) in PARTIALS.iter().enumerate() {
            let baseline = engine(seed).candidates().to_vec();
            let mut ime = engine("");
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
            assert_eq!(ime.candidates(), baseline);
            index(&ime, WORDS[i].1, CandidateKind::Word);
            for author in AUTHORS[i] {
                index(&ime, author, CandidateKind::Sentence);
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                assert_eq!(ime.candidates(), baseline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!ime.prediction_pending());
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), baseline);
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            commit_from(&mut ime, AUTHORS[i][0], CandidateKind::Sentence);
        }
    }
}
