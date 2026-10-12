//! Authored mealtime, tableware, and leftovers fallback, fixed before data additions.
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
    ("fan'hao'le", "饭好了"),
    ("bang'wo'cheng'yi'wan", "帮我盛一碗"),
    ("ba'wan'kuai'bai'hao", "把碗筷摆好"),
    ("cai'hai'mei'shang'qi", "菜还没上齐"),
    ("wo'chi'bu'xia'le", "我吃不下了"),
    ("sheng'xia'de'da'bao", "剩下的打包"),
    ("liu'yi'dian'gei'wo", "留一点给我"),
    ("xian'he'kou'tang", "先喝口汤"),
    ("wan'kuai", "碗筷"),
    ("cheng'fan", "盛饭"),
    ("fan'shao", "饭勺"),
    ("gong'kuai", "公筷"),
    ("tang'wan", "汤碗"),
    ("can'pan", "餐盘"),
    ("can'dian", "餐垫"),
    ("tang'shao", "汤勺"),
    ("da'bao'he", "打包盒"),
    ("sheng'fan", "剩饭"),
    ("tian'fan", "添饭"),
    ("kai'fan", "开饭"),
    ("duan'cai", "端菜"),
    ("jia'cai", "夹菜"),
    ("fan'liang", "饭量"),
    ("xiao'die", "小碟"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "饭好了",
        ["饭好了，过来一起吃吧。", "饭好了，今天多做了一个菜。"],
    ),
    (
        "帮我盛一碗",
        ["帮我盛一碗，我去拿筷子。", "帮我盛一碗，少放一点就好。"],
    ),
    (
        "把碗筷摆好",
        ["把碗筷摆好，我们准备吃饭。", "把碗筷摆好，汤也端上来了。"],
    ),
    (
        "菜还没上齐",
        [
            "菜还没上齐，我们再等一会儿。",
            "菜还没上齐，还有一道菜没来。",
        ],
    ),
    (
        "我吃不下了",
        ["我吃不下了，你们慢慢吃。", "我吃不下了，这一份比想象中多。"],
    ),
    (
        "剩下的打包",
        [
            "剩下的打包带走，麻烦给我一个袋子。",
            "剩下的打包，这两道菜分开放。",
        ],
    ),
    (
        "留一点给我",
        ["留一点给我，我马上就过来。", "留一点给我，我也想尝尝这个。"],
    ),
    (
        "先喝口汤",
        ["先喝口汤，饭马上就盛好。", "先喝口汤，这个味道挺清淡。"],
    ),
];

#[test]
fn mealtime_preserves_all_2946_readings_695_contexts_and_1388_sentences() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 2946);
    assert!(vocabulary.continuations().count() >= 695);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(2946) {
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
    assert_eq!(reading_hash, 0xc631aa70d5001a78);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(695) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1388);
    assert_eq!(context_hash, 0x9f1ab3da3bf8ef00);
}

#[test]
fn mealtime_appends_exactly_the_fixed_readings_and_context_window() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(WORDS.len(), 24);
    assert_eq!(SENTENCES.len(), 8);
    assert!(vocabulary.readings().len() >= 2970);
    for (entry, &(reading, text)) in vocabulary.readings()[2946..2970].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    let contexts = vocabulary
        .continuations()
        .skip(695)
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
fn mealtime_full_and_unfinished_pinyin_words_keep_literal_and_exact_commit() {
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
fn mealtime_specific_pinyin_offers_word_and_both_authored_sentence_choices() {
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
fn mealtime_adoption_and_following_pinyin_preserve_padding_and_long_frozen_text() {
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
            for tail in std::iter::once("带走".to_owned())
                .chain(spellings("dai'zou"))
                .chain(spellings("dai'zo"))
            {
                let seed = format!("{prefix}剩下的打包{padding}{tail}");
                let expected = format!("{prefix}剩下的打包{padding}带走，麻烦给我一个袋子。");
                let mut ime = engine(&seed);
                assert_local_draft(&ime, &seed);
                commit_and_undo(&mut ime, &expected, CandidateKind::Sentence);
                commit_and_undo(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn mealtime_keeps_invalid_boundaries_raw_literals_and_existing_mealtime_words() {
    for seed in [
        "zzqvjy",
        "mealtime@example.invalid",
        "https://example.invalid/meal",
        "“raw”  123\t???",
        "fan ha o le",
        "ba wa n kuai bai hao",
        "xian he ko u tang",
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
        let seed = format!("剩下的打包{boundary}dai zou");
        let mut ime = engine(&seed);
        assert_local_draft(&ime, &seed);
        let crossed = format!("剩下的打包{boundary}带走，麻烦给我一个袋子。");
        assert!(!ime.candidates().iter().any(|c| c.text == crossed));
        commit_and_undo(&mut ime, &seed, CandidateKind::Literal);
    }
    for (reading, word) in [
        ("mi'fan", "米饭"),
        ("kuai'zi", "筷子"),
        ("shao'zi", "勺子"),
        ("cha'zi", "叉子"),
        ("sheng'cai", "剩菜"),
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
        "mealtime-chinese-synthetic-error"
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
fn mealtime_choices_survive_missing_failed_timeout_and_empty_model_results() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        Some(LlmProviderError::HttpStatus(503)),
        None,
    ] {
        for (seed, word, sentence) in [
            ("fan hao le", "饭好了", "饭好了，过来一起吃吧。"),
            (
                "sheng xia de da ba",
                "剩下的打包",
                "剩下的打包带走，麻烦给我一个袋子。",
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
