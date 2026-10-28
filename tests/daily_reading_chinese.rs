//! Finite book-page and bookmark vocabulary; no model, file, or reading-state actions.
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
    ("wo'du'dao'zhe'yi'ye'le", "我读到这一页了"),
    ("shu'qian'jia'zai'zhe'ye", "书签夹在这页"),
    ("zhe'ye'hai'mei'kan'wan", "这页还没看完"),
    ("xian'fan'dao'xia'yi'ye", "先翻到下一页"),
    ("zhe'duan'wo'zai'du'yi'bian", "这段我再读一遍"),
    ("zhe'ben'shu'xian'bie'shou'qi'lai", "这本书先别收起来"),
    ("shu'qian", "书签"),
    ("shu'ye", "书页"),
    ("fan'ye", "翻页"),
    ("zhang'jie", "章节"),
    ("du'shu", "读书"),
    ("kan'shu", "看书"),
];
const TONES: &[&str] = &[
    "wo3'du2'dao4'zhe4'yi1'ye4'le5",
    "shu1'qian1'jia1'zai4'zhe4'ye4",
    "zhe4'ye4'hai2'mei2'kan4'wan2",
    "xian1'fan1'dao4'xia4'yi1'ye4",
    "zhe4'duan4'wo3'zai4'du2'yi1'bian4",
    "zhe4'ben3'shu1'xian1'bie2'shou1'qi3'lai2",
    "shu1'qian1",
    "shu1'ye4",
    "fan1'ye4",
    "zhang1'jie2",
    "du2'shu1",
    "kan4'shu1",
];
// Exactly these near-complete spellings, not arbitrary short-prefix predictions.
const PARTIALS: &[(&str, &str)] = &[
    ("wo'du'dao'zhe'yi'ye'l", "我读到这一页了"),
    ("shu'qian'jia'zai'zhe'y", "书签夹在这页"),
    ("zhe'ye'hai'mei'kan'wa", "这页还没看完"),
    ("xian'fan'dao'xia'yi'y", "先翻到下一页"),
    ("zhe'duan'wo'zai'du'yi'bia", "这段我再读一遍"),
    ("zhe'ben'shu'xian'bie'shou'qi'la", "这本书先别收起来"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "我读到这一页了",
        [
            "我读到这一页了，我先做个记号。",
            "我读到这一页了，明天从这里接着看。",
        ],
    ),
    (
        "书签夹在这页",
        [
            "书签夹在这页，我下次就能找到。",
            "书签夹在这页，先别把它拿出来。",
        ],
    ),
    (
        "这页还没看完",
        [
            "这页还没看完，我想再看一会儿。",
            "这页还没看完，先不要翻过去。",
        ],
    ),
    (
        "先翻到下一页",
        [
            "先翻到下一页，我想看看后面的图。",
            "先翻到下一页，这里等会儿再看。",
        ],
    ),
    (
        "这段我再读一遍",
        [
            "这段我再读一遍，我刚才没看明白。",
            "这段我再读一遍，你先别往下翻。",
        ],
    ),
    (
        "这本书先别收起来",
        [
            "这本书先别收起来，我还要看一会儿。",
            "这本书先别收起来，先放在桌边吧。",
        ],
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
fn reading_preserves_all_3366_readings_flags_and_839_contexts_in_original_order() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert!(vocabulary.readings().len() >= 3366);
    assert!(vocabulary.continuations().count() >= 839);
    let mut reading_hash = 0xcbf29ce484222325_u64;
    for entry in vocabulary.readings().iter().take(3366) {
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
    assert_eq!(reading_hash, 0x1fcf4163e0945a2a);
    let mut context_hash = 0xcbf29ce484222325_u64;
    let mut sentences = 0;
    for (context, values) in vocabulary.continuations().take(839) {
        sentences += values.len();
        for text in std::iter::once(context).chain(values.iter().map(String::as_str)) {
            for byte in text.bytes().chain([0]) {
                context_hash = (context_hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        context_hash = (context_hash ^ 0xff).wrapping_mul(0x100000001b3);
    }
    assert_eq!(sentences, 1676);
    assert_eq!(context_hash, 0xdc326f07c9c50045);
    // Invert just this trailing append, preserving the dirty-before JSON bytes.
    let mut raw = include_str!("../data/lexicons/zh-Hans.json").to_owned();
    if vocabulary.readings().len() > 3366 {
        for (first, after) in [
            (
                ",\n    {\"reading\": \"wo'du'dao'zhe'yi'ye'le\", \"text\": \"我读到这一页了\", \"kind\": \"word\"}",
                "\n  ],\n  \"continuations\": [",
            ),
            (",\n    [\n      \"我读到这一页了\",", "\n  ]\n}"),
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
        0xf430288c0ca112bd
    );
}

#[test]
fn reading_appends_only_twelve_words_and_six_exact_authored_contexts() {
    let vocabulary = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!((WORDS.len(), PARTIALS.len(), SENTENCES.len()), (12, 6, 6));
    assert_eq!(vocabulary.readings().len(), 3442);
    assert_eq!(vocabulary.continuations().count(), 959);
    assert_eq!(
        vocabulary
            .continuations()
            .map(|(_, values)| values.len())
            .sum::<usize>(),
        1916
    );
    for (entry, &(reading, text)) in vocabulary.readings()[3366..3378].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (reading, text));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for (((context, values), &(word, sentences)), &(_, declared_word)) in vocabulary
        .continuations()
        .skip(839)
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
fn reading_full_words_and_six_specific_partials_offer_exact_choices() {
    assert_eq!(TONES.len(), WORDS.len());
    for (&(reading, word), &tone) in WORDS.iter().zip(TONES) {
        // Tone digits test existing tolerant normalization, not tone-based ranking.
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
fn reading_spaces_adopted_known_tails_and_unicode_backtracking_are_lossless() {
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
    // Every first author has an existing wo -> 我 tail after its comma.
    for prefix in [
        String::new(),
        "已确认 API。".to_owned(),
        "前文 café 😀。  ".repeat(40),
    ] {
        for (&(full, word), &(_, sentences)) in WORDS.iter().take(6).zip(SENTENCES) {
            let sentence = sentences[0];
            assert!(sentence.starts_with(&format!("{word}，我")));
            let original = format!("{prefix}{}", full.replace('\'', " "));
            let adopted = format!("{prefix}{word}");
            let mut ime = engine(&original);
            ime.select_candidate(index(&ime, &adopted, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(adopted.as_str()));
            assert!(ime.snapshot().committed_text.is_empty());
            for gap in ["", " ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
                for tail in ["我", "wo", "WO"] {
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
fn reading_existing_book_words_authors_and_protected_line_boundaries_stay_intact() {
    // Already authored independent Words; this batch must not duplicate them.
    for (reading, word) in [
        ("yue'du", "阅读"),
        ("mu'lu", "目录"),
        ("ye'ma", "页码"),
        ("shu", "书"),
        ("jie'shu", "借书"),
        ("huan'shu", "还书"),
        ("shu'qian'lan", "书签栏"),
    ] {
        for seed in spellings(reading) {
            commit(&seed, word, CandidateKind::Word);
            commit(&seed, &seed, CandidateKind::Literal);
        }
    }
    for (reading, word, sentences) in [
        (
            "jie'shu'zheng",
            "借书证",
            ["借书证忘带了，可以用电子版吗？", "借书证可以在这里办理吗？"],
        ),
        (
            "gui'huan'tu'shu",
            "归还图书",
            ["归还图书可以放进还书箱。", "归还图书后再借新的。"],
        ),
        (
            "xu'jie'tu'shu",
            "续借图书",
            ["续借图书可以在网上办理吗？", "续借图书需要带借书证吗？"],
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
        "reading@example.invalid",
        "https://example.invalid/shuqian/jiazaizheye",
        "file_shuqianjiazaizheye",
        "“raw” 123\t???",
        "sh u qian jia zai zhe ye",
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
        let raw = format!("wo{line}du dao zhe yi ye le");
        assert!(
            !engine(&raw)
                .candidates()
                .iter()
                .any(|c| c.text == WORDS[0].1 || SENTENCES[0].1.contains(&c.text.as_str()))
        );
        commit(&raw, &raw, CandidateKind::Literal);
        let raw = format!("我读到这一页了{line}，wo");
        let crossed = format!("我读到这一页了{line}，我先做个记号。");
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
        "reading-chinese-synthetic-fallback"
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
fn reading_pending_missing_error_timeout_and_empty_models_preserve_the_entire_local_pool() {
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
