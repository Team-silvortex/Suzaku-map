//! Frozen photo/file-sharing fallback expectations; no network or real input events.
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
    ("zhao'pian'fa'yuan'tu'ba", "照片发原图吧"),
    ("zhe'zhang'zhao'pian'you'dian'hu", "这张照片有点糊"),
    ("jie'tu'fa'gei'wo'kan'kan", "截图发给我看看"),
    ("wen'jian'ming'gai'yi'xia", "文件名改一下"),
    ("fu'jian'wo'chong'xin'fa'le", "附件我重新发了"),
    ("zhe'ge'ban'ben'shi'zui'xin'de", "这个版本是最新的"),
    ("wen'dang'li'xie'qing'chu'le", "文档里写清楚了"),
    ("shou'dao'hou'hui'wo'yi'xia", "收到后回我一下"),
    ("yuan'tu", "原图"),
    ("ya'suo'tu", "压缩图"),
    ("tu'xiang'bi'li", "图像比例"),
    ("gao'qing'tu", "高清图"),
    ("qing'xi'du", "清晰度"),
    ("tu'pian'chi'cun", "图片尺寸"),
    ("heng'ping", "横屏"),
    ("shu'ping", "竖屏"),
    ("pai'she'shi'jian", "拍摄时间"),
    ("xiang'ce'feng'mian", "相册封面"),
    ("wen'jian'yu'lan", "文件预览"),
    ("pi'liang'fa'song", "批量发送"),
    ("fa'song'ji'lu", "发送记录"),
    ("gong'xiang'xiang'ce", "共享相册"),
    ("wen'dang'fu'ben", "文档副本"),
    ("xiu'gai'ji'lu", "修改记录"),
];

// Only these near-complete expressions are promised; short ambiguous prefixes
// retain the existing dictionary and homophone priorities.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("zhao'pian'fa'yuan'tu'b", "照片发原图吧"),
    ("zhe'zhang'zhao'pian'you'dian'h", "这张照片有点糊"),
    ("jie'tu'fa'gei'wo'kan'ka", "截图发给我看看"),
    ("wen'jian'ming'gai'yi'xi", "文件名改一下"),
    ("fu'jian'wo'chong'xin'fa'l", "附件我重新发了"),
    ("zhe'ge'ban'ben'shi'zui'xin'd", "这个版本是最新的"),
    ("wen'dang'li'xie'qing'chu'l", "文档里写清楚了"),
    ("shou'dao'hou'hui'wo'yi'xi", "收到后回我一下"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "照片发原图吧",
        [
            "照片发原图吧，我想看清楚细节。",
            "照片发原图吧，压缩后看不清。",
        ],
    ),
    (
        "这张照片有点糊",
        [
            "这张照片有点糊，可以再拍一张吗？",
            "这张照片有点糊，我找找有没有清楚的。",
        ],
    ),
    (
        "截图发给我看看",
        [
            "截图发给我看看，我确认一下内容。",
            "截图发给我看看，记得把完整提示截上。",
        ],
    ),
    (
        "文件名改一下",
        [
            "文件名改一下，方便大家查找。",
            "文件名改一下，把日期也加上吧。",
        ],
    ),
    (
        "附件我重新发了",
        [
            "附件我重新发了，这次应该完整了。",
            "附件我重新发了，你看看能不能打开。",
        ],
    ),
    (
        "这个版本是最新的",
        [
            "这个版本是最新的，后面都用这一份吧。",
            "这个版本是最新的，旧文件可以先留着。",
        ],
    ),
    (
        "文档里写清楚了",
        [
            "文档里写清楚了，你可以先看一下。",
            "文档里写清楚了，有疑问我们再讨论。",
        ],
    ),
    (
        "收到后回我一下",
        [
            "收到后回我一下，我好确认有没有漏发。",
            "收到后回我一下，不着急，方便时回复就好。",
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
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates().iter().any(|candidate| {
            candidate.text == seed && candidate.kind == CandidateKind::Literal
        })
    );
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
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if kind == CandidateKind::Word {
        assert!(index < PAGE_SIZE);
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
fn sharing_uses_fixed_3090_reading_and_743_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 8);
    assert!(vocabulary.readings().len() >= 3114);
    for (entry, &(reading, word)) in vocabulary.readings()[3090..3114].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(743)
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
fn sharing_full_and_specific_partial_inputs_keep_words_sentences_and_raw_submission() {
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
fn sharing_horizontal_spacing_preserves_raw_and_adopted_drafts() {
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
            let raw = format!("{}{separator}", reading.replace('\'', separator));
            let mut ime = engine(&raw);
            draft(&ime, &raw);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&raw), &raw, CandidateKind::Literal);

            let adopted = format!("{word}{separator}");
            let ime = engine(&adopted);
            draft(&ime, &adopted);
            for sentence in sentences {
                let expected = format!("{adopted}{}", sentence.strip_prefix(word).unwrap());
                commit_once_and_undo(&mut engine(&adopted), &expected, CandidateKind::Sentence);
            }
            commit_once_and_undo(&mut engine(&adopted), &adopted, CandidateKind::Literal);
        }
    }
}

#[test]
fn sharing_adoption_sentence_progress_and_reseeding_remain_editable_until_commit() {
    for (word, tail_reading, tail_word, sentence) in [
        ("照片发原图吧", "wo", "我", SENTENCES[0].1[0]),
        ("这张照片有点糊", "ke'yi", "可以", SENTENCES[1].1[0]),
        ("截图发给我看看", "wo", "我", SENTENCES[2].1[0]),
        ("附件我重新发了", "ni", "你", SENTENCES[4].1[1]),
        ("文档里写清楚了", "you", "有", SENTENCES[6].1[1]),
        ("收到后回我一下", "wo", "我", SENTENCES[7].1[0]),
    ] {
        let reading = WORDS.iter().find(|(_, text)| *text == word).unwrap().0;
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Core host-style reseeding, not physical keyboard dispatch or focus evidence.
        for padding in ["", " ", "\t", "\u{3000}"] {
            for tail in std::iter::once(tail_word.to_owned()).chain(spellings(tail_reading)) {
                let seed = format!("{word}{padding}，{tail}");
                let expected = format!("{word}{padding}{}", sentence.strip_prefix(word).unwrap());
                ime.seed(&seed);
                draft(&ime, &seed);
                index(&ime, &expected, CandidateKind::Sentence);
                ime.seed(reading);
                draft(&ime, reading);
                index(&ime, word, CandidateKind::Word);
                commit_once_and_undo(&mut engine(&seed), &expected, CandidateKind::Sentence);
                commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn sharing_urls_unknown_runs_and_line_boundaries_preserve_literal_text() {
    for seed in [
        "zzqvjy",
        "sharing@example.invalid",
        "https://example.invalid/zhaopianfayuantuba",
        "“raw” 123\t???",
        "zhao pi an fa yuan tu ba",
        "fu jia n wo chong xin fa le",
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(!ime.candidates().iter().any(|candidate| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|text| candidate.text.contains(text)))
        }));
        commit_once_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("照片发原图吧{boundary}，wo");
        let crossed = format!("照片发原图吧{boundary}，我想看清楚细节。");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| candidate.text == crossed)
        );
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FallbackProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        "sharing-chinese-synthetic-fallback"
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
fn sharing_model_unavailable_timeout_and_empty_results_preserve_local_choices() {
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
