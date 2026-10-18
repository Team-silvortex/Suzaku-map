//! Fixed appointment, walk-in and queue vocabulary, not a booking service.
//! No personal appointments, live opening hours or medical advice are used.
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

// Eight explicit context triggers, then sixteen related Word readings. The
// independent 取号 mapping is a real gap; no 里/那/把 character is added.
const WORDS: &[(&str, &str)] = &[
    ("xu'yao'ti'qian'yu'yue'ma", "需要提前预约吗"),
    ("ke'yi'zhi'jie'dao'dian'ma", "可以直接到店吗"),
    ("yu'yue'shi'jian'neng'gai'ma", "预约时间能改吗"),
    ("wo'yi'jing'dao'men'kou'le", "我已经到门口了"),
    ("dao'dian'hou'zen'me'qu'hao", "到店后怎么取号"),
    ("da'gai'hai'yao'pai'duo'jiu", "大概还要排多久"),
    ("guo'hao'le'hai'neng'zai'pai'ma", "过号了还能再排吗"),
    ("jin'tian'ji'dian'ting'zhi'jie'dai", "今天几点停止接待"),
    ("yu'yue'ming'e", "预约名额"),
    ("yu'yue'que'ren", "预约确认"),
    ("xian'chang'yu'yue", "现场预约"),
    ("xian'shang'yu'yue", "线上预约"),
    ("yu'yue'bian'geng", "预约变更"),
    ("qu'xiao'yu'yue", "取消预约"),
    ("dao'dian'deng'ji", "到店登记"),
    ("yu'yue'ping'zheng", "预约凭证"),
    ("qu'hao'ji", "取号机"),
    ("pai'dui'hao'ma", "排队号码"),
    ("pai'dui'ren'shu", "排队人数"),
    ("qu'hao", "取号"),
    ("guo'hao'chong'pai", "过号重排"),
    ("hou'wei'tong'zhi", "候位通知"),
    ("fu'wu'shi'duan", "服务时段"),
    ("zui'hou'jie'dai'shi'jian", "最后接待时间"),
];

// Exactly these near-complete inputs, not arbitrary shorter prefixes or all
// ambiguous reading branches, are promised to keep the eight whole Words.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("xu'yao'ti'qian'yu'yue'm", "需要提前预约吗"),
    ("ke'yi'zhi'jie'dao'dian'm", "可以直接到店吗"),
    ("yu'yue'shi'jian'neng'gai'm", "预约时间能改吗"),
    ("wo'yi'jing'dao'men'kou'l", "我已经到门口了"),
    ("dao'dian'hou'zen'me'qu'ha", "到店后怎么取号"),
    ("da'gai'hai'yao'pai'duo'ji", "大概还要排多久"),
    ("guo'hao'le'hai'neng'zai'pai'm", "过号了还能再排吗"),
    ("jin'tian'ji'dian'ting'zhi'jie'da", "今天几点停止接待"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "需要提前预约吗",
        [
            "需要提前预约吗？我想周末过去一趟。",
            "需要提前预约吗？还是到店后再登记？",
        ],
    ),
    (
        "可以直接到店吗",
        [
            "可以直接到店吗？我还没有预约。",
            "可以直接到店吗？不行的话我先约个时间。",
        ],
    ),
    (
        "预约时间能改吗",
        [
            "预约时间能改吗？我想换到明天下午。",
            "预约时间能改吗？原来的时段有点赶。",
        ],
    ),
    (
        "我已经到门口了",
        [
            "我已经到门口了，接下来去哪里登记？",
            "我已经到门口了，需要先出示预约凭证吗？",
        ],
    ),
    (
        "到店后怎么取号",
        [
            "到店后怎么取号？是在前台办理吗？",
            "到店后怎么取号？我第一次过来不太熟悉。",
        ],
    ),
    (
        "大概还要排多久",
        [
            "大概还要排多久？我先去附近买瓶水。",
            "大概还要排多久？前面还有几位在等？",
        ],
    ),
    (
        "过号了还能再排吗",
        [
            "过号了还能再排吗？我刚才没听到叫号。",
            "过号了还能再排吗？需要重新取一个号吗？",
        ],
    ),
    (
        "今天几点停止接待",
        [
            "今天几点停止接待？我怕赶过去太晚。",
            "今天几点停止接待？取号截止时间一样吗？",
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
fn appointments_uses_fixed_3202_reading_and_775_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 8);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 3226);
    for (entry, &(reading, word)) in vocabulary.readings()[3202..3226].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    assert_eq!(WORDS[19], ("qu'hao", "取号"));
    let contexts = vocabulary
        .continuations()
        .skip(775)
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
fn appointments_full_and_specific_partial_spellings_keep_words_sentences_and_raw() {
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
    for seed in ["quhao", "qu'hao", "qu hao", "QU HAO"] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(index(&ime, "取号", CandidateKind::Word), 0);
        commit_once_and_undo(&mut ime, "取号", CandidateKind::Word);
    }
}

#[test]
fn appointments_horizontal_spacing_keeps_raw_and_adopted_han_text_exact() {
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
fn appointments_adoption_reseed_and_unicode_prefixes_stay_editable_until_commit() {
    for (context, punctuation, reading_tail, han_tail, sentence) in [
        (0, "？", "wo", "我", SENTENCES[0].1[0]),
        (1, "？", "wo", "我", SENTENCES[1].1[0]),
        (2, "？", "wo", "我", SENTENCES[2].1[0]),
        (3, "，", "xu'yao", "需要", SENTENCES[3].1[1]),
        (4, "？", "shi", "是", SENTENCES[4].1[0]),
        (5, "？", "wo", "我", SENTENCES[5].1[0]),
        (6, "？", "wo", "我", SENTENCES[6].1[0]),
        (7, "？", "wo", "我", SENTENCES[7].1[0]),
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
fn appointments_boundaries_keep_literals_and_original_appointment_sentences_on_page_one() {
    for raw in [
        "zzqvjy",
        "appointments@example.invalid",
        "https://example.invalid/xuyaotiqianyuyuema",
        "file_xuyaotiqianyuyuema",
        "“raw” 123\t???",
        "yu yu e shi jian neng gai ma",
        "ke yi zhi ji e dao dian ma",
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
        let raw = format!("xu yao ti qian{boundary}yu yue ma");
        let mut ime = engine(&raw);
        draft(&ime, &raw);
        assert!(!ime.candidates().iter().any(|candidate| {
            candidate.text == WORDS[0].1 || SENTENCES[0].1.contains(&candidate.text.as_str())
        }));
        commit_once_and_undo(&mut ime, &raw, CandidateKind::Literal);
        let raw = format!("需要提前预约吗{boundary}？wo");
        let crossed = format!("需要提前预约吗{boundary}？我想周末过去一趟。");
        let mut ime = engine(&raw);
        draft(&ime, &raw);
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| candidate.text == crossed || candidate.text == SENTENCES[0].1[0])
        );
        commit_once_and_undo(&mut ime, &raw, CandidateKind::Literal);
    }
    // Preserve the existing whole Word and its two authored sentences, not an
    // invented promise that every newly enriched full vector stays identical.
    for raw in ["yuyue", "yu'yue", "yu yue", "YU YUE"] {
        let ime = engine(raw);
        draft(&ime, raw);
        assert_eq!(index(&ime, "预约", CandidateKind::Word), 0);
        for sentence in ["预约已经确认了。", "预约时间可以修改吗？"] {
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once_and_undo(&mut engine(raw), sentence, CandidateKind::Sentence);
        }
        commit_once_and_undo(&mut engine(raw), raw, CandidateKind::Literal);
    }
}

struct FallbackProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        "appointments-chinese-synthetic-fallback"
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
fn appointments_unavailable_timeout_and_empty_models_keep_the_complete_local_pool() {
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
