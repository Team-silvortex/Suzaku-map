//! Finite kitchen coordination and table clearing, not recipes or health advice.
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
    "1023a1295cd0180296ee0ff281f113b7e11ff5bfa9c52370152802de46988044";
const WORDS: &[(&str, &str)] = &[
    ("wo'xian'ba'cai'xi'hao", "我先把菜洗好"),
    ("qie'hao'de'cai'fang'zhe'bian", "切好的菜放这边"),
    ("bang'wo'na'yi'xia'cai'ban", "帮我拿一下菜板"),
    ("pan'zi'zai'na'liang'ge", "盘子再拿两个"),
    ("chi'wan'wo'lai'shou'zhuo'zi", "吃完我来收桌子"),
    ("xi'hao'de'wan'fang'zhe'bian", "洗好的碗放这边"),
    ("cai'ban", "菜板"),
    ("guo'chan", "锅铲"),
    ("pan'zi", "盘子"),
    ("die'zi", "碟子"),
    ("fan'wan", "饭碗"),
    ("kuai'jia", "筷架"),
];
const TONES: &[&str] = &[
    "wo3'xian1'ba3'cai4'xi3'hao3",
    "qie1'hao3'de5'cai4'fang4'zhe4'bian1",
    "bang1'wo3'na2'yi1'xia4'cai4'ban3",
    "pan2'zi5'zai4'na2'liang3'ge4",
    "chi1'wan2'wo3'lai2'shou1'zhuo1'zi5",
    "xi3'hao3'de5'wan3'fang4'zhe4'bian1",
    "cai4'ban3",
    "guo1'chan3",
    "pan2'zi5",
    "die2'zi5",
    "fan4'wan3",
    "kuai4'jia4",
];
// Only these six near-complete spellings, not arbitrary short-prefix predictions.
const PARTIALS: &[(&str, &str)] = &[
    ("wo'xian'ba'cai'xi'ha", "我先把菜洗好"),
    ("qie'hao'de'cai'fang'zhe'bia", "切好的菜放这边"),
    ("bang'wo'na'yi'xia'cai'ba", "帮我拿一下菜板"),
    ("pan'zi'zai'na'liang'g", "盘子再拿两个"),
    ("chi'wan'wo'lai'shou'zhuo'z", "吃完我来收桌子"),
    ("xi'hao'de'wan'fang'zhe'bia", "洗好的碗放这边"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "我先把菜洗好",
        [
            "我先把菜洗好，你把盘子拿出来。",
            "我先把菜洗好，洗完再叫你过来。",
        ],
    ),
    (
        "切好的菜放这边",
        [
            "切好的菜放这边，我等会儿来拿。",
            "切好的菜放这边，空盘子放另一边。",
        ],
    ),
    (
        "帮我拿一下菜板",
        [
            "帮我拿一下菜板，我腾不出手。",
            "帮我拿一下菜板，就在下面的柜子里。",
        ],
    ),
    (
        "盘子再拿两个",
        [
            "盘子再拿两个，还有两个人没拿到。",
            "盘子再拿两个，我来摆好。",
        ],
    ),
    (
        "吃完我来收桌子",
        [
            "吃完我来收桌子，你先坐着休息。",
            "吃完我来收桌子，碗筷留给我就好。",
        ],
    ),
    (
        "洗好的碗放这边",
        [
            "洗好的碗放这边，我来擦干。",
            "洗好的碗放这边，柜子里还有空位。",
        ],
    ),
];
// First authored suffixes use only existing independent Han/Pinyin mappings.
const TAILS: &[(&str, &str)] = &[
    ("你", "ni"),
    ("我", "wo"),
    ("我", "wo"),
    ("还", "hai"),
    ("你", "ni"),
    ("我", "wo"),
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
fn kitchen_preserves_all_3390_readings_flags_and_851_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3390);
    assert!(vocabulary.continuations().count() >= 851);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3390) {
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
    assert_eq!(reading_hash, 0x5c1b0e8d7dc75b7d);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(851) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1700);
    assert_eq!(context_hash, 0x96b352b2be075895);
    // Exact trailing-append inverse; the independent receipt also locks SHA-256.
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if vocabulary.readings().len() > 3390 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"wo'xian'ba'cai'xi'hao\", \"text\": \"我先把菜洗好\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"我先把菜洗好\",", "\n  ]\n}"),
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
        0xf94907b64887e6ac,
        "before resource SHA-256 receipt: {BEFORE_RESOURCE_SHA256}"
    );
}

#[test]
fn kitchen_appends_only_twelve_words_and_six_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (12, 6, 6));
    assert_eq!(vocabulary.readings().len(), 3410);
    assert_eq!(vocabulary.continuations().count(), 861);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, vs)| vs.len())
            .sum::<usize>(),
        1720
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3390..3402].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(851)
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
fn kitchen_full_words_tones_and_six_specific_partials_offer_exact_choices() {
    assert_eq!(TONES.len(), WORDS.len());
    for (&(reading, word), &tone) in WORDS.iter().zip(TONES) {
        // Tolerant tone-digit normalization, not tone-conditioned ranking.
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
fn kitchen_spaces_adopted_known_tails_and_unicode_backtracking_are_lossless() {
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
        for ((&(full, word), &(_, sentences)), &(han, pinyin)) in
            WORDS.iter().take(6).zip(SENTENCES).zip(TAILS)
        {
            let sentence = sentences[0];
            assert!(sentence.starts_with(&format!("{word}，{han}")));
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
fn kitchen_existing_mealtime_words_authors_homophones_and_protected_boundaries_stay_intact() {
    for (reading, word) in [
        ("xi", "西"),
        ("xi'wan", "洗碗"),
        ("kuai'jie'jian", "快捷键"),
    ] {
        for seed in spellings(reading) {
            assert_eq!(engine(&seed).candidates()[0].text, word);
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for seed in ["guo", "GUO", "guo2", "GUO4"] {
        let ime = engine(seed);
        assert_eq!(ime.candidates()[0].text, "国");
        assert!(index(&ime, "国", CandidateKind::Word) < index(&ime, "过", CandidateKind::Word));
        commit(seed, "过", CandidateKind::Word);
        commit(seed, seed, CandidateKind::Literal);
    }
    // kuai'ji may gain 筷架; protect the existing choice, not pool size/rank equality.
    for seed in spellings("kuai'ji") {
        index(&engine(&seed), "快捷键", CandidateKind::Word);
        commit(&seed, &seed, CandidateKind::Literal);
    }
    for (reading, word, sentences) in [
        (
            "wo'lai'xi'wan",
            "我来洗碗",
            ["我来洗碗，你把桌子擦一下。", "我来洗碗，洗完就过去找你。"],
        ),
        (
            "wan'xi'hao'le",
            "碗洗好了",
            [
                "碗洗好了，等干了再放进柜子。",
                "碗洗好了，我接着擦一下水槽。",
            ],
        ),
        (
            "ba'wan'kuai'bai'hao",
            "把碗筷摆好",
            ["把碗筷摆好，我们准备吃饭。", "把碗筷摆好，汤也端上来了。"],
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
        "kitchen@example.invalid",
        "https://example.invalid/wo/xian/ba",
        "file_pan_zi",
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
        let raw = format!("wo{line}xian ba cai xi hao");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()) })
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("我先把菜洗好{line}，ni");
        let crossed = format!("我先把菜洗好{line}，你把盘子拿出来。");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| { c.text == crossed || c.text == SENTENCES[0].1[0] })
        );
        commit(&raw, &raw, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "kitchen-chinese-synthetic-fallback"
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
fn kitchen_pending_missing_error_timeout_and_empty_models_preserve_the_entire_local_pool() {
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
