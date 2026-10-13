//! Authored laundry, drying and clothing fallback, fixed before the resource additions.
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
    ("yi'fu'hai'mei'gan", "衣服还没干"),
    ("yi'fu'xi'hao'le", "衣服洗好了"),
    ("zhe'jian'yao'shou'xi", "这件要手洗"),
    ("shen'qian'fen'kai'xi", "深浅分开洗"),
    ("bang'wo'liang'yi'xia", "帮我晾一下"),
    ("xian'ba'wa'zi'shou'le", "先把袜子收了"),
    ("jin'tian'chuan'na'jian", "今天穿哪件"),
    ("xiu'zi'you'dian'chang", "袖子有点长"),
    ("shou'xi", "手洗"),
    ("ji'xi", "机洗"),
    ("piao'xi", "漂洗"),
    ("tuo'shui", "脱水"),
    ("hong'gan", "烘干"),
    ("xi'yi'fen", "洗衣粉"),
    ("rou'shun'ji", "柔顺剂"),
    ("liang'yi'sheng", "晾衣绳"),
    ("xi'yi'lan", "洗衣篮"),
    ("liang'shai", "晾晒"),
    ("diao'se", "掉色"),
    ("suo'shui", "缩水"),
    ("xiu'kou", "袖口"),
    ("ku'jiao", "裤脚"),
    ("mao'yi", "毛衣"),
    ("qun'zi", "裙子"),
];

// Only these fixed, independently checked stems are unambiguous here.
// shoux/jix/diaos/qunz keep their existing choices, not guaranteed new words.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("yi'fu'hai'mei'ga", "衣服还没干"),
    ("yi'fu'xi'hao'l", "衣服洗好了"),
    ("zhe'jian'yao'shou'x", "这件要手洗"),
    ("shen'qian'fen'kai'x", "深浅分开洗"),
    ("bang'wo'liang'yi'xi", "帮我晾一下"),
    ("xian'ba'wa'zi'shou'l", "先把袜子收了"),
    ("jin'tian'chuan'na'jia", "今天穿哪件"),
    ("xiu'zi'you'dian'chan", "袖子有点长"),
    ("piao'x", "漂洗"),
    ("tuo'shu", "脱水"),
    ("hong'ga", "烘干"),
    ("xi'yi'fe", "洗衣粉"),
    ("rou'shun'j", "柔顺剂"),
    ("liang'yi'shen", "晾衣绳"),
    ("xi'yi'la", "洗衣篮"),
    ("liang'sha", "晾晒"),
    ("suo'shu", "缩水"),
    ("xiu'ko", "袖口"),
    ("ku'jia", "裤脚"),
    ("mao'y", "毛衣"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "衣服还没干",
        ["衣服还没干，今天先穿别的吧。", "衣服还没干，再晾一会儿。"],
    ),
    (
        "衣服洗好了",
        ["衣服洗好了，我去拿衣架。", "衣服洗好了，帮忙一起晾吧。"],
    ),
    (
        "这件要手洗",
        ["这件要手洗，先放在旁边吧。", "这件要手洗，我晚点再洗。"],
    ),
    (
        "深浅分开洗",
        ["深浅分开洗，白色的放这一边。", "深浅分开洗，我来分一下。"],
    ),
    (
        "帮我晾一下",
        ["帮我晾一下衬衫，我去收袜子。", "帮我晾一下，衣架就在旁边。"],
    ),
    (
        "先把袜子收了",
        [
            "先把袜子收了，其他的还没干。",
            "先把袜子收了，放进抽屉里吧。",
        ],
    ),
    (
        "今天穿哪件",
        [
            "今天穿哪件？我想挑件薄一点的。",
            "今天穿哪件？这两件都挺合适。",
        ],
    ),
    (
        "袖子有点长",
        ["袖子有点长，换小一号试试吧。", "袖子有点长，穿着不太方便。"],
    ),
];

#[test]
fn laundry_preserves_all_3042_readings_727_contexts_and_1452_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3042);
    assert!(vocabulary.continuations().count() >= 727);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3042) {
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
    assert_eq!(reading_hash, 0xa2955cd3932c52d2);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(727) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1452);
    assert_eq!(context_hash, 0xa1d9d47db1e38201);
}

#[test]
fn laundry_appends_exactly_the_fixed_readings_and_context_window() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 20);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 3066);
    for (entry, &(reading, text)) in vocabulary.readings()[3042..3066].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(727)
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
fn laundry_complete_words_and_checked_specific_partials_preserve_literal_commit() {
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
fn laundry_broad_stems_preserve_older_words_and_raw_input() {
    // A legacy nonstandard alias also competes with diaos. Preserve raw input
    // without presenting that alias as a newly authored pronunciation contract.
    for seed in spellings("diao's") {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, old_word) in [("shou'x", "首先"), ("ji'x", "继续"), ("qun'z", "群组")] {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            index(&ime, old_word, CandidateKind::Word);
            commit_and_undo(&mut ime, old_word, CandidateKind::Word);
            commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn laundry_contexts_offer_both_sentences_from_full_and_specific_partial_pinyin() {
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
fn laundry_adoption_spaces_and_pinyin_tail_keep_long_frozen_text() {
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
            for tail in std::iter::once("衬衫".to_owned())
                .chain(spellings("chen'shan"))
                .chain(spellings("chen'sha"))
            {
                let seed = format!("{prefix}帮我晾一下{padding}{tail}");
                let expected = format!("{prefix}帮我晾一下{padding}衬衫，我去收袜子。");
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn laundry_invalid_boundaries_and_original_words_keep_raw_input_and_priority() {
    for seed in [
        "zzqvjy",
        "laundry@example.invalid",
        "https://example.invalid/laundry",
        "“raw”  123\t???",
        "yi fu ha i mei gan",
        "bang wo lia ng yi xia",
        "xiu zi you dia n chang",
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
        let seed = format!("帮我晾一下{boundary}chen shan");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        let crossed = format!("帮我晾一下{boundary}衬衫，我去收袜子。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("xi'yi'fu", "洗衣服"),
        ("liang'yi'fu", "晾衣服"),
        ("wa'zi", "袜子"),
        ("chen'shan", "衬衫"),
        ("yi'jia", "衣架"),
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
        "laundry-chinese-synthetic-error"
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
fn laundry_choices_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            (
                "yi fu hai mei ga",
                "衣服还没干",
                "衣服还没干，今天先穿别的吧。",
            ),
            (
                "bang wo liang yi xi",
                "帮我晾一下",
                "帮我晾一下衬衫，我去收袜子。",
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
