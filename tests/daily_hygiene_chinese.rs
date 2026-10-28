//! Fixed basic toiletries/paper nouns and authored wording, not health advice.
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
    ("zhi'zhang", "纸张"),
    ("hu'fa'su", "护发素"),
    ("hu'shou'shuang", "护手霜"),
    ("run'chun'gao", "润唇膏"),
    ("mian'jin'zhi", "面巾纸"),
    ("ca'shou'zhi", "擦手纸"),
    ("fei'zao'he", "肥皂盒"),
    ("ti'huan'shua'tou", "替换刷头"),
];
// Four already-authored Words gain contexts, not duplicate reading records.
const CONTEXT_WORDS: &[(&str, &str)] = &[
    ("zhi'zhang", "纸张"),
    ("xiang'zao", "香皂"),
    ("xi'fa'shui", "洗发水"),
    ("hu'fa'su", "护发素"),
    ("ya'gao", "牙膏"),
    ("ya'shua", "牙刷"),
];
// Only these last-letter omissions are promised, not arbitrary short prefixes.
const PARTIALS: &[(&str, &str)] = &[
    ("zhi'zhan", "纸张"),
    ("xiang'za", "香皂"),
    ("xi'fa'shu", "洗发水"),
    ("hu'fa's", "护发素"),
    ("ya'ga", "牙膏"),
    ("ya'shu", "牙刷"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "纸张",
        ["纸张放在打印机旁边了。", "纸张还有一些，我先拿几张过来。"],
    ),
    (
        "香皂",
        ["香皂放在洗手台旁边。", "香皂还有一块，先不用再找了。"],
    ),
    (
        "洗发水",
        ["洗发水在浴室的架子上。", "洗发水我已经放进洗漱包了。"],
    ),
    (
        "护发素",
        ["护发素和洗发水分开放着。", "护发素还有一瓶没开封。"],
    ),
    (
        "牙膏",
        ["牙膏我放在牙刷旁边了。", "牙膏快用完了，我先记在清单上。"],
    ),
    (
        "牙刷",
        ["牙刷放在右边的杯子里。", "牙刷我已经装进洗漱包了。"],
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
fn hygiene_preserves_all_3322_readings_flags_and_815_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3322);
    assert!(vocabulary.continuations().count() >= 815);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3322) {
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
    assert_eq!(reading_hash, 0x1bf5f8de529cc72b);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(815) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1628);
    assert_eq!(context_hash, 0x0b2be7727787b5d5);
}

#[test]
fn hygiene_appends_only_eight_nouns_and_six_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(
        (
            WORDS.len(),
            CONTEXT_WORDS.len(),
            PARTIALS.len(),
            SENTENCES.len()
        ),
        (8, 6, 6, 6)
    );
    assert_eq!(vocabulary.readings().len(), 3442);
    assert_eq!(vocabulary.continuations().count(), 937);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1872
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3322..3330].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(815)
        .take(6)
        .zip(SENTENCES)
        .zip(CONTEXT_WORDS)
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
fn hygiene_nouns_full_readings_and_six_specific_partials_offer_exact_choices() {
    for &(reading, word) in WORDS {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for &(reading, word) in CONTEXT_WORDS.iter().chain(PARTIALS) {
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
fn hygiene_horizontal_spaces_known_tail_adoption_and_unicode_reseeding_are_lossless() {
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
        for (i, (han, reading)) in [
            ("放", "fang"),
            ("放", "fang"),
            ("在", "zai"),
            ("和", "he"),
            ("我", "wo"),
            ("放", "fang"),
        ]
        .into_iter()
        .enumerate()
        {
            let (full, word) = CONTEXT_WORDS[i];
            let sentence = SENTENCES[i].1[0];
            assert!(sentence.starts_with(&format!("{word}{han}")));
            let original = format!("{prefix}{}", full.replace('\'', " "));
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&original);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert!(ime.snapshot().committed_text.is_empty());
            for gap in ["", " ", "\t", "\u{3000}"] {
                for tail in std::iter::once(han.to_owned()).chain(spellings(reading)) {
                    let raw = format!("{adopted}{gap}{tail}");
                    let expected =
                        format!("{adopted}{gap}{}", sentence.strip_prefix(word).unwrap());
                    ime.seed(&raw);
                    draft(&ime, &raw);
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
fn hygiene_existing_words_paper_authors_and_literal_boundaries_are_preserved() {
    // These were already Words before this batch, not newly acquired readings.
    for (reading, word) in [
        ("xiang'zao", "香皂"),
        ("fei'zao", "肥皂"),
        ("xi'fa'shui", "洗发水"),
        ("ya'gao", "牙膏"),
        ("ya'shua", "牙刷"),
        ("zhi'jin", "纸巾"),
    ] {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
            if word == "纸巾" {
                for sentence in ["纸巾放在桌上了。", "纸巾快用完了。"] {
                    commit(&seed, sentence, CandidateKind::Sentence);
                }
            }
        }
    }
    for raw in [
        "zzqvjy",
        "hygiene@example.invalid",
        "https://example.invalid/zhizhang",
        "file_zhizhang",
        "“raw” 123\t???",
        "zhi zha ng",
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
        let raw = format!("zhi{line}zhang");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == "纸张" || SENTENCES[0].1.contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("纸张{line}fang");
        let crossed = format!("纸张{line}放在打印机旁边了。");
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
        "hygiene-chinese-synthetic-fallback"
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
fn hygiene_unavailable_timeout_and_empty_models_preserve_the_entire_local_pool() {
    for error in [
        Some(LlmProviderError::NoLocalModel),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (&(seed, word), &(_, sentences)) in PARTIALS.iter().zip(SENTENCES) {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            index(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                index(&ime, sentence, CandidateKind::Sentence);
            }
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
            commit_from(&mut ime, sentences[0], CandidateKind::Sentence);
        }
    }
}
