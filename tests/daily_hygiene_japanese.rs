//! Finite wash-item dialogue, not product advice, inventory sensing or purchasing.
//! Frozen before daily_hygiene against the dirty restock catalog: 586/303/605.
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

// The first five canonical Words are already Katakana. 洗顔料 is not a
// Katakana-to-Kanji alias: センガンリョウ retains its separate raw spelling.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shanpu-", "しゃんぷー", "シャンプー", "シャンプー"),
    ("rinsu", "りんす", "リンス", "リンス"),
    ("handoso-pu", "はんどそーぷ", "ハンドソープ", "ハンドソープ"),
    ("heaburashi", "へあぶらし", "ヘアブラシ", "ヘアブラシ"),
    ("so-puke-su", "そーぷけーす", "ソープケース", "ソープケース"),
    ("senganryou", "せんがんりょう", "センガンリョウ", "洗顔料"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "シャンプー",
        [
            "シャンプーの詰め替えはありますか。",
            "シャンプーが少なくなったので、買い物のメモに書いておきます。",
        ],
    ),
    (
        "リンス",
        [
            "リンスはどこに置きましたか。",
            "リンスを買う前に、家にある分を確認します。",
        ],
    ),
    (
        "ハンドソープ",
        [
            "ハンドソープの残りを確認します。",
            "ハンドソープは洗面台の横にあります。",
        ],
    ),
    (
        "ヘアブラシ",
        [
            "ヘアブラシはどこに置きましたか。",
            "ヘアブラシを洗面所に戻しておきます。",
        ],
    ),
    (
        "ソープケース",
        [
            "ソープケースを洗っておきます。",
            "ソープケースは棚の上にあります。",
        ],
    ),
    (
        "洗顔料",
        [
            "洗顔料を買い物のメモに追加します。",
            "洗顔料の予備はまだあります。",
        ],
    ),
];
// Only these near-complete inputs promise a first-page Word and authored pair.
// No promise is made for broad sha/rin/kon/han/se prefixes or their full pools.
const PARTIALS: &[&str] = &[
    "shanpu",
    "rins",
    "handoso-p",
    "heaburash",
    "so-puke-s",
    "senganryo",
];
const ALIASES: &[(&str, usize)] = &[
    ("syanpu-", 0),
    ("shannpu-", 0),
    ("rinnsu", 1),
    ("hanndoso-pu", 2),
    ("heaburasi", 3),
    ("senngannryou", 5),
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
fn no_hygiene_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected hygiene author for {seed:?}: {:?}",
        ime.candidates()
    );
}
fn commit_once(ime: &mut XRTabletImeEngine, seed: &str, text: &str, kind: CandidateKind) {
    assert_eq!(ime.snapshot().seed_text, seed);
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
fn hygiene_exact_six_readings_and_six_authored_pairs_are_appended_in_fixed_order() {
    assert_eq!(WORDS.len(), 6);
    assert_eq!(SENTENCES.len(), 6);
    assert_eq!(PARTIALS.len(), 6);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 592 && ja.continuations().count() >= 309);
    for (entry, &(_, kana, _, word)) in ja.readings()[586..592].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(303).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(309)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        617
    );
}

#[test]
fn hygiene_complete_spellings_specific_partials_and_existing_aliases_allow_deliberate_commits() {
    for (which, &(romaji, kana, katakana, word)) in WORDS.iter().enumerate() {
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
            commit_once(&mut engine(&seed), &seed, word, kind);
            for sentence in SENTENCES[which].1 {
                commit_once(&mut engine(&seed), &seed, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(&seed), &seed, &seed, CandidateKind::Literal);
        }
        let ime = engine(katakana);
        draft(&ime, katakana);
        first_page(&ime, katakana, CandidateKind::Literal);
        if katakana == word {
            authors(&ime, which);
        } else {
            no_hygiene_authors(&ime, katakana);
        }
        commit_once(
            &mut engine(katakana),
            katakana,
            katakana,
            CandidateKind::Literal,
        );
        let partial = PARTIALS[which];
        let ime = engine(partial);
        draft(&ime, partial);
        first_page(&ime, word, CandidateKind::Word);
        authors(&ime, which);
        commit_once(&mut engine(partial), partial, word, CandidateKind::Word);
        for sentence in SENTENCES[which].1 {
            commit_once(
                &mut engine(partial),
                partial,
                sentence,
                CandidateKind::Sentence,
            );
        }
        commit_once(
            &mut engine(partial),
            partial,
            partial,
            CandidateKind::Literal,
        );
    }
    for &(seed, which) in ALIASES {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, WORDS[which].3, CandidateKind::Word);
        authors(&ime, which);
        commit_once(&mut engine(seed), seed, WORDS[which].3, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, seed, CandidateKind::Literal);
    }
}

#[test]
fn hygiene_horizontal_spaces_adopted_particles_and_reseeding_preserve_raw_input() {
    let splits = [
        ("sha", "NPU-"),
        ("ri", "NSU"),
        ("han", "DOSO-PU"),
        ("hea", "BURASHI"),
        ("so-pu", "KE-SU"),
        ("sen", "GANRYOU"),
    ];
    for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        for (which, (left, right)) in splits.iter().enumerate() {
            let seed = format!("{separator}{left}{separator}{right}{separator}");
            let ime = engine(&seed);
            draft(&ime, &seed);
            first_page(&ime, WORDS[which].3, CandidateKind::Word);
            authors(&ime, which);
            commit_once(
                &mut engine(&seed),
                &seed,
                WORDS[which].3,
                CandidateKind::Word,
            );
            commit_once(&mut engine(&seed), &seed, &seed, CandidateKind::Literal);
        }
    }
    for (which, particle) in ["no", "ha", "no", "ha", "wo", "wo"].iter().enumerate() {
        let (reading, _, _, word) = WORDS[which];
        let mut ime = engine(reading);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert_eq!(ime.snapshot().seed_text, reading);
        assert!(ime.snapshot().committed_text.is_empty());
        for separator in ["", " ", "\u{3000}"] {
            let seed = format!("{word}{separator}{particle}");
            ime.seed(&seed);
            draft(&ime, &seed);
            // Authored sentences are canonical; only Literal retains separators.
            first_page(&ime, SENTENCES[which].1[0], CandidateKind::Sentence);
            commit_once(
                &mut engine(&seed),
                &seed,
                SENTENCES[which].1[0],
                CandidateKind::Sentence,
            );
            commit_once(&mut engine(&seed), &seed, &seed, CandidateKind::Literal);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
        }
    }
}

#[test]
fn hygiene_legacy_catalog_prefix_spelling_rules_and_existing_wash_words_are_preserved() {
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for &(seed, which) in ALIASES {
        assert_eq!(romaji_to_hiragana(seed), WORDS[which].1, "{seed}");
    }
    // Long marks are explicit, and n/nn only use the existing nasal rules.
    assert_ne!(romaji_to_hiragana("shanpu"), WORDS[0].1);
    assert_ne!(romaji_to_hiragana("handosopu"), WORDS[2].1);
    assert_ne!(romaji_to_hiragana("sopukesu"), WORDS[4].1);
    assert_ne!(romaji_to_hiragana("heaburashin"), WORDS[3].1);
    assert_eq!(romaji_to_hiragana("di"), "ぢ");
    assert_ne!(romaji_to_hiragana("dhi"), "でぃ");
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 586 && ja.continuations().count() >= 303);
    fn feed(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    let mut readings_hash = 0xcbf29ce484222325;
    for entry in &ja.readings()[..586] {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for field in [&*entry.reading, &*entry.text, kind] {
            feed(&mut readings_hash, field.bytes().chain([0]));
        }
        feed(&mut readings_hash, [u8::from(entry.require_separators)]);
    }
    assert_eq!(readings_hash, 0x0395e8e04576de39);
    let mut contexts_hash = 0xcbf29ce484222325;
    let mut sentence_count = 0;
    for (context, sentences) in ja.continuations().take(303) {
        feed(&mut contexts_hash, context.bytes().chain([0]));
        for sentence in sentences {
            feed(&mut contexts_hash, sentence.bytes().chain([0]));
        }
        feed(&mut contexts_hash, [0xff]);
        sentence_count += sentences.len();
    }
    assert_eq!(sentence_count, 605);
    assert_eq!(contexts_hash, 0xb5548df99cb10da4);
    // Old exact Words are not newly supported, nor promised invented authors.
    for (seed, word) in [
        ("sekken", "石鹸"),
        ("haburashi", "歯ブラシ"),
        ("hamigakiko", "歯磨き粉"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, seed, CandidateKind::Literal);
    }
    // 洗顔料 may enrich the old 洗顔 pool, but not replace its Word or authors.
    for (seed, word, sentences) in [
        (
            "sengan",
            "洗顔",
            [
                "洗顔を済ませてから着替えます。",
                "洗顔が終わったら声をかけます。",
            ],
        ),
        (
            "thisshu",
            "ティッシュ",
            [
                "ティッシュが残り少なくなりました。",
                "ティッシュを一箱買い足しておきます。",
            ],
        ),
        (
            "toirettope-pa-",
            "トイレットペーパー",
            [
                "トイレットペーパーの予備はありますか。",
                "トイレットペーパーを買い物のメモに追加します。",
            ],
        ),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        for sentence in sentences {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(seed), seed, sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(seed), seed, word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, seed, CandidateKind::Literal);
    }
}

#[test]
fn hygiene_protected_unknown_unicode_and_line_boundaries_do_not_borrow_authors() {
    let mut seeds = [
        "https://example.invalid/shanpu-",
        "user+heaburashi@example.invalid",
        "xyzhandoso-pu",
        "🙂ソープケースwo",
        "ヘアブラシ\u{200b}ha",
        "ｓｈａｎｐｕ－",
        "handosopu",
        "sopukesu",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("sha{boundary}npu-"));
        seeds.push(format!("ヘアブラシ{boundary}ha"));
    }
    for tail in ["rinsu", "ハンドソープno", "洗顔料wo"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        no_hygiene_authors(&ime, &seed);
        commit_once(&mut ime, &seed, &seed, CandidateKind::Literal);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-daily-hygiene-synthetic-model-error"
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
fn hygiene_unavailable_timeout_and_empty_models_preserve_each_entire_local_pool() {
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
            commit_once(
                &mut ime,
                reading,
                SENTENCES[which].1[0],
                CandidateKind::Sentence,
            );
        }
    }
}
