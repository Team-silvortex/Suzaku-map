//! Authored household chores and cleaning fallback, fixed before data additions.
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
    ("wo'lai'xi'wan", "我来洗碗"),
    ("ca'yi'xia'zhuo'zi", "擦一下桌子"),
    ("la'ji'dai'man'le", "垃圾袋满了"),
    ("di'shang'you'dian'shi", "地上有点湿"),
    ("xian'ba'la'ji'reng'le", "先把垃圾扔了"),
    ("wan'xi'hao'le", "碗洗好了"),
    ("ba'yi'fu'shou'qi'lai", "把衣服收起来"),
    ("liang'zai'yang'tai'shang", "晾在阳台上"),
    ("la'ji'tong", "垃圾桶"),
    ("bo'ji", "簸箕"),
    ("shui'tong", "水桶"),
    ("hai'mian", "海绵"),
    ("bai'jie'bu", "百洁布"),
    ("xiang'jiao'shou'tao", "橡胶手套"),
    ("liang'yi'jia", "晾衣架"),
    ("yi'jia", "衣夹"),
    ("xi'chen'qi", "吸尘器"),
    ("sao'di'ji", "扫地机"),
    ("qing'jie'shua", "清洁刷"),
    ("qu'wu", "去污"),
    ("ning'gan", "拧干"),
    ("ca'gan", "擦干"),
    ("qing'li'shui'cao", "清理水槽"),
    ("huan'la'ji'dai", "换垃圾袋"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "我来洗碗",
        ["我来洗碗，你把桌子擦一下。", "我来洗碗，洗完就过去找你。"],
    ),
    (
        "擦一下桌子",
        ["擦一下桌子，杯子先放到旁边。", "擦一下桌子，刚才洒了点水。"],
    ),
    (
        "垃圾袋满了",
        [
            "垃圾袋满了，换一个新的吧。",
            "垃圾袋满了，我出门时顺手带下去。",
        ],
    ),
    (
        "地上有点湿",
        [
            "地上有点湿，走过来时慢一点。",
            "地上有点湿，我拿拖把擦一下。",
        ],
    ),
    (
        "先把垃圾扔了",
        [
            "先把垃圾扔了，回来再整理厨房。",
            "先把垃圾扔了，袋子放在门边。",
        ],
    ),
    (
        "碗洗好了",
        [
            "碗洗好了，等干了再放进柜子。",
            "碗洗好了，我接着擦一下水槽。",
        ],
    ),
    (
        "把衣服收起来",
        [
            "把衣服收起来放进抽屉，椅子上就空出来了。",
            "把衣服收起来，留出地方晾毛巾。",
        ],
    ),
    (
        "晾在阳台上",
        [
            "晾在阳台上，干了以后我来收。",
            "晾在阳台上，衣架之间留点空隙。",
        ],
    ),
];

#[test]
fn household_preserves_all_2922_readings_687_contexts_and_1372_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2922);
    assert!(vocabulary.continuations().count() >= 687);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2922) {
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
    assert_eq!(reading_hash, 0x4ac126d078fe6bad);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(687) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1372);
    assert_eq!(context_hash, 0x06df73a06ff2be69);
}

#[test]
fn household_appends_exactly_the_fixed_readings_and_context_window() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 2946);
    for (entry, &(reading, text)) in vocabulary.readings()[2922..2946].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(687)
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

fn assert_local_draft(ime: &XRTabletImeEngine, seed: &str) {
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
    // Keep useful words on page one; authored sentences remain selectable
    // within the existing two pages when older conversions compete.
    if kind == CandidateKind::Word {
        assert!(index < PAGE_SIZE);
    }
    if kind == CandidateKind::Sentence {
        assert!(index < 2 * PAGE_SIZE);
    }
    index
}

fn commit_and_undo(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let candidate = index(ime, text, kind);
    ime.select_candidate(candidate);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok);
    assert_eq!(second.text, None);
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn household_full_and_unfinished_pinyin_words_keep_literal_and_exact_commit() {
    for &(reading, word) in WORDS {
        for fragment in [reading, &reading[..reading.len() - 1]] {
            for seed in spellings(fragment) {
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_and_undo(&mut ime, word, CandidateKind::Word);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn household_specific_pinyin_offers_word_and_both_authored_sentence_choices() {
    for (&(reading, word), &(context, sentences)) in WORDS.iter().zip(SENTENCES) {
        assert_eq!(word, context);
        for fragment in [reading, &reading[..reading.len() - 1]] {
            for seed in spellings(fragment) {
                let ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                index(&ime, word, CandidateKind::Word);
                for sentence in sentences {
                    commit_and_undo(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
            }
        }
    }
}

#[test]
fn household_adoption_and_following_pinyin_preserve_padding_and_long_frozen_text() {
    for (&(reading, word), &(_, sentences)) in WORDS.iter().zip(SENTENCES) {
        for padding in ["", " ", "\t", "\u{a0}", "\u{3000}"] {
            let mut ime = engine(reading);
            ime.select_candidate(index(&ime, word, CandidateKind::Word));
            let adopted = ime.selected_completion_text(true).unwrap().to_owned();
            assert_eq!(adopted, word);
            assert!(ime.snapshot().committed_text.is_empty());
            let seed = format!("{adopted}{padding}");
            ime.seed(&seed);
            assert_local_draft(&ime, &seed);
            for sentence in sentences {
                let expected = format!("{seed}{}", sentence.strip_prefix(word).unwrap());
                commit_and_undo(&mut engine(&seed), &expected, CandidateKind::Sentence);
            }
            commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
        }
    }
    for prefix in [String::new(), "前文 café 😀。  ".repeat(40)] {
        for padding in ["", "  ", "\t", "\u{3000}"] {
            for tail in std::iter::once("放进抽屉".to_owned())
                .chain(spellings("fang'jin'chou'ti"))
                .chain(spellings("fang'jin'chou't"))
            {
                let seed = format!("{prefix}把衣服收起来{padding}{tail}");
                let expected = format!("{prefix}把衣服收起来{padding}放进抽屉，椅子上就空出来了。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn household_keeps_invalid_boundaries_raw_literals_and_existing_housework_words() {
    for seed in [
        "zzqvjy",
        "household@example.invalid",
        "https://example.invalid/clean",
        "“raw”  123\t???",
        "wo lai xi wa n",
        "la ji da i man le",
        "di sha ng you dian shi",
    ] {
        let mut ime = engine(seed);
        assert_local_draft(&ime, seed);
        assert!(!ime.candidates().iter().any(|c| {
            SENTENCES
                .iter()
                .any(|(_, values)| values.contains(&c.text.as_str()))
        }));
        commit_and_undo(&mut ime, seed, CandidateKind::Literal);
    }
    for boundary in [
        "\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("把衣服收起来{boundary}fang jin chou ti");
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        let crossed = format!("把衣服收起来{boundary}放进抽屉，椅子上就空出来了。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("xi'wan", "洗碗"),
        ("ca'zhuo'zi", "擦桌子"),
        ("la'ji'dai", "垃圾袋"),
        ("liang'yi'fu", "晾衣服"),
        ("yi'jia", "衣架"),
        ("hai", "还"),
        ("hou", "后"),
    ] {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            assert_eq!(ime.candidates()[0].text, word);
            commit_and_undo(&mut ime, word, CandidateKind::Word);
        }
    }
}

struct UnavailableProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for UnavailableProvider {
    fn provider_id(&self) -> &str {
        "household-chinese-synthetic-error"
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
fn household_choices_survive_missing_failed_timeout_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("wo lai xi wan", "我来洗碗", "我来洗碗，你把桌子擦一下。"),
            (
                "la ji dai man l",
                "垃圾袋满了",
                "垃圾袋满了，换一个新的吧。",
            ),
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(UnavailableProvider(error.clone()))));
            ime.seed(seed);
            assert_local_draft(&ime, seed);
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
            assert_local_draft(&ime, seed);
            commit_and_undo(&mut ime, sentence, CandidateKind::Sentence);
        }
    }
}
