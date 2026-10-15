//! Frozen finding/belongings fallback expectations; no belongings or real inputs are inspected.
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
    ("yao'shi'wang'zai'jia'li'le", "钥匙忘在家里了"),
    ("bao'li'zai'zhao'yi'xia", "包里再找一下"),
    ("ni'kan'dao'wo'de'qian'bao'le'ma", "你看到我的钱包了吗"),
    ("zhe'ba'san'shi'shui'de", "这把伞是谁的"),
    ("dong'xi'wang'zai'che'shang'le", "东西忘在车上了"),
    ("bang'wo'liu'zhe'xian'bie'reng", "帮我留着先别扔"),
    ("wo'wan'dian'guo'qu'na", "我晚点过去拿"),
    ("zhao'dao'le'jiu'zai'zhuo'shang", "找到了就在桌上"),
    ("yao'shi'chuan", "钥匙串"),
    ("yao'shi'kou", "钥匙扣"),
    ("ling'qian'bao", "零钱包"),
    ("ka'bao", "卡包"),
    ("zheng'jian'dai", "证件袋"),
    ("yan'jing'he", "眼镜盒"),
    ("er'ji'dai", "耳机袋"),
    ("shou'na'bao", "收纳包"),
    ("wai'tao'kou'dai", "外套口袋"),
    ("ku'zi'kou'dai", "裤子口袋"),
    ("zuo'wei'di'xia", "座位底下"),
    ("zhao'ling'qi'shi", "招领启事"),
    ("ji'cun'chu", "寄存处"),
    ("yi'luo'wu'pin", "遗落物品"),
    ("wu'pin'gui'shu", "物品归属"),
    ("ling'qu'deng'ji", "领取登记"),
];

// Only these near-complete expressions are promised; short ambiguous prefixes
// retain the existing dictionary and homophone priorities.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("yao'shi'wang'zai'jia'li'l", "钥匙忘在家里了"),
    ("bao'li'zai'zhao'yi'xi", "包里再找一下"),
    ("ni'kan'dao'wo'de'qian'bao'le'm", "你看到我的钱包了吗"),
    ("zhe'ba'san'shi'shui'd", "这把伞是谁的"),
    ("dong'xi'wang'zai'che'shang'l", "东西忘在车上了"),
    ("bang'wo'liu'zhe'xian'bie'ren", "帮我留着先别扔"),
    ("wo'wan'dian'guo'qu'n", "我晚点过去拿"),
    ("zhao'dao'le'jiu'zai'zhuo'shan", "找到了就在桌上"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "钥匙忘在家里了",
        [
            "钥匙忘在家里了，我得回去拿一趟。",
            "钥匙忘在家里了，你先进去吧。",
        ],
    ),
    (
        "包里再找一下",
        [
            "包里再找一下，看看侧边的口袋。",
            "包里再找一下，可能夹在本子中间了。",
        ],
    ),
    (
        "你看到我的钱包了吗",
        [
            "你看到我的钱包了吗？刚才好像还在这里。",
            "你看到我的钱包了吗？我去问问前台。",
        ],
    ),
    (
        "这把伞是谁的",
        [
            "这把伞是谁的？我先放在门口。",
            "这把伞是谁的？走的时候别忘了拿。",
        ],
    ),
    (
        "东西忘在车上了",
        [
            "东西忘在车上了，我联系一下司机。",
            "东西忘在车上了，你帮我看看后排。",
        ],
    ),
    (
        "帮我留着先别扔",
        [
            "帮我留着先别扔，我晚点来取。",
            "帮我留着先别扔，那是我还要用的。",
        ],
    ),
    (
        "我晚点过去拿",
        [
            "我晚点过去拿，你先替我收好。",
            "我晚点过去拿，出发前再联系你。",
        ],
    ),
    (
        "找到了就在桌上",
        [
            "找到了就在桌上，刚才被纸挡住了。",
            "找到了就在桌上，我这就收起来。",
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
fn belongings_uses_fixed_3114_reading_and_751_context_append_windows() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 8);
    assert!(vocabulary.readings().len() >= 3138);
    for (entry, &(reading, word)) in vocabulary.readings()[3114..3138].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(751)
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
fn belongings_full_and_specific_partial_inputs_keep_words_sentences_and_raw_submission() {
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
fn belongings_horizontal_spacing_preserves_raw_and_adopted_drafts() {
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
fn belongings_adoption_progress_and_reseeding_remain_editable_until_commit() {
    for (word, punctuation, tail_reading, tail_word, sentence) in [
        ("钥匙忘在家里了", "，", "wo", "我", SENTENCES[0].1[0]),
        ("包里再找一下", "，", "kan", "看", SENTENCES[1].1[0]),
        ("这把伞是谁的", "？", "wo", "我", SENTENCES[3].1[0]),
        ("东西忘在车上了", "，", "wo", "我", SENTENCES[4].1[0]),
        ("我晚点过去拿", "，", "ni", "你", SENTENCES[6].1[0]),
        ("找到了就在桌上", "，", "wo", "我", SENTENCES[7].1[1]),
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
fn belongings_boundaries_keep_raw_text_and_prior_short_word_first_pages() {
    for seed in [
        "zzqvjy",
        "belongings@example.invalid",
        "https://example.invalid/yaoshiwangzaijialile",
        "“raw” 123\t???",
        "yao shi wa ng zai jia li le",
        "dong xi wang zai ch e shang le",
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
        let seed = format!("钥匙忘在家里了{boundary}，wo");
        let crossed = format!("钥匙忘在家里了{boundary}，我得回去拿一趟。");
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
}

struct FallbackProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FallbackProvider {
    fn provider_id(&self) -> &str {
        "belongings-chinese-synthetic-fallback"
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
fn belongings_model_unavailable_timeout_and_empty_results_preserve_local_choices() {
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
