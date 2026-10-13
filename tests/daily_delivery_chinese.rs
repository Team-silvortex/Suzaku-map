//! Authored parcel collection fallback, fixed before the resource additions.
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
    ("kuai'di'dao'le", "快递到了"),
    ("bang'wo'shou'yi'xia", "帮我收一下"),
    ("fang'men'kou'jiu'xing", "放门口就行"),
    ("wo'xia'lou'qu", "我下楼取"),
    ("bao'guo'hai'mei'dao", "包裹还没到"),
    ("bao'zhuang'po'le", "包装破了"),
    ("dao'le'da'dian'hua", "到了打电话"),
    ("wo'lai'na'bao'guo", "我来拿包裹"),
    ("dai'shou'dian", "代收点"),
    ("qu'jian'tong'zhi", "取件通知"),
    ("song'jian", "送件"),
    ("lan'shou", "揽收"),
    ("yun'dan", "运单"),
    ("mian'dan", "面单"),
    ("zhi'xiang", "纸箱"),
    ("feng'xiang'jiao'dai", "封箱胶带"),
    ("bao'zhuang'he", "包装盒"),
    ("chai'kuai'di", "拆快递"),
    ("yan'huo", "验货"),
    ("jian'shu", "件数"),
    ("huo'jia", "货架"),
    ("kuai'di'dai", "快递袋"),
    ("shang'men'qu'jian", "上门取件"),
    ("qu'jian'ping'zheng", "取件凭证"),
];

// These fixed stems were checked against the old and proposed vocabulary.
// mianda and kuaidida remain broad stems; new words must not evict old matches.
const SPECIFIC_PARTIALS: &[(&str, &str)] = &[
    ("kuai'di'dao'l", "快递到了"),
    ("bang'wo'shou'yi'xi", "帮我收一下"),
    ("fang'men'kou'jiu'xin", "放门口就行"),
    ("wo'xia'lou'q", "我下楼取"),
    ("bao'guo'hai'mei'da", "包裹还没到"),
    ("bao'zhuang'po'l", "包装破了"),
    ("dao'le'da'dian'hu", "到了打电话"),
    ("wo'lai'na'bao'gu", "我来拿包裹"),
    ("dai'shou'dia", "代收点"),
    ("qu'jian'tong'zh", "取件通知"),
    ("song'jia", "送件"),
    ("lan'sho", "揽收"),
    ("yun'da", "运单"),
    ("zhi'xian", "纸箱"),
    ("feng'xiang'jiao'da", "封箱胶带"),
    ("bao'zhuang'h", "包装盒"),
    ("chai'kuai'd", "拆快递"),
    ("yan'hu", "验货"),
    ("jian'sh", "件数"),
    ("huo'ji", "货架"),
    ("shang'men'qu'jia", "上门取件"),
    ("qu'jian'ping'zhen", "取件凭证"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "快递到了",
        ["快递到了，方便帮我拿一下吗？", "快递到了，我晚点去取。"],
    ),
    (
        "帮我收一下",
        ["帮我收一下，我还在路上。", "帮我收一下，回来我去找你拿。"],
    ),
    (
        "放门口就行",
        ["放门口就行，我马上过来拿。", "放门口就行，谢谢你跑一趟。"],
    ),
    (
        "我下楼取",
        ["我下楼取快递，马上就回来。", "我下楼取，麻烦等我两分钟。"],
    ),
    (
        "包裹还没到",
        ["包裹还没到，我再查一下物流。", "包裹还没到，到了我告诉你。"],
    ),
    (
        "包装破了",
        ["包装破了，里面的东西还完好吗？", "包装破了，我先拍张照片。"],
    ),
    (
        "到了打电话",
        ["到了打电话，我下来拿。", "到了打电话，别放在外面。"],
    ),
    (
        "我来拿包裹",
        [
            "我来拿包裹，取件码在手机里。",
            "我来拿包裹，麻烦帮我找一下。",
        ],
    ),
];

#[test]
fn delivery_preserves_all_2994_readings_711_contexts_and_1420_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2994);
    assert!(vocabulary.continuations().count() >= 711);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2994) {
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
    assert_eq!(reading_hash, 0x568ae01a05b4869c);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(711) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1420);
    assert_eq!(context_hash, 0x2baacbdef011542d);
}

#[test]
fn delivery_appends_exactly_the_fixed_readings_and_context_window() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SPECIFIC_PARTIALS.len(), 22);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 3018);
    for (entry, &(reading, text)) in vocabulary.readings()[2994..3018].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(711)
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
fn delivery_complete_words_and_checked_specific_partials_preserve_literal_commit() {
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
fn delivery_broad_stems_preserve_older_words_and_raw_input() {
    for (reading, old_word) in [("mian'da", "免打扰"), ("kuai'di'da", "快递单号")] {
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
fn delivery_contexts_offer_both_sentences_from_full_and_specific_partial_pinyin() {
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
fn delivery_adoption_spaces_and_pinyin_tail_keep_long_frozen_text() {
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
            for tail in std::iter::once("快递".to_owned())
                .chain(spellings("kuai'di"))
                .chain(spellings("kuai'd"))
            {
                let seed = format!("{prefix}我下楼取{padding}{tail}");
                let expected = format!("{prefix}我下楼取{padding}快递，马上就回来。");
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn delivery_invalid_boundaries_and_original_words_keep_raw_input_and_priority() {
    for seed in [
        "zzqvjy",
        "delivery@example.invalid",
        "https://example.invalid/parcel",
        "“raw”  123\t???",
        "ku ai di dao le",
        "bang wo sho u yi xia",
        "bao gu o hai mei dao",
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
        let seed = format!("我下楼取{boundary}kuai di");
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        let crossed = format!("我下楼取{boundary}快递，马上就回来。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("kuai'di", "快递"),
        ("bao'guo", "包裹"),
        ("qu'jian", "取件"),
        ("qian'shou", "签收"),
        ("dai'shou", "代收"),
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
        "delivery-chinese-synthetic-error"
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
fn delivery_choices_survive_missing_failed_timeout_and_empty_models() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("kuai di dao l", "快递到了", "快递到了，方便帮我拿一下吗？"),
            ("wo xia lou q", "我下楼取", "我下楼取快递，马上就回来。"),
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
