//! Authored household coordination, not location-specific waste-disposal advice.
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
    ("zhi'xiang'xian'ya'ping", "纸箱先压平"),
    ("kong'ping'fang'zai'zhe'bian", "空瓶放在这边"),
    ("zhe'xie'bao'zhuang'xian'bie'reng", "这些包装先别扔"),
    ("dai'kou'xian'ji'hao", "袋口先系好"),
    ("hui'shou'ri'shi'na'tian", "回收日是哪天"),
    ("zhe'dai'la'ji'wo'lai'ti", "这袋垃圾我来提"),
    ("hui'shou'xiang", "回收箱"),
    ("kong'ping", "空瓶"),
    ("zhi'he", "纸盒"),
    ("yi'la'guan", "易拉罐"),
    ("fei'zhi", "废纸"),
    ("hui'shou'ri", "回收日"),
];
const TONES: &[&str] = &[
    "zhi3'xiang1'xian1'ya1'ping2",
    "kong1'ping2'fang4'zai4'zhe4'bian1",
    "zhe4'xie1'bao1'zhuang1'xian1'bie2'reng1",
    "dai4'kou3'xian1'ji4'hao3",
    "hui2'shou1'ri4'shi4'na3'tian1",
    "zhe4'dai4'la1'ji1'wo3'lai2'ti2",
    "hui2'shou1'xiang1",
    "kong1'ping2",
    "zhi3'he2",
    "yi4'la1'guan4",
    "fei4'zhi3",
    "hui2'shou1'ri4",
];
const PARTIALS: &[&str] = &[
    "zhi'xiang'xian'ya'pi",
    "kong'ping'fang'zai'zhe'bia",
    "zhe'xie'bao'zhuang'xian'bie're",
    "dai'kou'xian'ji'ha",
    "hui'shou'ri'shi'na'tia",
    "zhe'dai'la'ji'wo'lai't",
];
const AUTHORS: [[&str; 2]; 6] = [
    [
        "纸箱先压平，我来把它们放好。",
        "纸箱先压平，等会儿再一起搬下去。",
    ],
    [
        "空瓶放在这边，我先找个袋子。",
        "空瓶放在这边，别和还没喝完的放一起。",
    ],
    [
        "这些包装先别扔，我再确认一下。",
        "这些包装先别扔，里面可能还有配件。",
    ],
    ["袋口先系好，我来拿另一袋。", "袋口先系好，放在门边等我。"],
    [
        "回收日是哪天？我先记下来。",
        "回收日是哪天？我们再看一下通知。",
    ],
    [
        "这袋垃圾我来提，你帮我开一下门。",
        "这袋垃圾我来提，另外那袋先放着。",
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
fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn recycling_preserves_all_3410_old_readings_861_contexts_and_exact_resource_bytes() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(
        fnv(zh.readings().iter().take(3410).flat_map(|e| {
            let kind = match e.kind {
                EntryKind::Word => "word",
                EntryKind::Sentence => "sentence",
            };
            e.reading
                .bytes()
                .chain([0])
                .chain(e.text.bytes())
                .chain([0])
                .chain(kind.bytes())
                .chain([u8::from(e.require_separators)])
        })),
        0x1b69da7b02e5c691
    );
    assert_eq!(
        fnv(zh.continuations().take(861).flat_map(|(k, vs)| {
            std::iter::once(k)
                .chain(vs.iter().map(String::as_str))
                .flat_map(|s| s.bytes().chain([0]))
                .chain([0xff])
        })),
        0x953465185f929ee8
    );
    assert_eq!(
        zh.continuations()
            .take(861)
            .map(|(_, v)| v.len())
            .sum::<usize>(),
        1720
    );
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if zh.readings().len() > 3410 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"zhi'xiang'xian'ya'ping\", \"text\": \"纸箱先压平\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"纸箱先压平\",", "\n  ]\n}"),
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
        "f8ea73423ac9040d14c946a7f09fd6e96e98c569737a266227a06f7572fad553"
    );
}

#[test]
fn recycling_adds_exactly_twelve_words_six_contexts_and_twelve_authors() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(
        (zh.readings().len(), zh.continuations().count()),
        (3442, 906)
    );
    assert_eq!(
        zh.continuations().map(|(_, vs)| vs.len()).sum::<usize>(),
        1810
    );
    assert_eq!((WORDS.len(), TONES.len(), PARTIALS.len()), (12, 12, 6));
    for (e, &(reading, text)) in zh.readings()[3410..3422].iter().zip(WORDS) {
        assert_eq!(
            (&*e.reading, &*e.text, e.kind, e.require_separators),
            (reading, text, EntryKind::Word, false)
        );
    }
    for (i, (key, vs)) in zh.continuations().skip(861).take(6).enumerate() {
        assert_eq!(key, WORDS[i].1);
        assert_eq!(
            vs.iter().map(String::as_str).collect::<Vec<_>>(),
            AUTHORS[i]
        );
    }
}

#[test]
fn recycling_specific_full_and_partial_pinyin_offer_words_authors_and_literal_commits() {
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
fn recycling_horizontal_spaces_adopted_tails_and_long_unicode_drafts_are_lossless() {
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
            let punctuation = if i == 4 { "？" } else { "，" };
            let (han, pinyin) = if i == 5 { ("你", "ni") } else { ("我", "wo") };
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
fn recycling_preserves_old_complete_choices_and_does_not_cross_protected_boundaries() {
    for (reading, word, authors) in [
        (
            "dao'la'ji",
            "倒垃圾",
            ["倒垃圾的时候顺便带上这个袋子。", "倒垃圾的时间到了。"],
        ),
        (
            "la'ji'dai'man'le",
            "垃圾袋满了",
            [
                "垃圾袋满了，换一个新的吧。",
                "垃圾袋满了，我出门时顺手带下去。",
            ],
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
        "recycling@example.invalid",
        "https://example.invalid/cartons",
        "file_hui_shou_ri",
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
        let raw = format!("zhi{line}xiang xian ya ping");
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
        "recycling-chinese-synthetic-fallback"
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
fn recycling_synthetic_pending_missing_error_timeout_and_empty_models_keep_local_pool() {
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
