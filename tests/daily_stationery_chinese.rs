//! Finite stationery and handwriting phrases, independent of models or desk actions.
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
    ("qian'bi'you'dian'dun'le", "铅笔有点钝了"),
    ("zhe'ge'zi'xie'cuo'le", "这个字写错了"),
    ("bang'wo'hua'yi'tiao'xian", "帮我画一条线"),
    ("xian'na'zhang'cao'gao'zhi", "先拿张草稿纸"),
    ("bi'mao'ji'de'gai'hao", "笔帽记得盖好"),
    ("zhe'ji'zhang'zhi'ding'yi'xia", "这几张纸订一下"),
    ("qian'bi", "铅笔"),
    ("chi'zi", "尺子"),
    ("jiao'dai", "胶带"),
    ("bian'qian", "便签"),
    ("cao'gao'zhi", "草稿纸"),
    ("yuan'zhu'bi", "圆珠笔"),
];
const TONES: &[&str] = &[
    "qian2'bi3'you3'dian3'dun4'le5",
    "zhe4'ge5'zi4'xie3'cuo4'le5",
    "bang1'wo3'hua4'yi1'tiao2'xian4",
    "xian1'na2'zhang1'cao3'gao3'zhi3",
    "bi3'mao4'ji4'de5'gai4'hao3",
    "zhe4'ji3'zhang1'zhi3'ding4'yi1'xia4",
    "qian2'bi3",
    "chi3'zi5",
    "jiao1'dai4",
    "bian4'qian1",
    "cao3'gao3'zhi3",
    "yuan2'zhu1'bi3",
];
// These specific final-letter omissions do not promise arbitrary short prefixes.
const PARTIALS: &[(&str, &str)] = &[
    ("qian'bi'you'dian'dun'l", "铅笔有点钝了"),
    ("zhe'ge'zi'xie'cuo'l", "这个字写错了"),
    ("bang'wo'hua'yi'tiao'xia", "帮我画一条线"),
    ("xian'na'zhang'cao'gao'zh", "先拿张草稿纸"),
    ("bi'mao'ji'de'gai'ha", "笔帽记得盖好"),
    ("zhe'ji'zhang'zhi'ding'yi'xi", "这几张纸订一下"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "铅笔有点钝了",
        [
            "铅笔有点钝了，我先削一下再写。",
            "铅笔有点钝了，换一支尖一点的吧。",
        ],
    ),
    (
        "这个字写错了",
        [
            "这个字写错了，我先擦掉再重写。",
            "这个字写错了，在旁边改一下就好。",
        ],
    ),
    (
        "帮我画一条线",
        [
            "帮我画一条线，我想把这两段分开。",
            "帮我画一条线，用尺子画会整齐一点。",
        ],
    ),
    (
        "先拿张草稿纸",
        [
            "先拿张草稿纸，我想算一下这道题。",
            "先拿张草稿纸，这张先别放进文件夹。",
        ],
    ),
    (
        "笔帽记得盖好",
        [
            "笔帽记得盖好，我先收一下桌面。",
            "笔帽记得盖好，用完就收回笔袋吧。",
        ],
    ),
    (
        "这几张纸订一下",
        [
            "这几张纸订一下，我想把它们放在一起。",
            "这几张纸订一下，订好后放进文件夹吧。",
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
    if matches!(kind, CandidateKind::Word | CandidateKind::Sentence) {
        assert!(at < PAGE_SIZE, "Word/Sentence left page one: {text:?}");
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
fn stationery_preserves_all_3354_readings_flags_and_833_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3354);
    assert!(vocabulary.continuations().count() >= 833);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3354) {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for byte in entry
            .reading
            .bytes()
            .chain([0])
            .chain(entry.text.bytes())
            .chain([0])
            .chain(kind.bytes())
            .chain([u8::from(entry.require_separators)])
        {
            reading_hash = (reading_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    assert_eq!(reading_hash, 0xe64dca96feb2f68a);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(833) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1664);
    assert_eq!(context_hash, 0xb35ecd27c2f5f9b3);
    // Preserve the actual dirty-before JSON bytes, not merely parsed records.
    // After the append, project only its exact trailing reading/context ranges.
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if vocabulary.readings().len() > 3354 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"qian'bi'you'dian'dun'le\", \"text\": \"铅笔有点钝了\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"铅笔有点钝了\",", "\n  ]\n}"),
        ] {
            assert_eq!(raw.matches(first).count(), 1);
            assert_eq!(raw.matches(after).count(), 1);
            let (start, end) = (raw.find(first).unwrap(), raw.find(after).unwrap());
            assert!(start < end);
            raw.replace_range(start..end, "");
        }
    }
    assert_eq!(
        raw.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        }),
        0xfa710fe928160bf7
    );
}

#[test]
fn stationery_appends_only_twelve_words_and_six_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (12, 6, 6));
    assert_eq!(vocabulary.readings().len(), 3442);
    assert_eq!(vocabulary.continuations().count(), 906);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1810
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3354..3366].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(833)
        .take(6)
        .zip(SENTENCES)
        .zip(WORDS)
    {
        assert_eq!(context, word);
        assert_eq!(word, declared_word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            sentences
        );
    }
}

#[test]
fn stationery_full_words_and_six_specific_partials_offer_exact_choices() {
    assert_eq!(TONES.len(), WORDS.len());
    for (&(reading, word), &tone) in WORDS.iter().zip(TONES) {
        for seed in spellings(reading).into_iter().chain([tone.to_owned()]) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for &(reading, word) in WORDS.iter().take(6).chain(PARTIALS) {
        let sentences = SENTENCES
            .iter()
            .find(|(context, _)| *context == word)
            .unwrap()
            .1;
        let mut forms = spellings(reading).to_vec();
        if let Some(at) = WORDS.iter().take(6).position(|(full, _)| *full == reading) {
            forms.push(TONES[at].to_owned());
        }
        for seed in forms {
            commit(&seed, word, CandidateKind::Word);
            for sentence in sentences {
                commit(&seed, sentence, CandidateKind::Sentence);
            }
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn stationery_spaces_adopted_known_tails_and_unicode_backtracking_are_lossless() {
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for &(reading, word) in WORDS {
            let raw = format!("{}{gap}", reading.replace('\'', gap));
            commit(&raw, word, CandidateKind::Word);
            commit(&raw, &raw, CandidateKind::Literal);
        }
        for &(word, sentences) in SENTENCES {
            let adopted = format!("{word}{gap}");
            for sentence in sentences {
                let expected = format!("{adopted}{}", sentence.strip_prefix(word).unwrap());
                commit(&adopted, &expected, CandidateKind::Sentence);
            }
            commit(&adopted, &adopted, CandidateKind::Literal);
        }
    }
    // Every first authored sentence uses existing wo -> 我 after its comma.
    // Selection/reseeding does not simulate physical keys or general-tail prediction.
    for prefix in [
        String::new(),
        "已确认 API。".to_owned(),
        "前文 café 😀。  ".repeat(40),
    ] {
        for (&(full, word), &(_, sentences)) in WORDS.iter().take(6).zip(SENTENCES) {
            let sentence = sentences[0];
            assert!(sentence.starts_with(&format!("{word}，我")));
            let original = format!("{prefix}{}", full.replace('\'', " "));
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&original);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert!(ime.snapshot().committed_text.is_empty());
            for gap in ["", " ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
                for tail in ["我", "wo", "WO"] {
                    let raw = format!("{adopted}{gap}，{tail}");
                    let expected =
                        format!("{adopted}{gap}{}", sentence.strip_prefix(word).unwrap());
                    ime.seed(&raw);
                    draft(&ime, &raw);
                    assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
                    index(&ime, &expected, CandidateKind::Sentence);
                    ime.seed(&original);
                    draft(&ime, &original);
                    index(&ime, &adopted, CandidateKind::Word);
                    commit(&raw, &expected, CandidateKind::Sentence);
                    commit(&raw, &raw, CandidateKind::Literal);
                }
            }
        }
    }
}

#[test]
fn stationery_existing_paper_pen_words_authors_and_protected_line_boundaries_stay_intact() {
    // These are old independent Words, not discoveries or duplicate appends.
    for (reading, word) in [
        ("na'zhang'zhi", "拿张纸"),
        ("tian'ge'ming'zi", "填个名字"),
        ("xie'ge'di'zhi", "写个地址"),
        ("bi'mao", "笔帽"),
        ("xiang'pi", "橡皮"),
        ("qian'zi'bi", "签字笔"),
        ("ding'shu'ji", "订书机"),
        ("bian'li'tie", "便利贴"),
        ("cao'gao", "草稿"),
        ("cuo'bie'zi", "错别字"),
        ("feng'xiang'jiao'dai", "封箱胶带"),
    ] {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for (reading, word, sentences) in [
        (
            "jie'zhi'bi",
            "借支笔",
            [
                "借支笔填个名字，马上还你。",
                "借支笔写个地址，用完就还给你。",
            ],
        ),
        (
            "zhi'zhang",
            "纸张",
            ["纸张放在打印机旁边了。", "纸张还有一些，我先拿几张过来。"],
        ),
        (
            "pin'xie'cuo'wu",
            "拼写错误",
            ["拼写错误已经修正了。", "拼写错误请帮我标出来。"],
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
    for raw in [
        "zzqvjy",
        "stationery@example.invalid",
        "https://example.invalid/qianbi/youdiandunle",
        "file_qianbiyoudiandunle",
        "“raw” 123\t???",
        "qia n bi you dian dun le",
    ] {
        assert!(!engine(raw).candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|s| c.text.contains(s)))
        }));
        commit(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("qian{line}bi you dian dun le");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("铅笔有点钝了{line}，wo");
        let crossed = format!("铅笔有点钝了{line}，我先削一下再写。");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == crossed || c.text == SENTENCES[0].1[0])
        );
        commit(&raw, &raw, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "stationery-chinese-synthetic-fallback"
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
fn stationery_pending_missing_error_timeout_and_empty_models_preserve_the_entire_local_pool() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (&(seed, word), &(_, sentences)) in PARTIALS.iter().zip(SENTENCES) {
            let without_model = engine(seed).candidates().to_vec();
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            assert!(ime.prediction_pending());
            assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
            assert_eq!(ime.candidates(), without_model);
            draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                index(&ime, sentence, CandidateKind::Sentence);
            }
            let before = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                assert_eq!(ime.candidates(), before);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!ime.prediction_pending());
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
