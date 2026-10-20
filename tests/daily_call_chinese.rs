//! Fixed video/audio recovery wording, not device or call controls.
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
    ("ni'de'hua'mian'ka'zhu'le", "你的画面卡住了"),
    ("wo'zhe'bian'kan'bu'dao'hua'mian", "我这边看不到画面"),
    ("wo'huan'fu'er'ji'shi'shi", "我换副耳机试试"),
    ("wo'tui'chu'chong'jin'yi'xia", "我退出重进一下"),
    ("neng'da'kai'she'xiang'tou'ma", "能打开摄像头吗"),
    ("xian'gai'yong'wen'zi'liao", "先改用文字聊"),
    ("hua'mian", "画面"),
    ("sheng'hua'bu'tong'bu", "声画不同步"),
    ("hua'mian'yan'chi", "画面延迟"),
    ("shi'pin'ka'dun", "视频卡顿"),
    ("chong'xin'jia'ru", "重新加入"),
    ("tui'chu'tong'hua", "退出通话"),
    ("guan'bi'she'xiang'tou", "关闭摄像头"),
    ("da'kai'she'xiang'tou", "打开摄像头"),
    ("qie'huan'jing'tou", "切换镜头"),
    ("mian'ti", "免提"),
    ("ting'tong", "听筒"),
    ("chong'bo", "重拨"),
];
// Only these final-letter omissions, not arbitrary shorter prefixes, are fixed.
const PARTIALS: &[(&str, &str)] = &[
    ("ni'de'hua'mian'ka'zhu'l", "你的画面卡住了"),
    ("wo'zhe'bian'kan'bu'dao'hua'mia", "我这边看不到画面"),
    ("wo'huan'fu'er'ji'shi'sh", "我换副耳机试试"),
    ("wo'tui'chu'chong'jin'yi'xi", "我退出重进一下"),
    ("neng'da'kai'she'xiang'tou'm", "能打开摄像头吗"),
    ("xian'gai'yong'wen'zi'lia", "先改用文字聊"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "你的画面卡住了",
        [
            "你的画面卡住了，但声音还听得见。",
            "你的画面卡住了，我先等一会儿。",
        ],
    ),
    (
        "我这边看不到画面",
        [
            "我这边看不到画面，只能听到声音。",
            "我这边看不到画面，你能看到我吗？",
        ],
    ),
    (
        "我换副耳机试试",
        ["我换副耳机试试，你先别挂。", "我换副耳机试试，接好后再说。"],
    ),
    (
        "我退出重进一下",
        [
            "我退出重进一下，你稍等一会儿。",
            "我退出重进一下，回来后再继续聊。",
        ],
    ),
    (
        "能打开摄像头吗",
        [
            "能打开摄像头吗？我想确认一下画面。",
            "能打开摄像头吗？不方便的话只用语音也可以。",
        ],
    ),
    (
        "先改用文字聊",
        [
            "先改用文字聊，刚才那句话我没听清。",
            "先改用文字聊，等声音恢复了再继续。",
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
fn call_uses_fixed_3268_reading_and_797_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (18, 6, 6));
    assert!(vocabulary.readings().len() >= 3286);
    for (entry, &(reading, text)) in vocabulary.readings()[3268..3286].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts: Vec<_> = vocabulary.continuations().skip(797).take(6).collect();
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
fn call_full_and_specific_partial_spellings_keep_words_authors_and_raw_selectable() {
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
fn call_horizontal_spacing_and_unicode_prefixes_preserve_exact_text() {
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
fn call_adoption_and_known_tail_progress_remain_editable_until_explicit_commit() {
    for (context, mark, han, reading, sentence) in [
        (0, "，", "我", "wo", SENTENCES[0].1[1]),
        (1, "，", "你", "ni", SENTENCES[1].1[1]),
        (2, "，", "你", "ni", SENTENCES[2].1[0]),
        (3, "，", "你", "ni", SENTENCES[3].1[0]),
        (4, "？", "我", "wo", SENTENCES[4].1[0]),
        (5, "，", "刚才", "gang'cai", SENTENCES[5].1[0]),
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
fn call_unknown_protected_tokens_wrong_syllables_and_lines_do_not_borrow_authors() {
    for raw in [
        "zzqvjy",
        "call@example.invalid",
        "https://example.invalid/nidehuamiankazhule",
        "file_nidehuamiankazhule",
        "“raw” 123\t???",
        "ni de hua mi an ka zhu le",
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
        let raw = format!("ni de hua mian{line}ka zhu le");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()) })
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("你的画面卡住了{line}，我");
        let crossed = format!("你的画面卡住了{line}，我先等一会儿。");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == crossed || c.text == SENTENCES[0].1[1] })
        );
        commit(&raw, &raw, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "call-chinese-synthetic-fallback"
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
fn call_unavailable_timeout_and_empty_models_preserve_the_entire_local_pool() {
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
