//! Authored visits and hosting fallback, fixed before data additions.
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
    ("men'kai'zhe'ne", "门开着呢"),
    ("jin'lai'zuo'ba", "进来坐吧"),
    ("tuo'xie'zai'men'kou", "拖鞋在门口"),
    ("wo'gei'ni'dao'shui", "我给你倒水"),
    ("sui'bian'zuo", "随便坐"),
    ("bie'zhan'zai'men'kou", "别站在门口"),
    ("zai'zuo'yi'hui'er", "再坐一会儿"),
    ("wo'song'ni'chu'qu", "我送你出去"),
    ("qing'jin", "请进"),
    ("qing'zuo", "请坐"),
    ("jin'wu", "进屋"),
    ("qiao'men", "敲门"),
    ("xuan'guan", "玄关"),
    ("men'dian", "门垫"),
    ("xie'gui", "鞋柜"),
    ("cha'ji", "茶几"),
    ("cha'bei", "茶杯"),
    ("cha'hu", "茶壶"),
    ("dao'cha", "倒茶"),
    ("dao'shui", "倒水"),
    ("zuo'ke", "做客"),
    ("chuan'men", "串门"),
    ("liu'bu", "留步"),
    ("man'zou", "慢走"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "门开着呢",
        ["门开着呢，直接进来就好。", "门开着呢，我在客厅等你。"],
    ),
    (
        "进来坐吧",
        ["进来坐吧，外面有点冷。", "进来坐吧，我刚把茶泡好。"],
    ),
    (
        "拖鞋在门口",
        [
            "拖鞋在门口，换好再进来吧。",
            "拖鞋在门口，穿那双灰色的就行。",
        ],
    ),
    (
        "我给你倒水",
        [
            "我给你倒水，你先坐一会儿。",
            "我给你倒水，想喝热的还是凉的？",
        ],
    ),
    (
        "随便坐",
        [
            "随便坐一会儿，我去切点水果。",
            "随便坐，东西放在茶几上就好。",
        ],
    ),
    (
        "别站在门口",
        ["别站在门口，进屋慢慢聊。", "别站在门口，里面还有空位。"],
    ),
    (
        "再坐一会儿",
        [
            "再坐一会儿，我们还没聊完呢。",
            "再坐一会儿，等雨小一点再走。",
        ],
    ),
    (
        "我送你出去",
        ["我送你出去，电梯就在右手边。", "我送你出去，下次有空再来。"],
    ),
];

#[test]
fn visits_preserves_all_2970_readings_703_contexts_and_1404_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2970);
    assert!(vocabulary.continuations().count() >= 703);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2970) {
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
    assert_eq!(reading_hash, 0x7d8dcb49dba18271);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(703) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1404);
    assert_eq!(context_hash, 0x965e1dd9693caaff);
}

#[test]
fn visits_appends_exactly_the_fixed_readings_and_context_window() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 2994);
    for (entry, &(reading, text)) in vocabulary.readings()[2970..2994].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(703)
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
fn visits_full_and_specific_unfinished_pinyin_keep_literal_and_exact_commit() {
    for &(reading, word) in WORDS {
        for fragment in [reading, &reading[..reading.len() - 1]] {
            // These five short stems already match older words. In particular,
            // qingji has eight older matches before 请进; do not evict them or
            // enlarge the candidate budget to promise every unfinished word.
            if fragment != reading && ["请进", "进屋", "门垫", "茶几", "倒茶"].contains(&word)
            {
                continue;
            }
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
fn visits_broad_stems_keep_old_choices_and_exact_literal_input() {
    for (reading, old_word) in [
        ("qing'ji", "请假"),
        ("jin'w", "今晚"),
        ("men'dia", "门店"),
        ("cha'j", "插件"),
        ("dao'ch", "导出"),
    ] {
        for seed in spellings(reading) {
            let mut ime = engine(&seed);
            assert_local_draft(&ime, &seed);
            index(&ime, old_word, CandidateKind::Word);
            commit_and_undo(&mut ime, old_word, CandidateKind::Word);
            commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn visits_specific_pinyin_offers_word_and_both_authored_sentence_choices() {
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
fn visits_adoption_and_following_pinyin_preserve_padding_and_long_frozen_text() {
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
            for tail in std::iter::once("一会儿".to_owned())
                .chain(spellings("yi'hui'er"))
                .chain(spellings("yi'hui'e"))
            {
                let seed = format!("{prefix}随便坐{padding}{tail}");
                let expected = format!("{prefix}随便坐{padding}一会儿，我去切点水果。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn visits_keeps_invalid_boundaries_raw_literals_and_existing_welcome_words() {
    for seed in [
        "zzqvjy",
        "visits@example.invalid",
        "https://example.invalid/visit",
        "“raw”  123\t???",
        "men ka i zhe ne",
        "tuo xi e zai men kou",
        "sui bia n zuo",
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
        let seed = format!("随便坐{boundary}yi hui er");
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        let crossed = format!("随便坐{boundary}一会儿，我去切点水果。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("kai'men", "开门"),
        ("men'ling", "门铃"),
        ("tuo'xie", "拖鞋"),
        ("zai'jian", "再见"),
        ("xie'xie", "谢谢"),
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
        "visits-chinese-synthetic-error"
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
fn visits_choices_survive_missing_failed_timeout_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("men kai zhe ne", "门开着呢", "门开着呢，直接进来就好。"),
            ("sui bian zu", "随便坐", "随便坐一会儿，我去切点水果。"),
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
