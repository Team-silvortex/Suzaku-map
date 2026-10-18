//! Fixed after-sales messages, proof of purchase and packaging vocabulary.
//! These are authored requests, not store policies or a live returns service.
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

// Six explicit context triggers followed by twelve related Words. 包装 is an
// intentional new exact Word, not a promise to leave its old prefix pool intact.
const WORDS: &[(&str, &str)] = &[
    ("zhe'ge'shou'dao'shi'jiu'huai'le", "这个收到时就坏了"),
    ("wo'xiang'huan'tong'kuan'de", "我想换同款的"),
    ("yuan'bao'zhuang'wo'hai'liu'zhe", "原包装我还留着"),
    ("gou'mai'ping'zheng'zen'me'ti'gong", "购买凭证怎么提供"),
    ("ji'hui'qu'xu'yao'shen'me", "寄回去需要什么"),
    ("shou'hou'jin'du'bang'wo'cha'yi'xia", "售后进度帮我查一下"),
    ("shou'hou'dan'hao", "售后单号"),
    ("wen'ti'zhao'pian", "问题照片"),
    ("bao'zhuang", "包装"),
    ("huan'huo'shuo'ming", "换货说明"),
    ("yuan'bao'zhuang", "原包装"),
    ("pei'jian'qing'dan", "配件清单"),
    ("gou'mai'ping'zheng", "购买凭证"),
    ("ding'dan'jie'tu", "订单截图"),
    ("ji'hui'di'zhi", "寄回地址"),
    ("tui'jian'dan'hao", "退件单号"),
    ("chu'li'jin'du", "处理进度"),
    ("bu'fa'pei'jian", "补发配件"),
];

// Only the final ASCII letter of each complete trigger is omitted. No shorter
// ambiguous prefix or arbitrary composed branch is included in this contract.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("zhe'ge'shou'dao'shi'jiu'huai'l", "这个收到时就坏了"),
    ("wo'xiang'huan'tong'kuan'd", "我想换同款的"),
    ("yuan'bao'zhuang'wo'hai'liu'zh", "原包装我还留着"),
    ("gou'mai'ping'zheng'zen'me'ti'gon", "购买凭证怎么提供"),
    ("ji'hui'qu'xu'yao'shen'm", "寄回去需要什么"),
    ("shou'hou'jin'du'bang'wo'cha'yi'xi", "售后进度帮我查一下"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "这个收到时就坏了",
        [
            "这个收到时就坏了，我把照片发给你看。",
            "这个收到时就坏了，麻烦帮我核实一下。",
        ],
    ),
    (
        "我想换同款的",
        [
            "我想换同款的，请问需要怎么处理？",
            "我想换同款的，先帮我看看还有没有货。",
        ],
    ),
    (
        "原包装我还留着",
        [
            "原包装我还留着，需要一起寄回吗？",
            "原包装我还留着，里面的配件也都在。",
        ],
    ),
    (
        "购买凭证怎么提供",
        [
            "购买凭证怎么提供？我可以发订单截图吗？",
            "购买凭证怎么提供？需要把小票拍清楚吗？",
        ],
    ),
    (
        "寄回去需要什么",
        [
            "寄回去需要什么？麻烦发一份清单给我。",
            "寄回去需要什么？地址也请再确认一下。",
        ],
    ),
    (
        "售后进度帮我查一下",
        [
            "售后进度帮我查一下，我想确认现在到哪一步了。",
            "售后进度帮我查一下，有结果再告诉我就好。",
        ],
    ),
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

fn spellings(reading: &str) -> [String; 4] {
    [
        reading.replace('\'', ""),
        reading.into(),
        reading.replace('\'', " "),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local)
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
    index(ime, seed, CandidateKind::Literal);
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if kind == CandidateKind::Word {
        assert!(index < PAGE_SIZE, "word left page one: {text:?}");
    }
    index
}

fn commit_once_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let committed = ime.commit(CommitOptions { force: true });
    assert!(committed.ok);
    assert_eq!(committed.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn aftercare_uses_fixed_3226_reading_and_783_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 18);
    assert_eq!(SPECIFIC_PARTIALS.len(), 6);
    assert_eq!(SENTENCES.len(), 6);
    assert!(vocabulary.readings().len() >= 3244);
    for (entry, &(reading, word)) in vocabulary.readings()[3226..3244].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    assert_eq!(WORDS[8], ("bao'zhuang", "包装"));
    let contexts = vocabulary
        .continuations()
        .skip(783)
        .take(6)
        .collect::<Vec<_>>();
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
fn aftercare_full_and_specific_partial_spellings_keep_words_sentences_and_raw() {
    for &(reading, word) in WORDS.iter().chain(SPECIFIC_PARTIALS) {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            if let Some((_, sentences)) = SENTENCES.iter().find(|(context, _)| *context == word) {
                for sentence in sentences {
                    commit_once_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
    for seed in spellings("bao'zhuang") {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert_eq!(index(&ime, "包装", CandidateKind::Word), 0);
        commit_once_and_undo(&mut ime, "包装", CandidateKind::Word);
    }
}

#[test]
fn aftercare_horizontal_spacing_keeps_raw_and_adopted_han_text_exact() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{}{separator}", reading.replace('\'', separator));
            let mut ime = engine(&raw);
            draft(&ime, &raw);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&raw), &raw, CandidateKind::Literal);
            let adopted = format!("{word}{separator}");
            draft(&engine(&adopted), &adopted);
            for sentence in sentences {
                let expected = format!("{adopted}{}", sentence.strip_prefix(word).unwrap());
                commit_once_and_undo(&mut engine(&adopted), &expected, CandidateKind::Sentence);
            }
            commit_once_and_undo(&mut engine(&adopted), &adopted, CandidateKind::Literal);
        }
    }
}

#[test]
fn aftercare_adoption_reseed_and_unicode_prefixes_stay_editable_until_commit() {
    for (context, punctuation, reading_tail, han_tail, sentence) in [
        (0, "，", "wo", "我", SENTENCES[0].1[0]),
        (1, "，", "xian", "先", SENTENCES[1].1[1]),
        (2, "，", "xu'yao", "需要", SENTENCES[2].1[0]),
        (3, "？", "wo", "我", SENTENCES[3].1[0]),
        (4, "？", "ma'fan", "麻烦", SENTENCES[4].1[0]),
        (5, "，", "wo", "我", SENTENCES[5].1[0]),
    ] {
        let (reading, word) = WORDS[context];
        assert!(sentence.starts_with(&format!("{word}{punctuation}{han_tail}")));
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-style selection/reseed only, not physical IBus key acceptance.
        for padding in ["", " ", "\t", "\u{3000}"] {
            for tail in std::iter::once(han_tail.to_owned()).chain(spellings(reading_tail)) {
                let raw = format!("{word}{padding}{punctuation}{tail}");
                let expected = format!("{word}{padding}{}", sentence.strip_prefix(word).unwrap());
                ime.seed(&raw);
                draft(&ime, &raw);
                index(&ime, &expected, CandidateKind::Sentence);
                ime.seed(reading);
                draft(&ime, reading);
                index(&ime, word, CandidateKind::Word);
                commit_once_and_undo(&mut engine(&raw), &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&raw), &raw, CandidateKind::Literal);
            }
        }
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{prefix}{}", reading.replace('\'', " "));
            let ime = engine(&raw);
            draft(&ime, &raw);
            assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
            commit_once_and_undo(
                &mut engine(&raw),
                &format!("{prefix}{word}"),
                CandidateKind::Word,
            );
            for sentence in sentences {
                commit_once_and_undo(
                    &mut engine(&raw),
                    &format!("{prefix}{sentence}"),
                    CandidateKind::Sentence,
                );
            }
            commit_once_and_undo(&mut engine(&raw), &raw, CandidateKind::Literal);
        }
    }
}

#[test]
fn aftercare_boundaries_keep_literals_and_existing_return_sentences_on_page_one() {
    for raw in [
        "zzqvjy",
        "aftercare@example.invalid",
        "https://example.invalid/zhegeshoudaoshijiuhuaile",
        "file_zhegeshoudaoshijiuhuaile",
        "“raw” 123\t???",
        "zhe ge shou da o shi jiu huai le",
        "gou ma i ping zheng zen me ti gong",
    ] {
        let mut ime = engine(raw);
        draft(&ime, raw);
        assert!(!ime.candidates().iter().any(|candidate| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|text| candidate.text.contains(text)))
        }));
        commit_once_and_undo(&mut ime, raw, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("zhe ge shou dao{boundary}shi jiu huai le");
        let mut ime = engine(&raw);
        draft(&ime, &raw);
        assert!(!ime.candidates().iter().any(|candidate| {
            candidate.text == WORDS[0].1 || SENTENCES[0].1.contains(&candidate.text.as_str())
        }));
        commit_once_and_undo(&mut ime, &raw, CandidateKind::Literal);
        let raw = format!("这个收到时就坏了{boundary}，wo");
        let crossed = format!("这个收到时就坏了{boundary}，我把照片发给你看。");
        let mut ime = engine(&raw);
        draft(&ime, &raw);
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| candidate.text == crossed || candidate.text == SENTENCES[0].1[0])
        );
        commit_once_and_undo(&mut ime, &raw, CandidateKind::Literal);
    }
    // This unrelated old whole Word and its authors remain protected. 包装 is
    // deliberately not frozen as an unchanged pool: its exact Word is new.
    for raw in spellings("tui'huo") {
        let ime = engine(&raw);
        draft(&ime, &raw);
        assert_eq!(index(&ime, "退货", CandidateKind::Word), 0);
        for sentence in ["退货需要保留包装吗？", "退货申请已经提交了。"] {
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once_and_undo(&mut engine(&raw), sentence, CandidateKind::Sentence);
        }
        commit_once_and_undo(&mut engine(&raw), &raw, CandidateKind::Literal);
    }
}

struct FallbackProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        "aftercare-chinese-synthetic-fallback"
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
fn aftercare_unavailable_timeout_and_empty_models_keep_the_complete_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (&(seed, word), &(_, sentences)) in SPECIFIC_PARTIALS.iter().zip(SENTENCES) {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FallbackProvider(error.clone()))));
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
            commit_once_and_undo(&mut ime, sentences[0], CandidateKind::Sentence);
        }
    }
}
