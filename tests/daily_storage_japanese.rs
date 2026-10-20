//! Fixed offline storage and rearrangement expressions, not inventory or device control.
//! The before catalog includes the uncommitted route and call batches: 574/291/581.
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

// Independent expectations frozen before appending six readings and six contexts.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shuunou", "しゅうのう", "シュウノウ", "収納"),
    ("hikidashi", "ひきだし", "ヒキダシ", "引き出し"),
    ("ugokashimasu", "うごかします", "ウゴカシマス", "動かします"),
    (
        "supe-suwoakemasu",
        "すぺーすをあけます",
        "スペースヲアケマス",
        "スペースを空けます",
    ),
    (
        "waketeokimasu",
        "わけておきます",
        "ワケテオキマス",
        "分けて置きます",
    ),
    ("motonobasho", "もとのばしょ", "モトノバショ", "元の場所"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "収納",
        [
            "収納する前に、中身を確認します。",
            "収納場所を先に決めておきます。",
        ],
    ),
    (
        "引き出し",
        [
            "引き出しの中を少し整理します。",
            "引き出しに入れる前に、種類ごとに分けましょう。",
        ],
    ),
    (
        "動かします",
        [
            "動かしますので、置き場所を教えてください。",
            "動かします。先に中のものを出してもいいですか。",
        ],
    ),
    (
        "スペースを空けます",
        [
            "スペースを空けますので、この箱を少し移してもいいですか。",
            "スペースを空けます。ここに荷物を置いてください。",
        ],
    ),
    (
        "分けて置きます",
        [
            "分けて置きますので、必要な方を取ってください。",
            "分けて置きます。混ざらないようにしてください。",
        ],
    ),
    (
        "元の場所",
        [
            "元の場所に戻しておきます。",
            "元の場所が分からないので、教えてください。",
        ],
    ),
];
const PARTIALS: &[&str] = &[
    "shuuno",
    "hikidash",
    "ugokashima",
    "supe-suwoakema",
    "waketeokima",
    "motonobash",
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
fn authors(ime: &XRTabletImeEngine, which: usize) {
    for sentence in SENTENCES[which].1 {
        first_page(ime, sentence, CandidateKind::Sentence);
    }
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

#[test]
fn storage_fixed_readings_existing_spelling_rules_and_append_windows() {
    assert_eq!(WORDS.len(), 6);
    assert_eq!(SENTENCES.len(), 6);
    assert_eq!(PARTIALS.len(), 6);
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for (seed, kana) in [
        ("syuunou", WORDS[0].1),
        ("hikidasi", WORDS[1].1),
        ("ugokasimasu", WORDS[2].1),
        ("motonobasyo", WORDS[5].1),
    ] {
        assert_eq!(romaji_to_hiragana(seed), kana, "{seed}");
    }
    // しゅうのう has no nasal ん. Terminal-n handling is not a new whole-word alias.
    // This does not prohibit the existing live n-vowel completion probes.
    assert_eq!(romaji_to_hiragana("shuun"), "しゅうん");
    assert_ne!(romaji_to_hiragana("shuunnou"), WORDS[0].1);
    assert_ne!(romaji_to_hiragana("supesuwoakemasu"), WORDS[3].1);
    assert_ne!(romaji_to_hiragana("supe-suoakemasu"), WORDS[3].1);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 580 && ja.continuations().count() >= 297);
    for (entry, &(_, kana, _, word)) in ja.readings()[574..580].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(291).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(291)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        581
    );
    assert_eq!(
        ja.continuations()
            .take(297)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        593
    );
}

#[test]
fn storage_complete_spellings_and_six_specific_partials_offer_explicit_commit_choices() {
    for (which, &(romaji, kana, katakana, word)) in WORDS.iter().enumerate() {
        for seed in [
            romaji.to_owned(),
            romaji.to_ascii_uppercase(),
            kana.to_owned(),
            word.to_owned(),
        ] {
            let ime = engine(&seed);
            draft(&ime, &seed);
            // Written text is the existing raw Literal, not a duplicate Word.
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
        // Preserve Katakana literally; no new Katakana-to-Kanji conversion is promised.
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
        // These near-complete inputs do not promise first-page results for every short prefix.
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
fn storage_horizontal_spaces_adopted_progress_and_reseeding_keep_raw_spelling() {
    let splits = [
        ("shuu", "NOU"),
        ("hiki", "DASHI"),
        ("ugo", "KASHIMASU"),
        ("supe-su", "WOAKEMASU"),
        ("wakete", "OKIMASU"),
        ("motono", "BASHO"),
    ];
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (which, (left, right)) in splits.iter().enumerate() {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, WORDS[which].3, CandidateKind::Word);
            authors(&ime, which);
            commit_once(&mut engine(&seed), WORDS[which].3, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
    for (which, particle) in ["suru", "no", "ので", "ので", "ので", "ni"]
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
            // Conversion removes horizontal reading separators; the author stays canonical.
            first_page(&ime, SENTENCES[which].1[0], CandidateKind::Sentence);
            commit_once(
                &mut engine(&seed),
                SENTENCES[which].1[0],
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
        }
    }
}

#[test]
fn storage_preserves_old_tidying_places_and_the_existing_du_zu_boundary() {
    for (reading, word, sentences) in [
        (
            "kataduke",
            "片付け",
            [
                "片付けを手伝ってもらえますか。",
                "片付けが終わったら出かけます。",
            ],
        ),
        (
            "okibasho",
            "置き場所",
            [
                "置き場所を決めておきます。",
                "置き場所が分からなくなりました。",
            ],
        ),
        (
            "hokanbasho",
            "保管場所",
            ["保管場所を確認してきます。", "保管場所は受付の奥です。"],
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
    for seed in ["katazuke", "KATAZUKE", "かたずけ"] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(ime.candidates().iter().all(|c| c.text != "片付け"));
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn storage_protected_unknown_seven_line_and_long_drafts_do_not_borrow_authors() {
    let mut seeds = [
        "https://shuunou.example.invalid",
        "hikidashi@example.invalid",
        "xyzugokashimasu",
        "元の場所dexyz",
        "🙂分けて置きますので",
        "収納xyz",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("supe-su{boundary}woakemasu"));
        seeds.push(format!("元の場所{boundary}ni"));
    }
    for tail in ["hikidashi", "元の場所ni", "スペースを空けますので"] {
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
            "unexpected storage author for {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-storage-synthetic-model-error"
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
fn storage_unavailable_timeout_and_empty_models_preserve_each_complete_local_pool() {
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
