//! Authored offline call-recovery expressions, not call control or audio diagnostics.
//! The before catalog is the current uncommitted route batch: 568/285/569.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{EntryKind, builtin},
};

// Fixed independently before the JSON append. Every new Word has two authors.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("kikoenikui", "きこえにくい", "キコエニクイ", "聞こえにくい"),
    (
        "myu-tokaijo",
        "みゅーとかいじょ",
        "ミュートカイジョ",
        "ミュート解除",
    ),
    (
        "gamengatomatteimasu",
        "がめんがとまっています",
        "ガメンガトマッテイマス",
        "画面が止まっています",
    ),
    (
        "otogazureteimasu",
        "おとがずれています",
        "オトガズレテイマス",
        "音がずれています",
    ),
    (
        "hairinaoshimasu",
        "はいりなおします",
        "ハイリナオシマス",
        "入り直します",
    ),
    (
        "tsunaginaoshimasu",
        "つなぎなおします",
        "ツナギナオシマス",
        "つなぎ直します",
    ),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "聞こえにくい",
        [
            "聞こえにくいので、もう少し大きな声で話してもらえますか。",
            "聞こえにくいので、チャットに書いてもらえますか。",
        ],
    ),
    (
        "ミュート解除",
        [
            "ミュート解除をお願いできますか。",
            "ミュート解除のボタンを押してから話します。",
        ],
    ),
    (
        "画面が止まっています",
        [
            "画面が止まっていますが、声は聞こえます。",
            "画面が止まっていますので、一度入り直します。",
        ],
    ),
    (
        "音がずれています",
        [
            "音がずれています。映像より少し遅れて聞こえます。",
            "音がずれていますので、一度つなぎ直します。",
        ],
    ),
    (
        "入り直します",
        [
            "入り直しますので、このまま少し待っていてください。",
            "入り直します。つながったら続きを話しましょう。",
        ],
    ),
    (
        "つなぎ直します",
        [
            "つなぎ直しますので、少しお待ちください。",
            "つなぎ直します。音が戻ったか教えてください。",
        ],
    ),
];
const PARTIALS: &[&str] = &[
    "kikoeniku",
    "myu-tokaij",
    "gamengatomatteima",
    "otogazureteima",
    "hairinaoshima",
    "tsunaginaoshima",
];

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "ja".into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    ime
}
fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    index(ime, seed, CandidateKind::Literal);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| { c.source == CandidateSource::Local && (0.0..=100.0).contains(&c.score) })
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
fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(index(ime, text, kind) < PAGE_SIZE, "{:?}", ime.candidates());
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
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
fn authors(ime: &XRTabletImeEngine, which: usize) {
    for sentence in SENTENCES[which].1 {
        first_page(ime, sentence, CandidateKind::Sentence);
    }
}

#[test]
fn call_fixed_readings_existing_aliases_and_append_windows() {
    assert_eq!(WORDS.len(), 6);
    assert_eq!(SENTENCES.len(), 6);
    assert_eq!(PARTIALS.len(), 6);
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    // These aliases already belong to the converter, not to this vocabulary batch.
    for (seed, kana) in [
        ("gamenngatomatteimasu", WORDS[2].1),
        ("gamengatomaltuteimasu", WORDS[2].1),
        ("myu-tokaijyo", WORDS[1].1),
        ("hairinaosimasu", WORDS[4].1),
        ("tsunaginaosimasu", WORDS[5].1),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    // A missing long mark and the extra vowel in kaijou are not new aliases.
    assert_ne!(romaji_to_hiragana("myutokaijo"), WORDS[1].1);
    assert_ne!(romaji_to_hiragana("myu-tokaijou"), WORDS[1].1);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 574 && ja.continuations().count() >= 291);
    for (entry, &(_, kana, _, word)) in ja.readings()[568..574].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(285).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(285)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        569
    );
    assert_eq!(
        ja.continuations()
            .take(291)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        581
    );
}

#[test]
fn call_complete_spellings_and_six_specific_partials_offer_words_authors_and_raw_once() {
    for (which, &(romaji, kana, katakana, word)) in WORDS.iter().enumerate() {
        // The written spelling is its raw Literal, not a second duplicate Word.
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            word.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            let kind = if seed == word {
                CandidateKind::Literal
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            authors(&ime, which);
            commit_once(&mut engine(&seed), word, kind);
            for sentence in SENTENCES[which].1 {
                commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
        // Existing Katakana input stays raw; this does not introduce a Kana-to-Kanji alias.
        let ime = engine(katakana);
        draft(&ime, katakana);
        first_page(&ime, katakana, CandidateKind::Literal);
        assert!(ime.candidates().iter().all(|c| {
            !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|s| c.text.contains(*s))
        }));
        commit_once(&mut engine(katakana), katakana, CandidateKind::Literal);
        // Specific near-complete readings only, not a promise about every broad short prefix.
        let partial = PARTIALS[which];
        let ime = engine(partial);
        draft(&ime, partial);
        first_page(&ime, word, CandidateKind::Word);
        authors(&ime, which);
        commit_once(&mut engine(partial), word, CandidateKind::Word);
        for sentence in SENTENCES[which].1 {
            commit_once(&mut engine(partial), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(partial), partial, CandidateKind::Literal);
    }
}

#[test]
fn call_horizontal_gaps_and_adopted_progress_remain_editable_and_reseedable() {
    let splits = [
        ("kikoe", "NIKUI"),
        ("myu-to", "KAIJO"),
        ("gamen", "GATOMATTEIMASU"),
        ("oto", "GAZURETEIMASU"),
        ("hairi", "NAOSHIMASU"),
        ("tsunagi", "NAOSHIMASU"),
    ];
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (which, (left, right)) in splits.iter().enumerate() {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, WORDS[which].3, CandidateKind::Word);
            authors(&ime, which);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for (which, particle) in ["ので", "wo", "が", "ので", "ので", "ので"]
        .iter()
        .enumerate()
    {
        let (reading, _, _, word) = WORDS[which];
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        for separator in ["", " ", "\u{3000}"] {
            let seed = format!("{word}{separator}{particle}");
            ime.seed(&seed);
            draft(&ime, &seed);
            let sentence = if which == 3 {
                SENTENCES[which].1[1]
            } else {
                SENTENCES[which].1[0]
            };
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
        }
    }
}

#[test]
fn call_preserves_existing_contact_and_connection_authors() {
    for (reading, word, sentences) in [
        (
            "tsuuwa",
            "通話",
            [
                "通話は今できますか。",
                "通話が途切れたので、もう一度かけます。",
            ],
        ),
        (
            "denpa",
            "電波",
            [
                "電波が弱いので、少し待ってください。",
                "電波が届く場所からかけ直します。",
            ],
        ),
        (
            "setsuzoku",
            "接続",
            [
                "接続を確認してから送ります。",
                "接続が切れたので、もう一度試します。",
            ],
        ),
    ] {
        let ime = engine(reading);
        draft(&ime, reading);
        first_page(&ime, word, CandidateKind::Word);
        for sentence in sentences {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(reading), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(reading), word, CandidateKind::Word);
        commit_once(&mut engine(reading), reading, CandidateKind::Literal);
    }
}

#[test]
fn call_protected_unknown_seven_line_and_long_drafts_do_not_borrow_recovery_sentences() {
    let mut seeds = [
        "https://myu-tokaijo.example.invalid",
        "kikoenikui@example.invalid",
        "xyzhairinaoshimasu",
        "画面が止まっていますdexyz",
        "🙂音がずれていますので",
        "つなぎ直しますxyz",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("myu-to{boundary}kaijo"));
        seeds.push(format!("聞こえにくい{boundary}ので"));
    }
    for tail in ["hairinaoshimasu", "音がずれていますので", "つなぎ直します"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates().iter().all(|c| !SENTENCES
                .iter()
                .flat_map(|(_, ss)| ss)
                .any(|s| c.text.contains(*s))),
            "unexpected call author for {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-call-synthetic-model-error"
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
fn call_unavailable_timeout_and_empty_models_preserve_each_complete_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for (which, &(reading, _, _, word)) in WORDS.iter().enumerate() {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
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
            commit_once(&mut ime, SENTENCES[which].1[0], CandidateKind::Sentence);
        }
    }
}
