//! Fixed household supply reminders, not inventory sensing or shopping actions.
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

const WORDS: &[(&str, &str)] = &[
    ("zhi'jin'zhi'sheng'yi'bao'le", "纸巾只剩一包了"),
    ("xi'shou'ye'kuai'yong'wan'le", "洗手液快用完了"),
    ("jia'li'hai'you'bei'yong'de'ma", "家里还有备用的吗"),
    ("xian'ji'zai'bu'huo'qing'dan'shang", "先记在补货清单上"),
    ("zhe'yi'ping'hai'neng'yong'ji'tian", "这一瓶还能用几天"),
    ("ji'de'bu'yi'bao'la'ji'dai", "记得补一包垃圾袋"),
    ("xi'shou'ye", "洗手液"),
    ("wei'sheng'zhi", "卫生纸"),
    ("juan'zhi", "卷纸"),
    ("shi'jin", "湿巾"),
    ("chu'fang'zhi", "厨房纸"),
    ("bao'xian'mo", "保鲜膜"),
    ("mi'feng'dai", "密封袋"),
    ("bu'chong'zhuang", "补充装"),
    ("bu'huo'qing'dan", "补货清单"),
    ("bei'yong'dian'chi", "备用电池"),
    ("zhi'jin'he", "纸巾盒"),
    ("yu'liang", "余量"),
];
// Only these final-letter omissions, not arbitrary shorter prefixes, are fixed.
const PARTIALS: &[(&str, &str)] = &[
    ("zhi'jin'zhi'sheng'yi'bao'l", "纸巾只剩一包了"),
    ("xi'shou'ye'kuai'yong'wan'l", "洗手液快用完了"),
    ("jia'li'hai'you'bei'yong'de'm", "家里还有备用的吗"),
    ("xian'ji'zai'bu'huo'qing'dan'shan", "先记在补货清单上"),
    ("zhe'yi'ping'hai'neng'yong'ji'tia", "这一瓶还能用几天"),
    ("ji'de'bu'yi'bao'la'ji'da", "记得补一包垃圾袋"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "纸巾只剩一包了",
        [
            "纸巾只剩一包了，我先在清单上记一下。",
            "纸巾只剩一包了，柜子里的已经用完了。",
        ],
    ),
    (
        "洗手液快用完了",
        [
            "洗手液快用完了，我看看还有没有补充装。",
            "洗手液快用完了，先别把空瓶扔掉。",
        ],
    ),
    (
        "家里还有备用的吗",
        [
            "家里还有备用的吗？我想先确认一下。",
            "家里还有备用的吗？有的话先拿出来。",
        ],
    ),
    (
        "先记在补货清单上",
        [
            "先记在补货清单上，我晚点再核对一遍。",
            "先记在补货清单上，免得下次又忘了。",
        ],
    ),
    (
        "这一瓶还能用几天",
        [
            "这一瓶还能用几天？我想估计一下余量。",
            "这一瓶还能用几天？不急的话先用着。",
        ],
    ),
    (
        "记得补一包垃圾袋",
        [
            "记得补一包垃圾袋，原来那包已经用完了。",
            "记得补一包垃圾袋，我先在清单上做个记号。",
        ],
    ),
];

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.into(),
        reading.replace('\'', " "),
        reading.replace('\'', ""),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let at = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if kind == CandidateKind::Word {
        assert!(at < PAGE_SIZE, "Word left page one: {text:?}");
    }
    at
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
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
fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    if !seed.is_empty() {
        draft(&ime, seed);
    }
    ime
}
fn commit(seed: &str, text: &str, kind: CandidateKind) {
    commit_from(&mut engine(seed), text, kind);
}
fn commit_from(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let seed = ime.snapshot().seed_text;
    draft(ime, &seed);
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert_eq!(ime.snapshot().seed_text, seed);
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
fn restock_uses_fixed_3304_reading_and_809_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (18, 6, 6));
    assert!(vocabulary.readings().len() >= 3322);
    for (entry, &(reading, text)) in vocabulary.readings()[3304..3322].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts: Vec<_> = vocabulary.continuations().skip(809).take(6).collect();
    assert_eq!(contexts.len(), 6);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        12
    );
    for ((context, values), &(word, sentences)) in contexts.into_iter().zip(SENTENCES) {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
    }
}

#[test]
fn restock_full_and_specific_partial_spellings_keep_words_authors_and_raw_selectable() {
    for &(reading, word) in WORDS.iter().chain(PARTIALS) {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    commit(&seed, sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn restock_horizontal_spacing_and_unicode_prefixes_preserve_exact_text() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{}{gap}", reading.replace('\'', gap));
            commit(&raw, word, CandidateKind::Word);
            commit(&raw, &raw, CandidateKind::Literal);
            let adopted = format!("{word}{gap}");
            for sentence in sentences {
                commit(
                    &adopted,
                    &format!("{adopted}{}", sentence.strip_prefix(word).unwrap()),
                    CandidateKind::Sentence,
                );
            }
            commit(&adopted, &adopted, CandidateKind::Literal);
        }
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{prefix}{}", reading.replace('\'', " "));
            assert!(
                engine(&raw)
                    .candidates()
                    .iter()
                    .all(|c| c.text.starts_with(&prefix))
            );
            commit(&raw, &format!("{prefix}{word}"), CandidateKind::Word);
            for sentence in sentences {
                commit(
                    &raw,
                    &format!("{prefix}{sentence}"),
                    CandidateKind::Sentence,
                );
            }
            commit(&raw, &raw, CandidateKind::Literal);
        }
    }
}

#[test]
fn restock_adoption_and_known_tail_progress_remain_editable_until_explicit_commit() {
    // Each Pinyin tail has an existing explicit Word mapping; no new single
    // character, stock estimate, or physical input behavior is implied.
    for (context, mark, han, reading, sentence) in [
        (0, "，", "柜子", "gui'zi", SENTENCES[0].1[1]),
        (1, "，", "先", "xian", SENTENCES[1].1[1]),
        (2, "？", "我", "wo", SENTENCES[2].1[0]),
        (3, "，", "我", "wo", SENTENCES[3].1[0]),
        (4, "？", "不", "bu", SENTENCES[4].1[1]),
        (5, "，", "原来", "yuan'lai", SENTENCES[5].1[0]),
    ] {
        let (full, word) = WORDS[context];
        assert!(sentence.starts_with(&format!("{word}{mark}{han}")));
        let mut ime = engine(full);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-style selection/reseed only, not physical IBus key acceptance.
        for gap in ["", " ", "\t", "\u{3000}"] {
            for tail in std::iter::once(han.to_owned()).chain(spellings(reading)) {
                let raw = format!("{word}{gap}{mark}{tail}");
                let expected = format!("{word}{gap}{}", sentence.strip_prefix(word).unwrap());
                ime.seed(&raw);
                draft(&ime, &raw);
                index(&ime, &expected, CandidateKind::Sentence);
                ime.seed(full);
                draft(&ime, full);
                index(&ime, word, CandidateKind::Word);
                commit(&raw, &expected, CandidateKind::Sentence);
                commit(&raw, &raw, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn restock_unknown_protected_tokens_wrong_syllables_and_lines_do_not_borrow_authors() {
    // These complete old controls retain their Words and authored choices;
    // enriching a short prefix is not a promise that its entire pool is unchanged.
    for (reading, word, sentences) in [
        (
            "kuai'yong'wan'le",
            "快用完了",
            [
                "快用完了，下次买东西时记得补上。",
                "快用完了，先看看还有没有备用的。",
            ],
        ),
        (
            "hai'sheng'duo'shao",
            "还剩多少",
            ["还剩多少，我们先数一下。", "还剩多少，不够的话我去补一些。"],
        ),
        (
            "liu'dian'bei'yong",
            "留点备用",
            ["留点备用，别一次全用掉。", "留点备用，放在容易找到的地方。"],
        ),
        (
            "la'ji'dai'man'le",
            "垃圾袋满了",
            [
                "垃圾袋满了，换一个新的吧。",
                "垃圾袋满了，我出门时顺手带下去。",
            ],
        ),
        (
            "xi'shou'jian",
            "洗手间",
            ["洗手间在哪里？", "洗手间在走廊尽头。"],
        ),
        (
            "bei'yong'yao'shi",
            "备用钥匙",
            ["备用钥匙放在抽屉里。", "备用钥匙可以借我一下吗？"],
        ),
    ] {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            for sentence in sentences {
                commit(&seed, sentence, CandidateKind::Sentence);
            }
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    commit("la ji dai", "垃圾袋", CandidateKind::Word);
    for raw in [
        "zzqvjy",
        "restock@example.invalid",
        "https://example.invalid/zhijinzhishengyibaole",
        "file_zhijinzhishengyibaole",
        "“raw” 123\t???",
        "zhi jin zhi she ng yi bao le",
    ] {
        let ime = engine(raw);
        assert!(!ime.candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|s| c.text.contains(s)))
        }));
        commit(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("zhi jin zhi sheng{line}yi bao le");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()) })
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("纸巾只剩一包了{line}，我");
        let crossed = format!("纸巾只剩一包了{line}，我先在清单上记一下。");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == crossed || c.text == SENTENCES[0].1[0] })
        );
        commit(&raw, &raw, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "restock-chinese-synthetic-fallback"
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
fn restock_unavailable_timeout_and_empty_models_preserve_the_entire_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (&(seed, word), &(_, sentences)) in PARTIALS.iter().zip(SENTENCES) {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                index(&ime, sentence, CandidateKind::Sentence);
            }
            let before = ime.candidates().to_vec();
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
            assert_eq!(ime.candidates(), before);
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            draft(&ime, seed);
            commit_from(&mut ime, sentences[0], CandidateKind::Sentence);
        }
    }
}
