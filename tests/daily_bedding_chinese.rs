//! Finite bedding and bedtime tidying dialogue, not medical or sleep-effect advice.
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

const BEFORE_RESOURCE_SHA256: &str =
    "0fb23eb192fed8e96523ec1af10a71ff10d05a3b4a327ac191fadbc66a6231eb";
const WORDS: &[(&str, &str)] = &[
    ("zhen'tou'fang'zai'chuang'tou", "枕头放在床头"),
    ("bei'zi'xian'die'qi'lai", "被子先叠起来"),
    ("chuang'dan'huan'hao'le'ma", "床单换好了吗"),
    ("shui'qian'ba'chuang'pu'hao", "睡前把床铺好"),
    ("zhen'tou", "枕头"),
    ("bei'zi", "被子"),
    ("tan'zi", "毯子"),
    ("chuang'dian", "床垫"),
];
const TONES: &[&str] = &[
    "zhen3'tou5'fang4'zai4'chuang2'tou2",
    "bei4'zi5'xian1'die2'qi3'lai2",
    "chuang2'dan1'huan4'hao3'le5'ma5",
    "shui4'qian2'ba3'chuang2'pu1'hao3",
    "zhen3'tou5",
    "bei4'zi5",
    "tan3'zi5",
    "chuang2'dian4",
];
// Exactly these four near-complete readings, not arbitrary short-prefix promises.
const PARTIALS: &[(&str, &str)] = &[
    ("zhen'tou'fang'zai'chuang'to", "枕头放在床头"),
    ("bei'zi'xian'die'qi'la", "被子先叠起来"),
    ("chuang'dan'huan'hao'le'm", "床单换好了吗"),
    ("shui'qian'ba'chuang'pu'ha", "睡前把床铺好"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "枕头放在床头",
        [
            "枕头放在床头，我来整理被子。",
            "枕头放在床头，别压住旁边的书。",
        ],
    ),
    (
        "被子先叠起来",
        [
            "被子先叠起来，我把床单拉平。",
            "被子先叠起来，等会儿再放进柜子。",
        ],
    ),
    (
        "床单换好了吗",
        [
            "床单换好了吗？我把枕套拿过来。",
            "床单换好了吗？旧的我来收起来。",
        ],
    ),
    (
        "睡前把床铺好",
        [
            "睡前把床铺好，我先去洗漱。",
            "睡前把床铺好，衣服放到椅子上。",
        ],
    ),
];
// Actual authored punctuation, then the existing wo -> 我 mapping.
const TAILS: &[(&str, &str, &str)] = &[
    ("，", "我", "wo"),
    ("，", "我", "wo"),
    ("？", "我", "wo"),
    ("，", "我", "wo"),
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
fn bedding_preserves_all_3402_readings_flags_and_857_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3402);
    assert!(vocabulary.continuations().count() >= 857);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3402) {
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
    assert_eq!(reading_hash, 0xa571ee180bf634ed);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(857) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1712);
    assert_eq!(context_hash, 0x37054f5c2d8d8e7e);
    // Exact trailing-append inverse; the independent receipt also locks SHA-256.
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if vocabulary.readings().len() > 3402 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"zhen'tou'fang'zai'chuang'tou\", \"text\": \"枕头放在床头\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"枕头放在床头\",", "\n  ]\n}"),
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
        0x767c4bb6ecbcd10f,
        "before resource SHA-256 receipt: {BEFORE_RESOURCE_SHA256}"
    );
}

#[test]
fn bedding_appends_only_eight_words_and_four_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (8, 4, 4));
    assert_eq!(vocabulary.readings().len(), 3442);
    assert_eq!(vocabulary.continuations().count(), 937);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, vs)| vs.len())
            .sum::<usize>(),
        1872
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3402..3410].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(857)
        .take(4)
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
fn bedding_full_words_tones_and_four_specific_partials_offer_exact_choices() {
    assert_eq!(TONES.len(), WORDS.len());
    for (&(reading, word), &tone) in WORDS.iter().zip(TONES) {
        // Tolerant tone-digit normalization, not tone-conditioned ranking.
        for seed in spellings(reading).into_iter().chain([tone.to_owned()]) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for &(reading, word) in WORDS.iter().take(4).chain(PARTIALS) {
        let sentences = SENTENCES
            .iter()
            .find(|(context, _)| *context == word)
            .unwrap()
            .1;
        let mut forms = spellings(reading).to_vec();
        if let Some(at) = WORDS.iter().take(4).position(|(full, _)| *full == reading) {
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
fn bedding_spaces_adopted_known_tails_and_unicode_backtracking_are_lossless() {
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
    for prefix in [
        String::new(),
        "已确认 API。".to_owned(),
        "前文 café 😀。  ".repeat(40),
    ] {
        for ((&(full, word), &(_, sentences)), &(punctuation, han, pinyin)) in
            WORDS.iter().take(4).zip(SENTENCES).zip(TAILS)
        {
            let sentence = sentences[0];
            assert!(sentence.starts_with(&format!("{word}{punctuation}{han}")));
            let original = format!("{prefix}{}", full.replace('\'', " "));
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&original);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert!(ime.snapshot().committed_text.is_empty());
            for gap in ["", " ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
                for tail in [
                    han.to_owned(),
                    pinyin.to_owned(),
                    pinyin.to_ascii_uppercase(),
                ] {
                    let raw = format!("{adopted}{gap}{punctuation}{tail}");
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
fn bedding_old_bedlinen_words_authors_and_protected_boundaries_stay_intact() {
    // Complete old entries stay selectable. No arbitrary zhen/bei/chuang/tan pool freeze:
    // zhen't / bei'z / chuang'd already have 枕套 / 备注 / 床单 alternatives.
    for (reading, word) in [
        ("chuang'dan", "床单"),
        ("bei'tao", "被套"),
        ("zhen'tao", "枕套"),
        ("bei'zhu", "备注"),
        ("shui'qian'guan'deng", "睡前关灯"),
        ("zhun'bei'shui'le", "准备睡了"),
        ("tang'shi", "堂食"),
        ("tang'wan", "汤碗"),
        ("tang'shao", "汤勺"),
    ] {
        for seed in spellings(reading) {
            assert_eq!(engine(&seed).candidates()[0].text, word);
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for (reading, word, sentences) in [
        ("wan'an", "晚安", ["晚安，早点休息。", "晚安，明天见。"]),
        (
            "hai'mei'shui",
            "还没睡",
            ["还没睡，我把手头的事收个尾。", "还没睡，不过准备休息了。"],
        ),
        (
            "wo'qu'xi'shu",
            "我去洗漱",
            ["我去洗漱，等会儿回来。", "我去洗漱，你先坐一会儿。"],
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
        "bedding@example.invalid",
        "https://example.invalid/zhen/tou",
        "file_chuang_dian",
        "“raw” 123\t???",
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
        let raw = format!("zhen{line}tou fang zai chuang tou");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()) })
        );
        commit(&raw, &raw, CandidateKind::Literal);
        for &(word, sentences) in SENTENCES {
            let punctuation = if word.ends_with("吗") { "？" } else { "，" };
            let raw = format!("{word}{line}{punctuation}wo");
            let crossed = format!("{word}{line}{}", sentences[0].strip_prefix(word).unwrap());
            assert!(
                !engine(&raw)
                    .candidates()
                    .iter()
                    .any(|c| { c.text == crossed || c.text == sentences[0] })
            );
            commit(&raw, &raw, CandidateKind::Literal);
        }
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "bedding-chinese-synthetic-fallback"
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
fn bedding_pending_missing_error_timeout_and_empty_models_preserve_the_entire_local_pool() {
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
