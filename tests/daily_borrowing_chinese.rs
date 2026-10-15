//! Frozen borrowing/return/storage fallback expectations; no real borrowing or inputs are inspected.
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
    ("zhe'ge'neng'jie'wo'liang'tian'ma", "这个能借我两天吗"),
    ("wo'ming'tian'dai'lai'huan'ni", "我明天带来还你"),
    ("xian'fang'wo'zhe'li'bao'guan", "先放我这里保管"),
    ("dong'xi'wo'ti'ni'shou'zhe", "东西我替你收着"),
    ("jie'zou'qian'gen'wo'shuo'sheng", "借走前跟我说声"),
    ("huan'de'shi'hou'gao'su'wo", "还的时候告诉我"),
    ("zan'shi'fang'ni'na'bian'ba", "暂时放你那边吧"),
    ("wo'yong'wan'jiu'song'hui'lai", "我用完就送回来"),
    ("jie'yong'ji'lu", "借用记录"),
    ("gui'huan'shi'jian", "归还时间"),
    ("lin'shi'bao'guan", "临时保管"),
    ("wu'pin'jiao'jie", "物品交接"),
    ("jie'yong'qi'xian", "借用期限"),
    ("jie'huan'deng'ji", "借还登记"),
    ("dai'wei'bao'guan", "代为保管"),
    ("zan'cun'wu'pin", "暂存物品"),
    ("ji'cun'ping'tiao", "寄存凭条"),
    ("bao'guan'xiang", "保管箱"),
    ("chu'wu'gui", "储物柜"),
    ("jie'yong'ren", "借用人"),
    ("wu'pin'qing'dan", "物品清单"),
    ("yuan'yang'gui'huan", "原样归还"),
    ("dang'mian'jiao'huan", "当面交还"),
    ("jie'shu'ka", "借书卡"),
];

// Only these near-complete expressions are promised; short ambiguous prefixes
// retain the existing dictionary and homophone priorities.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("zhe'ge'neng'jie'wo'liang'tian'm", "这个能借我两天吗"),
    ("wo'ming'tian'dai'lai'huan'n", "我明天带来还你"),
    ("xian'fang'wo'zhe'li'bao'gua", "先放我这里保管"),
    ("dong'xi'wo'ti'ni'shou'zh", "东西我替你收着"),
    ("jie'zou'qian'gen'wo'shuo'shen", "借走前跟我说声"),
    ("huan'de'shi'hou'gao'su'w", "还的时候告诉我"),
    ("zan'shi'fang'ni'na'bian'b", "暂时放你那边吧"),
    ("wo'yong'wan'jiu'song'hui'la", "我用完就送回来"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "这个能借我两天吗",
        [
            "这个能借我两天吗？周末我就还给你。",
            "这个能借我两天吗？我想带回去试试。",
        ],
    ),
    (
        "我明天带来还你",
        [
            "我明天带来还你，今天先放在我包里。",
            "我明天带来还你，谢谢你借给我用。",
        ],
    ),
    (
        "先放我这里保管",
        [
            "先放我这里保管，你走的时候再拿。",
            "先放我这里保管，免得一会儿找不到。",
        ],
    ),
    (
        "东西我替你收着",
        [
            "东西我替你收着，回来记得找我拿。",
            "东西我替你收着，就放在柜子里面。",
        ],
    ),
    (
        "借走前跟我说声",
        [
            "借走前跟我说声，我好知道东西在哪。",
            "借走前跟我说声，用完放回原处就行。",
        ],
    ),
    (
        "还的时候告诉我",
        [
            "还的时候告诉我，我看看有没有漏下的。",
            "还的时候告诉我，我们约个方便的时间。",
        ],
    ),
    (
        "暂时放你那边吧",
        [
            "暂时放你那边吧，我回头来拿。",
            "暂时放你那边吧，别跟别人的混在一起。",
        ],
    ),
    (
        "我用完就送回来",
        [
            "我用完就送回来，不会耽误你用。",
            "我用完就送回来，谢谢你帮忙。",
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
fn borrowing_uses_fixed_3138_reading_and_759_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 8);
    assert!(vocabulary.readings().len() >= 3162);
    for (entry, &(reading, word)) in vocabulary.readings()[3138..3162].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(759)
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
fn borrowing_full_and_specific_partial_inputs_keep_words_sentences_and_raw_submission() {
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
fn borrowing_horizontal_spacing_preserves_raw_and_adopted_drafts() {
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
fn borrowing_adoption_progress_and_reseeding_remain_editable_until_commit() {
    for (word, punctuation, tail_reading, tail_word, sentence) in [
        ("这个能借我两天吗", "？", "wo", "我", SENTENCES[0].1[1]),
        (
            "我明天带来还你",
            "，",
            "jin'tian",
            "今天",
            SENTENCES[1].1[0],
        ),
        ("先放我这里保管", "，", "ni", "你", SENTENCES[2].1[0]),
        ("借走前跟我说声", "，", "wo", "我", SENTENCES[4].1[0]),
        ("暂时放你那边吧", "，", "wo", "我", SENTENCES[6].1[0]),
        ("我用完就送回来", "，", "xie'xie", "谢谢", SENTENCES[7].1[1]),
    ] {
        let reading = WORDS.iter().find(|(_, text)| *text == word).unwrap().0;
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        // Host-style adoption/reseed/rewind; physical keyboard dispatch is separate.
        for padding in ["", " ", "\t", "\u{3000}"] {
            for tail in std::iter::once(tail_word.to_owned()).chain(spellings(tail_reading)) {
                let seed = format!("{word}{padding}{punctuation}{tail}");
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
fn borrowing_boundaries_keep_raw_text_and_prior_short_word_first_pages() {
    for seed in [
        "zzqvjy",
        "borrowing@example.invalid",
        "https://example.invalid/zhegenengjiewoliangtianma",
        "“raw” 123\t???",
        "zhe ge ne ng jie wo liang tian ma",
        "wo ming tian dai la i huan ni",
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
        let seed = format!("这个能借我两天吗{boundary}？wo");
        let crossed = format!("这个能借我两天吗{boundary}？我想带回去试试。");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|candidate| candidate.text == crossed)
        );
        commit_once_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    // Growing this collection must retain the previously repaired demonstrative page.
    for seed in ["zheg", "zhe'g", "zhe g", "ZHE G"] {
        let ime = engine(seed);
        draft(&ime, seed);
        for (text, score) in [
            ("这个词", 100.0),
            ("这个意思", 92.0),
            ("这个更合适", 90.0),
            ("这个", 88.0),
        ] {
            let position = index(&ime, text, CandidateKind::Word);
            assert_eq!(ime.candidates()[position].score, score);
        }
        for sentence in ["这个词是什么意思？", "这个词应该怎么用？"] {
            assert!(index(&ime, sentence, CandidateKind::Sentence) < PAGE_SIZE);
        }
        commit_once_and_undo(&mut engine(seed), "这个", CandidateKind::Word);
    }
    for (reading, word, prior) in [
        ("zhe'bia", "这边", Some("这边弄好了")),
        ("zai'na'l", "在哪里", None),
    ] {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            if let Some(prior) = prior {
                index(&ime, prior, CandidateKind::Word);
            }
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for (reading, word) in [
        ("jie'wo'yi'xia", "借我一下"),
        ("xian'jie'wo'yong'yong", "先借我用用"),
        ("yong'wan'huan'gei'wo", "用完还给我"),
        ("yong'wan'huan'gei'ni", "用完还给你"),
        ("qing'jie'wo'yong'yi'xia", "请借我用一下"),
        ("gui'huan'tu'shu", "归还图书"),
    ] {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            commit_once_and_undo(&mut ime, word, CandidateKind::Word);
            commit_once_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for seed in ["hai", "HAI"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(ime.candidates()[0].text, "还");
        assert_eq!(ime.candidates()[0].score, 100.0);
        commit_once_and_undo(&mut engine(seed), "还", CandidateKind::Word);
    }
}

struct FallbackProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        "borrowing-chinese-synthetic-fallback"
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
fn borrowing_model_unavailable_timeout_and_empty_results_preserve_local_choices() {
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
