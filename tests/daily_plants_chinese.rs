//! Finite balcony-plant dialogue; no plant-care advice, sensing, or watering actions.
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
    ("zhe'pen'hua'xian'fang'chuang'bian", "这盆花先放窗边"),
    ("hua'pen'di'xia'you'dian'shui", "花盆底下有点水"),
    ("zhe'pian'ye'zi'diao'xia'lai'le", "这片叶子掉下来了"),
    ("bang'wo'na'yi'xia'pen'hu", "帮我拿一下喷壶"),
    ("yang'tai'de'hua'wo'lai'jiao", "阳台的花我来浇"),
    ("xian'bie'nuo'zhe'ge'hua'pen", "先别挪这个花盆"),
    ("zhi'wu", "植物"),
    ("hua'pen", "花盆"),
    ("pen'zai", "盆栽"),
    ("ye'zi", "叶子"),
    ("pen'hu", "喷壶"),
    ("chuang'tai", "窗台"),
];
const TONES: &[&str] = &[
    "zhe4'pen2'hua1'xian1'fang4'chuang1'bian1",
    "hua1'pen2'di3'xia5'you3'dian3'shui3",
    "zhe4'pian4'ye4'zi5'diao4'xia4'lai2'le5",
    "bang1'wo3'na2'yi1'xia4'pen1'hu2",
    "yang2'tai2'de5'hua1'wo3'lai2'jiao1",
    "xian1'bie2'nuo2'zhe4'ge5'hua1'pen2",
    "zhi2'wu4",
    "hua1'pen2",
    "pen2'zai1",
    "ye4'zi5",
    "pen1'hu2",
    "chuang1'tai2",
];
// Exactly these near-complete spellings, not arbitrary short-prefix predictions.
const PARTIALS: &[(&str, &str)] = &[
    ("zhe'pen'hua'xian'fang'chuang'bia", "这盆花先放窗边"),
    ("hua'pen'di'xia'you'dian'shu", "花盆底下有点水"),
    ("zhe'pian'ye'zi'diao'xia'lai'l", "这片叶子掉下来了"),
    ("bang'wo'na'yi'xia'pen'h", "帮我拿一下喷壶"),
    ("yang'tai'de'hua'wo'lai'jia", "阳台的花我来浇"),
    ("xian'bie'nuo'zhe'ge'hua'pe", "先别挪这个花盆"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "这盆花先放窗边",
        [
            "这盆花先放窗边，我腾个位置出来。",
            "这盆花先放窗边，等会儿再搬到阳台。",
        ],
    ),
    (
        "花盆底下有点水",
        [
            "花盆底下有点水，我拿块布擦一下。",
            "花盆底下有点水，先把桌面擦干吧。",
        ],
    ),
    (
        "这片叶子掉下来了",
        [
            "这片叶子掉下来了，我先捡起来。",
            "这片叶子掉下来了，放进垃圾袋里吧。",
        ],
    ),
    (
        "帮我拿一下喷壶",
        [
            "帮我拿一下喷壶，我把它放到花盆旁边。",
            "帮我拿一下喷壶，用完我就放回去。",
        ],
    ),
    (
        "阳台的花我来浇",
        [
            "阳台的花我来浇，我先去拿水壶。",
            "阳台的花我来浇，你先收一下桌面吧。",
        ],
    ),
    (
        "先别挪这个花盆",
        [
            "先别挪这个花盆，我来腾一下位置。",
            "先别挪这个花盆，旁边的东西还没收好。",
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
fn plants_preserves_all_3378_readings_flags_and_845_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3378);
    assert!(vocabulary.continuations().count() >= 845);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3378) {
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
    assert_eq!(reading_hash, 0x772ea0f1c457fcb7);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(845) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1688);
    assert_eq!(context_hash, 0xc3519b7eea76c788);
    // Invert just this trailing append, preserving the dirty-before JSON bytes.
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if vocabulary.readings().len() > 3378 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"zhe'pen'hua'xian'fang'chuang'bian\", \"text\": \"这盆花先放窗边\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"这盆花先放窗边\",", "\n  ]\n}"),
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
        0xcc8717b6a0b003bd
    );
}

#[test]
fn plants_appends_only_twelve_words_and_six_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (12, 6, 6));
    assert_eq!(vocabulary.readings().len(), 3442);
    assert_eq!(vocabulary.continuations().count(), 937);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1872
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3378..3390].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(845)
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
fn plants_full_words_and_six_specific_partials_offer_exact_choices() {
    assert_eq!(TONES.len(), WORDS.len());
    for (&(reading, word), &tone) in WORDS.iter().zip(TONES) {
        // Tone digits test existing tolerant normalization, not tone-based ranking.
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
fn plants_spaces_adopted_known_tails_and_unicode_backtracking_are_lossless() {
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
    // Every first author has an existing wo -> 我 tail after its comma.
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
fn plants_existing_watering_balcony_words_authors_and_protected_boundaries_stay_intact() {
    // Prior independent Words and authored pairs, not new discoveries or duplicate appends.
    for (reading, word) in [
        ("yang'tai", "阳台"),
        ("tuo'pan", "托盘"),
        ("shai'tai'yang", "晒太阳"),
    ] {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for (reading, word, sentences) in [
        (
            "jiao'shui",
            "浇水",
            ["浇水的时间需要调整一下。", "浇水用的水壶在阳台。"],
        ),
        (
            "gei'hua'jiao'shui",
            "给花浇水",
            ["给花浇水的事就拜托你了。", "给花浇水时别倒太多。"],
        ),
        (
            "liang'zai'yang'tai'shang",
            "晾在阳台上",
            [
                "晾在阳台上，干了以后我来收。",
                "晾在阳台上，衣架之间留点空隙。",
            ],
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
        "plants@example.invalid",
        "https://example.invalid/zhepen/hua",
        "file_zhepenhua",
        "“raw” 123\t???",
        "zh e pen hua xian fang chuang bian",
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
        let raw = format!("zhe{line}pen hua xian fang chuang bian");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("这盆花先放窗边{line}，wo");
        let crossed = format!("这盆花先放窗边{line}，我腾个位置出来。");
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
        "plants-chinese-synthetic-fallback"
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
fn plants_pending_missing_error_timeout_and_empty_models_preserve_the_entire_local_pool() {
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
