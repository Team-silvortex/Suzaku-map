//! Finite entryway and before-leaving dialogue, not personal-location or lock control.
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
    ("xie'gui'li'hai'you'yi'shuang", "鞋柜里还有一双"),
    ("men'dian'pang'bian'xian'liu'kong", "门垫旁边先留空"),
    ("xie'dai'wo'zai'ji'yi'xia", "鞋带我再系一下"),
    ("men'jin'ka'xian'bang'wo'na'zhe", "门禁卡先帮我拿着"),
    ("yao'shi'kou'zai'xiao'he'li", "钥匙扣在小盒里"),
    ("zhe'die'san'xian'fang'san'jia", "折叠伞先放伞架"),
    ("men'jin'ka", "门禁卡"),
    ("zhe'die'san", "折叠伞"),
    ("san'jia", "伞架"),
];
const TONES: &[&str] = &[
    "xie2'gui4'li3'hai2'you3'yi4'shuang1",
    "men2'dian4'pang2'bian1'xian1'liu2'kong4",
    "xie2'dai4'wo3'zai4'ji4'yi2'xia4",
    "men2'jin4'ka3'xian1'bang1'wo3'na2'zhe5",
    "yao4'shi5'kou4'zai4'xiao3'he2'li3",
    "zhe2'die2'san3'xian1'fang4'san3'jia4",
    "men2'jin4'ka3",
    "zhe2'die2'san3",
    "san3'jia4",
];
const PARTIALS: &[&str] = &[
    "xie'gui'li'hai'you'yi'shuan",
    "men'dian'pang'bian'xian'liu'kon",
    "xie'dai'wo'zai'ji'yi'xi",
    "men'jin'ka'xian'bang'wo'na'zh",
    "yao'shi'kou'zai'xiao'he'l",
    "zhe'die'san'xian'fang'san'ji",
];
const AUTHORS: [[&str; 2]; 6] = [
    [
        "鞋柜里还有一双，我帮你拿出来。",
        "鞋柜里还有一双，放在下面那层。",
    ],
    [
        "门垫旁边先留空，袋子我来挪开。",
        "门垫旁边先留空，等我把鞋放好。",
    ],
    [
        "鞋带我再系一下，你先把东西拿好。",
        "鞋带我再系一下，系好就一起走。",
    ],
    [
        "门禁卡先帮我拿着，我先把鞋穿好。",
        "门禁卡先帮我拿着，等会儿再递给我。",
    ],
    [
        "钥匙扣在小盒里，和备用钥匙放一起。",
        "钥匙扣在小盒里，你打开看看就能找到。",
    ],
    [
        "折叠伞先放伞架，我把袋子收起来。",
        "折叠伞先放伞架，出门时再一起拿。",
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
fn entryway_preserves_3432_readings_873_contexts_flags_order_and_exact_resource_bytes() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert!(zh.readings().len() >= 3432 && zh.continuations().count() >= 873);
    assert_eq!(
        zh.continuations()
            .take(873)
            .map(|(_, v)| v.len())
            .sum::<usize>(),
        1744
    );
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if zh.readings().len() > 3432 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"xie'gui'li'hai'you'yi'shuang\", \"text\": \"鞋柜里还有一双\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"鞋柜里还有一双\",", "\n  ]\n}"),
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
        "23f481f15542d626f6dde4e1948f230e9f40c213200adc5a58c1e429932dd026"
    );
    for &(_, word) in WORDS {
        assert!(!zh.readings()[..3432].iter().any(|e| e.text == word));
    }
}

#[test]
fn entryway_adds_nine_words_six_contexts_and_twelve_authors_without_duplicating_old_nouns() {
    let zh = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(
        (zh.readings().len(), zh.continuations().count()),
        (3442, 906)
    );
    assert_eq!(
        zh.continuations().map(|(_, vs)| vs.len()).sum::<usize>(),
        1810
    );
    assert_eq!((WORDS.len(), TONES.len(), PARTIALS.len()), (9, 9, 6));
    for (e, &(reading, text)) in zh.readings()[3432..3441].iter().zip(WORDS) {
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
    for (i, (key, vs)) in zh.continuations().skip(873).take(6).enumerate() {
        assert_eq!(key, WORDS[i].1);
        assert_eq!(
            vs.iter().map(String::as_str).collect::<Vec<_>>(),
            AUTHORS[i]
        );
    }
    for text in ["鞋柜", "门垫", "鞋带", "钥匙扣", "雨伞"] {
        assert_eq!(zh.readings().iter().filter(|e| e.text == text).count(), 1);
    }
}

#[test]
fn entryway_specific_full_and_partial_pinyin_offer_words_authors_and_literal_commits() {
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
fn entryway_horizontal_spaces_adopted_tails_and_long_unicode_drafts_are_lossless() {
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
            let punctuation = "，";
            let (han, pinyin) = [
                ("我", "wo"),
                ("袋", "dai"),
                ("你", "ni"),
                ("我", "wo"),
                ("和", "he"),
                ("我", "wo"),
            ][i];
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
fn entryway_preserves_old_complete_choices_and_does_not_cross_protected_boundaries() {
    // Preserve the historical xi spelling as well as the new phrase's ji reading.
    for (reading, word, authors) in [
        (
            "wang'dai'yu'san",
            "忘带雨伞",
            ["忘带雨伞了，可以借我一把吗？", "忘带雨伞了，我回去拿一下。"],
        ),
        (
            "xie'dai'xi'hao'le",
            "鞋带系好了",
            [
                "鞋带系好了，现在可以出门了。",
                "鞋带系好了，我们慢慢走过去。",
            ],
        ),
        (
            "xie'dai'you'song'le",
            "鞋带又松了",
            ["鞋带又松了，我先重新系一下。", "鞋带又松了，等我一下再走。"],
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
        "entryway@example.invalid",
        "https://example.invalid/keycard",
        "file_men_jin_ka",
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
        let raw = format!("xie{line}gui li hai you yi shuang");
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
        "entryway-chinese-synthetic-fallback"
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
fn entryway_synthetic_pending_missing_error_timeout_and_empty_models_keep_local_pool() {
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
