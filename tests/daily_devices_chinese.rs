//! Authored everyday device vocabulary, fixed before the resource additions.
//! These isolated engine checks neither inspect nor control real devices.
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
    ("shou'ji'kuai'mei'dian'le", "手机快没电了"),
    ("wo'xian'chong'hui'er'dian", "我先充会儿电"),
    ("er'ji'lian'shang'le", "耳机连上了"),
    ("sheng'yin'you'dian'xiao", "声音有点小"),
    ("ping'mu'tai'liang'le", "屏幕太亮了"),
    ("chong'dian'qi'wang'dai'le", "充电器忘带了"),
    ("dian'liang'hai'gou'yong", "电量还够用"),
    ("er'ji'fang'he'li'le", "耳机放盒里了"),
    ("wu'xian'er'ji", "无线耳机"),
    ("you'xian'er'ji", "有线耳机"),
    ("er'ji'he", "耳机盒"),
    ("er'sai", "耳塞"),
    ("yin'liang'jian", "音量键"),
    ("dian'yuan'jian", "电源键"),
    ("chong'dian'kou", "充电口"),
    ("chong'dian'cha'tou", "充电插头"),
    ("shou'ji'zhi'jia", "手机支架"),
    ("shou'ji'ke", "手机壳"),
    ("gang'hua'mo", "钢化膜"),
    ("zhuan'jie'tou", "转接头"),
    ("chong'dian'di'zuo", "充电底座"),
    ("sheng'yu'dian'liang", "剩余电量"),
    ("dian'liang'bai'fen'bi", "电量百分比"),
    ("ling'sheng'yin'liang", "铃声音量"),
];

// Deliberately narrow authored stems; broad prefixes keep their old ranking.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("shou'ji'kuai'mei'dian'l", "手机快没电了"),
    ("wo'xian'chong'hui'er'dia", "我先充会儿电"),
    ("er'ji'lian'shang'l", "耳机连上了"),
    ("sheng'yin'you'dian'xia", "声音有点小"),
    ("ping'mu'tai'liang'l", "屏幕太亮了"),
    ("chong'dian'qi'wang'dai'l", "充电器忘带了"),
    ("dian'liang'hai'gou'yon", "电量还够用"),
    ("er'ji'fang'he'li'l", "耳机放盒里了"),
];

const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "手机快没电了",
        [
            "手机快没电了，我晚点再回复你。",
            "手机快没电了，充电线在包里吗？",
        ],
    ),
    (
        "我先充会儿电",
        [
            "我先充会儿电再出门，你不用等我。",
            "我先充会儿电，等一下就回来。",
        ],
    ),
    (
        "耳机连上了",
        [
            "耳机连上了手机，现在听得很清楚。",
            "耳机连上了，声音现在正常了。",
        ],
    ),
    (
        "声音有点小",
        ["声音有点小，我这边听不清。", "声音有点小，你能听见吗？"],
    ),
    (
        "屏幕太亮了",
        [
            "屏幕太亮了，我想调暗一点。",
            "屏幕太亮了，晚上看着有点晃眼。",
        ],
    ),
    (
        "充电器忘带了",
        [
            "充电器忘带了，今天能借你的用一下吗？",
            "充电器忘带了，我回去拿一下。",
        ],
    ),
    (
        "电量还够用",
        ["电量还够用，等回家再充吧。", "电量还够用，出门前刚充过。"],
    ),
    (
        "耳机放盒里了",
        [
            "耳机放盒里了，等会儿出门再拿。",
            "耳机放盒里了，就在桌子旁边。",
        ],
    ),
];

#[test]
fn devices_preserve_all_3066_readings_735_contexts_and_1468_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3066);
    assert!(vocabulary.continuations().count() >= 735);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3066) {
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
    assert_eq!(reading_hash, 0x907be4481adf8936);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(735) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1468);
    assert_eq!(context_hash, 0x6cef6b4fb0c4a875);
}

#[test]
fn devices_append_the_fixed_twenty_four_readings_and_eight_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 8);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 3090);
    for (entry, &(reading, text)) in vocabulary.readings()[3066..3090].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(735)
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

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    ime
}

fn spellings(reading: &str) -> [String; 5] {
    [
        reading.replace('\'', ""),
        reading.replace('\'', " "),
        reading.into(),
        reading.replace('\'', "\t"),
        reading.replace('\'', " ").to_ascii_uppercase(),
    ]
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
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
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let index = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if kind == CandidateKind::Word {
        assert!(index < PAGE_SIZE);
    }
    if kind == CandidateKind::Sentence {
        assert!(index < 2 * PAGE_SIZE);
    }
    index
}

fn commit_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn device_words_and_specific_partials_preserve_exact_selection_and_literal() {
    for &(reading, word) in WORDS.iter().chain(SPECIFIC_PARTIALS) {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            commit_and_undo(&mut ime, word, CandidateKind::Word);
            commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn device_contexts_offer_both_sentences_from_full_and_specific_partial_pinyin() {
    for ((&(reading, word), &(partial, partial_word)), &(context, sentences)) in
        WORDS.iter().zip(SPECIFIC_PARTIALS).zip(SENTENCES)
    {
        assert_eq!((word, partial_word), (context, context));
        for fragment in [reading, partial] {
            for seed in spellings(fragment) {
                let ime = engine(&seed);
                draft(&ime, &seed);
                index(&ime, word, CandidateKind::Word);
                for sentence in sentences {
                    commit_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn device_adoption_spaces_and_pinyin_tail_keep_long_frozen_text() {
    for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
        for padding in ["", " ", "\t", "\u{a0}", "\u{3000}"] {
            let mut ime = engine(reading);
            ime.select_candidate(index(&ime, word, CandidateKind::Word));
            let adopted = ime.selected_completion_text(true).unwrap().to_owned();
            assert_eq!(adopted, word);
            assert!(ime.snapshot().committed_text.is_empty());
            // Host-like reseeding, not physical-keyboard integration evidence.
            let seed = format!("{adopted}{padding}");
            ime.seed(&seed);
            draft(&ime, &seed);
            for sentence in sentences {
                let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                commit_and_undo(&mut engine(&seed), &expected, CandidateKind::Sentence);
            }
            commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", "  ", "\t", "\u{3000}"] {
            for tail in std::iter::once("手机".to_owned())
                .chain(spellings("shou'ji"))
                .chain(spellings("shou'j"))
            {
                let seed = format!("{prefix}耳机连上了{padding}{tail}");
                let expected = format!("{prefix}耳机连上了{padding}手机，现在听得很清楚。");
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn device_invalid_boundaries_and_older_words_preserve_raw_input_and_priority() {
    for seed in [
        "zzqvjy",
        "devices@example.invalid",
        "https://example.invalid/devices",
        "“raw”  123\t???",
        "shou ji ku ai mei dian le",
        "er ji lia n shang le",
        "ping mu ta i liang le",
    ] {
        let mut ime = engine(seed);
        draft(&ime, seed);
        assert!(!ime.candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.contains(&c.text.as_str()))
        }));
        commit_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("耳机连上了{boundary}shou ji");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        let crossed = format!("耳机连上了{boundary}手机，现在听得很清楚。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("shou'ji", "手机"),
        ("chong'dian'qi", "充电器"),
        ("er'ji", "耳机"),
        ("yin'liang", "音量"),
        ("ping'mu", "屏幕"),
        ("dian'liang", "电量"),
        ("hai", "还"),
        ("hou", "后"),
    ] {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            assert_eq!(ime.candidates()[0].text, word);
            commit_and_undo(&mut ime, word, CandidateKind::Word);
        }
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "devices-chinese-synthetic-error"
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
fn device_choices_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "shou ji kuai mei dian l",
                "手机快没电了",
                "手机快没电了，我晚点再回复你。",
            ),
            (
                "er ji lian shang l",
                "耳机连上了",
                "耳机连上了手机，现在听得很清楚。",
            ),
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
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
            commit_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}
