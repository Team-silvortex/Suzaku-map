//! Frozen everyday asynchronous messages and arrangement-confirmation fallback.
//! Expectations are authored before data, without models or native input events.
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

// The first eight are exact context triggers; the remaining sixteen are related
// words, two per context. No new independent character or inferred syllable.
const WORDS: &[(&str, &str)] = &[
    ("gang'kan'dao'ni'de'xiao'xi", "刚看到你的消息"),
    ("xiao'xi'wo'shou'dao'le", "消息我收到了"),
    ("ni'ba'zhong'dian'fa'gei'wo", "你把重点发给我"),
    ("fang'bian'shi'hui'wo'jiu'hao", "方便时回我就好"),
    ("wo'men'gai'dao'xia'wu'ba", "我们改到下午吧"),
    ("wo'xian'he'da'jia'que'ren", "我先和大家确认"),
    ("wo'ba'yi'jian'zheng'li'yi'xia", "我把意见整理一下"),
    ("you'xin'jin'zhan'zai'lian'xi", "有新进展再联系"),
    ("lou'kan'xiao'xi", "漏看消息"),
    ("wei'du'xiao'xi", "未读消息"),
    ("xiao'xi'ti'xing", "消息提醒"),
    ("yi'du'hui'zhi", "已读回执"),
    ("guan'jian'xin'xi", "关键信息"),
    ("zhong'dian'shuo'ming", "重点说明"),
    ("bu'ji'zhe'hui'fu", "不急着回复"),
    ("kong'le'zai'hui'wo", "空了再回我"),
    ("gai'yue'shi'jian", "改约时间"),
    ("ti'qian'gao'zhi", "提前告知"),
    ("he'dui'shi'jian", "核对时间"),
    ("que'ren'an'pai", "确认安排"),
    ("xiu'gai'yi'jian", "修改意见"),
    ("fan'kui'hui'zong", "反馈汇总"),
    ("zui'xin'jin'zhan", "最新进展"),
    ("hou'xu'an'pai", "后续安排"),
];

// Only these eight explicitly near-complete inputs are promised, not arbitrary
// shorter prefixes or every ambiguous branch within the same twelve slots.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("gang'kan'dao'ni'de'xiao'x", "刚看到你的消息"),
    ("xiao'xi'wo'shou'dao'l", "消息我收到了"),
    ("ni'ba'zhong'dian'fa'gei'w", "你把重点发给我"),
    ("fang'bian'shi'hui'wo'jiu'ha", "方便时回我就好"),
    ("wo'men'gai'dao'xia'wu'b", "我们改到下午吧"),
    ("wo'xian'he'da'jia'que're", "我先和大家确认"),
    ("wo'ba'yi'jian'zheng'li'yi'xi", "我把意见整理一下"),
    ("you'xin'jin'zhan'zai'lian'x", "有新进展再联系"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "刚看到你的消息",
        [
            "刚看到你的消息，刚才在忙没顾上看。",
            "刚看到你的消息，我先看一下再回复你。",
        ],
    ),
    (
        "消息我收到了",
        [
            "消息我收到了，你不用再重复发了。",
            "消息我收到了，等我确认好细节就回复。",
        ],
    ),
    (
        "你把重点发给我",
        [
            "你把重点发给我，我晚点一起看。",
            "你把重点发给我，不确定的地方我再问你。",
        ],
    ),
    (
        "方便时回我就好",
        [
            "方便时回我就好，这件事不着急。",
            "方便时回我就好，先忙你手头的事。",
        ],
    ),
    (
        "我们改到下午吧",
        [
            "我们改到下午吧，上午的时间有点赶。",
            "我们改到下午吧，你看看几点合适。",
        ],
    ),
    (
        "我先和大家确认",
        [
            "我先和大家确认，再给你一个准信。",
            "我先和大家确认，免得时间安排冲突。",
        ],
    ),
    (
        "我把意见整理一下",
        [
            "我把意见整理一下，晚点一起发给你。",
            "我把意见整理一下，再说哪些地方需要改。",
        ],
    ),
    (
        "有新进展再联系",
        [
            "有新进展再联系，现在先按这个安排。",
            "有新进展再联系，有变化我会告诉你。",
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
fn messaging_uses_fixed_3178_reading_and_767_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 8);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 3202);
    for (entry, &(reading, word)) in vocabulary.readings()[3178..3202].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(767)
        .take(8)
        .collect::<Vec<_>>();
    assert_eq!(contexts.len(), 8);
    assert_eq!(
        contexts
            .iter()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        16
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
fn messaging_full_and_specific_partial_spellings_keep_words_both_sentences_and_raw() {
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
}

#[test]
fn messaging_horizontal_spacing_keeps_raw_and_selected_han_text_exact() {
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
fn messaging_adoption_reseed_and_unicode_prefixes_remain_editable_until_commit() {
    for (context, tail_reading, tail_word, sentence) in [
        (0, "wo", "我", SENTENCES[0].1[1]),
        (1, "ni", "你", SENTENCES[1].1[0]),
        (2, "wo", "我", SENTENCES[2].1[0]),
        (3, "xian", "先", SENTENCES[3].1[1]),
        (4, "ni", "你", SENTENCES[4].1[1]),
        (5, "zai", "再", SENTENCES[5].1[0]),
        (6, "wan'dian", "晚点", SENTENCES[6].1[0]),
        (7, "xian'zai", "现在", SENTENCES[7].1[0]),
    ] {
        let (reading, word) = WORDS[context];
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Engine-level adoption/reseed only, not physical-key or panel acceptance.
        for padding in ["", " ", "\t", "\u{3000}"] {
            for tail in std::iter::once(tail_word.to_owned()).chain(spellings(tail_reading)) {
                let raw = format!("{word}{padding}，{tail}");
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
fn messaging_unknown_tokens_wrong_splits_and_seven_lines_never_borrow_contexts() {
    for raw in [
        "zzqvjy",
        "messaging@example.invalid",
        "https://example.invalid/gangkandaonidexiaoxi",
        "file_gangkandaonidexiaoxi",
        "“raw” 123\t???",
        "gang kan da o ni de xiao xi",
        "wo xian he da ji a que ren",
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
        let raw = format!("gang{boundary}kan dao ni de xiao xi");
        let mut ime = engine(&raw);
        draft(&ime, &raw);
        assert!(!ime.candidates().iter().any(|candidate| {
            candidate.text == WORDS[0].1 || SENTENCES[0].1.contains(&candidate.text.as_str())
        }));
        commit_once_and_undo(&mut ime, &raw, CandidateKind::Literal);
        let raw = format!("刚看到你的消息{boundary}，wo");
        let crossed = format!("刚看到你的消息{boundary}，我先看一下再回复你。");
        let mut ime = engine(&raw);
        draft(&ime, &raw);
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| candidate.text == crossed || candidate.text == SENTENCES[0].1[1])
        );
        commit_once_and_undo(&mut ime, &raw, CandidateKind::Literal);
    }
}

struct FallbackProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        "messaging-chinese-synthetic-fallback"
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
fn messaging_unavailable_timeout_and_empty_models_preserve_the_complete_local_pool() {
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
