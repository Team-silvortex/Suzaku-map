//! Finite indoor-light, ventilation and comfort expressions, not device control.
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
    ("fang'jian'you'dian'men", "房间有点闷"),
    ("kai'chuang'tong'tong'feng", "开窗通通风"),
    ("ba'chuang'lian'la'kai", "把窗帘拉开"),
    ("deng'guang'you'dian'ci'yan", "灯光有点刺眼"),
    ("feng'shan'tiao'xiao'yi'dian", "风扇调小一点"),
    ("kong'tiao'wen'du'tiao'gao'yi'dian", "空调温度调高一点"),
    ("guang'xian", "光线"),
    ("tong'feng", "通风"),
    ("tai'deng", "台灯"),
    ("feng'shan", "风扇"),
    ("kong'tiao", "空调"),
    ("wen'du", "温度"),
];
// These specific final-letter omissions do not promise arbitrary short prefixes.
const PARTIALS: &[(&str, &str)] = &[
    ("fang'jian'you'dian'me", "房间有点闷"),
    ("kai'chuang'tong'tong'fen", "开窗通通风"),
    ("ba'chuang'lian'la'ka", "把窗帘拉开"),
    ("deng'guang'you'dian'ci'ya", "灯光有点刺眼"),
    ("feng'shan'tiao'xiao'yi'dia", "风扇调小一点"),
    ("kong'tiao'wen'du'tiao'gao'yi'dia", "空调温度调高一点"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "房间有点闷",
        [
            "房间有点闷，我想开一会儿窗。",
            "房间有点闷，我们换个位置坐吧。",
        ],
    ),
    (
        "开窗通通风",
        [
            "开窗通通风，我先把桌上的纸压好。",
            "开窗通通风，等会儿再关上。",
        ],
    ),
    (
        "把窗帘拉开",
        [
            "把窗帘拉开，我想让房间亮一点。",
            "把窗帘拉开，看看外面的天气。",
        ],
    ),
    (
        "灯光有点刺眼",
        [
            "灯光有点刺眼，我想换个位置坐。",
            "灯光有点刺眼，能不能调暗一点？",
        ],
    ),
    (
        "风扇调小一点",
        [
            "风扇调小一点，我这边风有点大。",
            "风扇调小一点，别对着桌上的纸吹。",
        ],
    ),
    (
        "空调温度调高一点",
        [
            "空调温度调高一点，我觉得有点冷。",
            "空调温度调高一点，先这样试一会儿吧。",
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
fn room_comfort_preserves_all_3342_readings_flags_and_827_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3342);
    assert!(vocabulary.continuations().count() >= 827);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3342) {
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
    assert_eq!(reading_hash, 0x2ef7e4dc5e940f83);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(827) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1652);
    assert_eq!(context_hash, 0x5098d957a58750e4);
}

#[test]
fn room_comfort_appends_only_twelve_words_and_six_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (12, 6, 6));
    assert_eq!(vocabulary.readings().len(), 3410);
    assert_eq!(vocabulary.continuations().count(), 861);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1720
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3342..3354].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(827)
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
fn room_comfort_full_words_and_six_specific_partials_offer_exact_choices() {
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
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
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            for sentence in sentences {
                commit(&seed, sentence, CandidateKind::Sentence);
            }
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn room_comfort_spaces_adopted_known_tails_and_unicode_backtracking_are_lossless() {
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
fn room_comfort_old_window_light_and_temperature_authors_and_boundaries_are_preserved() {
    // Existing window/light/cold/heat words and authors are controls, not new coverage.
    for (reading, word, sentences) in [
        (
            "chuang'hu'guan'bu'shang",
            "窗户关不上",
            [
                "窗户关不上，想约师傅过来看看。",
                "窗户关不上，这个问题有几天了。",
            ],
        ),
        (
            "ping'mu'tai'liang'le",
            "屏幕太亮了",
            [
                "屏幕太亮了，我想调暗一点。",
                "屏幕太亮了，晚上看着有点晃眼。",
            ],
        ),
        (
            "you'dian'leng",
            "有点冷",
            ["有点冷，我去拿件外套。", "有点冷，先把窗户关上吧。"],
        ),
        (
            "you'dian're",
            "有点热",
            ["有点热，我想开一会儿窗。", "有点热，等凉快一点再出门。"],
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
        "room@example.invalid",
        "https://example.invalid/fangjian/youdianmen",
        "file_fangjianyoudianmen",
        "“raw” 123\t???",
        "fang jia n you dian men",
    ] {
        assert!(!engine(raw).candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.iter().any(|sentence| c.text.contains(sentence)))
        }));
        commit(raw, raw, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let raw = format!("fang{line}jian you dian men");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("房间有点闷{line}，wo");
        let crossed = format!("房间有点闷{line}，我想开一会儿窗。");
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
        "room-comfort-chinese-synthetic-fallback"
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
fn room_comfort_pending_missing_timeout_and_empty_models_preserve_the_entire_local_pool() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
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
