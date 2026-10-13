//! Authored home-fault reporting and repair appointments fallback, fixed before the resource additions.
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
    ("chuang'hu'guan'bu'shang", "窗户关不上"),
    ("men'ba'shou'song'le", "门把手松了"),
    ("chou'ti'la'bu'kai", "抽屉拉不开"),
    ("xi'yi'ji'bu'zhuan'le", "洗衣机不转了"),
    ("ma'fan'jian'cha'yi'xia", "麻烦检查一下"),
    ("ming'tian'fang'bian'shang'men'ma", "明天方便上门吗"),
    ("da'gai'yao'xiu'duo'jiu", "大概要修多久"),
    ("xiu'hao'le'gao'su'wo", "修好了告诉我"),
    ("men'ba'shou", "门把手"),
    ("men'suo", "门锁"),
    ("jiao'lian", "铰链"),
    ("suo'xin", "锁芯"),
    ("sha'chuang", "纱窗"),
    ("hua'gui", "滑轨"),
    ("mi'feng'tiao", "密封条"),
    ("wei'xiu'dan", "维修单"),
    ("bao'xiu'ji'lu", "报修记录"),
    ("shang'men'shi'jian", "上门时间"),
    ("wei'xiu'shi'fu", "维修师傅"),
    ("gu'zhang'miao'shu", "故障描述"),
    ("wei'xiu'fei'yong", "维修费用"),
    ("ling'jian", "零件"),
    ("jian'xiu", "检修"),
    ("xiu'li", "修理"),
];

// Independently checked stems, not a promise for arbitrary short prefixes.
// menbashou also begins 门把手松了; suoxi has the old 缩写 match.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("chuang'hu'guan'bu'shan", "窗户关不上"),
    ("men'ba'shou'song'l", "门把手松了"),
    ("chou'ti'la'bu'ka", "抽屉拉不开"),
    ("xi'yi'ji'bu'zhuan'l", "洗衣机不转了"),
    ("ma'fan'jian'cha'yi'xi", "麻烦检查一下"),
    ("ming'tian'fang'bian'shang'men'm", "明天方便上门吗"),
    ("da'gai'yao'xiu'duo'ji", "大概要修多久"),
    ("xiu'hao'le'gao'su'w", "修好了告诉我"),
    ("men'su", "门锁"),
    ("jiao'lia", "铰链"),
    ("sha'chuan", "纱窗"),
    ("hua'gu", "滑轨"),
    ("mi'feng'tia", "密封条"),
    ("wei'xiu'da", "维修单"),
    ("bao'xiu'ji'l", "报修记录"),
    ("shang'men'shi'jia", "上门时间"),
    ("wei'xiu'shi'f", "维修师傅"),
    ("gu'zhang'miao'sh", "故障描述"),
    ("wei'xiu'fei'yon", "维修费用"),
    ("ling'jia", "零件"),
    ("jian'xi", "检修"),
    ("xiu'l", "修理"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "窗户关不上",
        [
            "窗户关不上，想约师傅过来看看。",
            "窗户关不上，这个问题有几天了。",
        ],
    ),
    (
        "门把手松了",
        [
            "门把手松了，麻烦安排人来检查。",
            "门把手松了，开关门时总晃。",
        ],
    ),
    (
        "抽屉拉不开",
        [
            "抽屉拉不开，能帮忙看一下吗？",
            "抽屉拉不开，里面还有东西没拿。",
        ],
    ),
    (
        "洗衣机不转了",
        [
            "洗衣机不转了，想问问什么时候能来检查。",
            "洗衣机不转了，已经提交报修了。",
        ],
    ),
    (
        "麻烦检查一下",
        [
            "麻烦检查一下门锁，最近不太好开。",
            "麻烦检查一下，看看问题出在哪里。",
        ],
    ),
    (
        "明天方便上门吗",
        [
            "明天方便上门吗？我下午都在家。",
            "明天方便上门吗？时间可以再商量。",
        ],
    ),
    (
        "大概要修多久",
        [
            "大概要修多久？我想安排一下时间。",
            "大概要修多久？需要等配件吗？",
        ],
    ),
    (
        "修好了告诉我",
        ["修好了告诉我，我再过去拿。", "修好了告诉我，我好安排时间。"],
    ),
];

#[test]
fn repairs_preserves_all_3018_readings_719_contexts_and_1436_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3018);
    assert!(vocabulary.continuations().count() >= 719);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3018) {
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
    assert_eq!(reading_hash, 0x12b2f1db6e8f1b61);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(719) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1436);
    assert_eq!(context_hash, 0x0fe6890fc3ff37eb);
}

#[test]
fn repairs_appends_exactly_the_fixed_readings_and_context_window() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 22);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 3042);
    for (entry, &(reading, text)) in vocabulary.readings()[3018..3042].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(719)
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
fn repairs_complete_words_and_checked_specific_partials_preserve_literal_commit() {
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
fn repairs_broad_stems_preserve_older_words_and_raw_input() {
    for seed in spellings("men'ba'sho") {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for seed in spellings("suo'xi") {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        index(&ime, "缩写", CandidateKind::Word);
        commit_and_undo(&mut ime, "缩写", CandidateKind::Word);
        commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn repairs_contexts_offer_both_sentences_from_full_and_specific_partial_pinyin() {
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
fn repairs_adoption_spaces_and_pinyin_tail_keep_long_frozen_text() {
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
            for tail in std::iter::once("门锁".to_owned())
                .chain(spellings("men'suo"))
                .chain(spellings("men'su"))
            {
                let seed = format!("{prefix}麻烦检查一下{padding}{tail}");
                let expected = format!("{prefix}麻烦检查一下{padding}门锁，最近不太好开。");
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn repairs_invalid_boundaries_and_original_words_keep_raw_input_and_priority() {
    for seed in [
        "zzqvjy",
        "repairs@example.invalid",
        "https://example.invalid/repair",
        "“raw”  123\t???",
        "chu ang hu guan bu shang",
        "men ba sho u song le",
        "ma fan jian cha yi xi a",
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
        let seed = format!("麻烦检查一下{boundary}men suo");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        let crossed = format!("麻烦检查一下{boundary}门锁，最近不太好开。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("wei'xiu", "维修"),
        ("yu'yue", "预约"),
        ("bao'xiu", "保修"),
        ("chuang'hu", "窗户"),
        ("chou'ti", "抽屉"),
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
    for seed in spellings("bao'xiu") {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            index(&ime, "保修", CandidateKind::Word) < index(&ime, "报修", CandidateKind::Word)
        );
        commit_and_undo(&mut ime, "报修", CandidateKind::Word);
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "repairs-chinese-synthetic-error"
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
fn repairs_choices_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "chuang hu guan bu shan",
                "窗户关不上",
                "窗户关不上，想约师傅过来看看。",
            ),
            (
                "ma fan jian cha yi xi",
                "麻烦检查一下",
                "麻烦检查一下门锁，最近不太好开。",
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
