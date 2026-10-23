//! Finite authored clothing and fit dialogue; frozen before the 592/309/617 append.
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

// 袖 and ポケット already have whole-word readings. Only the other four
// readings are appended; all six contexts receive their first authored pair.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("fasuna-", "ふぁすなー", "ファスナー", "ファスナー"),
    ("botan", "ぼたん", "ボタン", "ボタン"),
    ("sode", "そで", "ソデ", "袖"),
    ("kutsuhimo", "くつひも", "クツヒモ", "靴ひも"),
    ("poketto", "ぽけっと", "ポケット", "ポケット"),
    ("kitsui", "きつい", "キツイ", "きつい"),
];
const NEW_READINGS: &[(&str, &str)] = &[
    ("ふぁすなー", "ファスナー"),
    ("ぼたん", "ボタン"),
    ("くつひも", "靴ひも"),
    ("きつい", "きつい"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "ファスナー",
        [
            "ファスナーを最後まで閉めます。",
            "ファスナーが引っかかったので、ゆっくり動かします。",
        ],
    ),
    (
        "ボタン",
        [
            "ボタンを留めてから出かけます。",
            "ボタンが一つ取れてしまいました。",
        ],
    ),
    (
        "袖",
        [
            "袖を少しまくってから洗い物をします。",
            "袖が長いので、少し折り返します。",
        ],
    ),
    (
        "靴ひも",
        [
            "靴ひもを結び直してから歩きます。",
            "靴ひもがほどけたので、少し待ってください。",
        ],
    ),
    (
        "ポケット",
        [
            "ポケットに鍵を入れておきます。",
            "ポケットの中を確認してから洗濯します。",
        ],
    ),
    (
        "きつい",
        [
            "きつい服は動きにくいです。",
            "きついので、一つ大きいサイズを試します。",
        ],
    ),
];
// These six near-complete inputs promise the Word and both authors on page one.
// Broad prefixes, alternate long-vowel spelling and reverse Katakana-to-Kanji
// conversion are outside these finite expectations.
const PARTIALS: &[&str] = &["fasuna", "bota", "sod", "kutsuhim", "pokett", "kitsu"];
const ALIASES: &[(&str, usize)] = &[
    ("huxasuna-", 0),
    ("botann", 1),
    ("kutuhimo", 3),
    ("pokextuto", 4),
    ("kitui", 5),
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
fn raw_kind(ime: &XRTabletImeEngine, seed: &str) -> CandidateKind {
    let raw = ime
        .candidates()
        .iter()
        .find(|c| c.text == seed)
        .unwrap_or_else(|| panic!("missing raw {seed:?}: {:?}", ime.candidates()));
    // A canonical Kana word such as きつい can retain Word classification when
    // its unchanged payload is deduplicated with the raw choice.
    assert!(matches!(
        raw.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    raw.kind
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    raw_kind(ime, seed);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local && (0.0..=100.0).contains(&c.score))
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
    assert!(
        index(ime, text, kind) < PAGE_SIZE,
        "{text:?}: {:?}",
        ime.candidates()
    );
}
fn authors(ime: &XRTabletImeEngine, which: usize) {
    for sentence in SENTENCES[which].1 {
        first_page(ime, sentence, CandidateKind::Sentence);
    }
}
fn no_dressing_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected dressing author for {seed:?}: {:?}",
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
fn commit_raw(ime: &mut XRTabletImeEngine, seed: &str) {
    let kind = raw_kind(ime, seed);
    commit_once(ime, seed, seed, kind);
}

#[test]
fn dressing_exact_four_new_readings_six_contexts_and_twelve_authors_append_once() {
    assert_eq!(WORDS.len(), 6);
    assert_eq!(NEW_READINGS.len(), 4);
    assert_eq!(SENTENCES.len(), 6);
    assert_eq!(PARTIALS.len(), 6);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 596 && ja.continuations().count() >= 315);
    for (entry, &(kana, word)) in ja.readings()[592..596].iter().zip(NEW_READINGS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(ja.readings()[..592].iter().all(|old| old.text != word));
    }
    for &(position, which) in &[(400, 2), (460, 4)] {
        let entry = &ja.readings()[position];
        assert_eq!(
            (&*entry.reading, &*entry.text),
            (WORDS[which].1, WORDS[which].3)
        );
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
    }
    for &(_, kana, _, word) in WORDS {
        assert_eq!(
            ja.readings()
                .iter()
                .filter(|entry| entry.reading == kana && entry.text == word)
                .count(),
            1,
            "duplicate reading for {word}"
        );
        assert_eq!(
            ja.readings()
                .iter()
                .filter(|entry| entry.text == word)
                .count(),
            1
        );
        assert!(
            ja.continuations()
                .take(309)
                .all(|(context, _)| context != word)
        );
        assert_eq!(
            ja.continuations()
                .filter(|(context, _)| *context == word)
                .count(),
            1
        );
    }
    for ((context, sentences), &(word, expected)) in
        ja.continuations().skip(309).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(315)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        629
    );
}

#[test]
fn dressing_full_spellings_six_bounded_partials_and_valid_aliases_commit_deliberately() {
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
                raw_kind(&ime, &seed)
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            authors(&ime, which);
            commit_once(&mut engine(&seed), &seed, word, kind);
            for sentence in SENTENCES[which].1 {
                commit_once(&mut engine(&seed), &seed, sentence, CandidateKind::Sentence);
            }
            commit_raw(&mut engine(&seed), &seed);
        }
        let ime = engine(katakana);
        draft(&ime, katakana);
        first_page(&ime, katakana, raw_kind(&ime, katakana));
        if katakana == word {
            authors(&ime, which);
        } else {
            no_dressing_authors(&ime, katakana);
        }
        commit_raw(&mut engine(katakana), katakana);
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
        commit_raw(&mut engine(partial), partial);
    }
    for &(seed, which) in ALIASES {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, WORDS[which].3, CandidateKind::Word);
        authors(&ime, which);
        commit_once(&mut engine(seed), seed, WORDS[which].3, CandidateKind::Word);
        for sentence in SENTENCES[which].1 {
            commit_once(&mut engine(seed), seed, sentence, CandidateKind::Sentence);
        }
        commit_raw(&mut engine(seed), seed);
    }
}

#[test]
fn dressing_six_horizontal_separators_adopted_progress_and_reseeding_preserve_raw() {
    let splits = [
        ("fa", "SUNA-"),
        ("bo", "TAN"),
        ("so", "DE"),
        ("kutsu", "HIMO"),
        ("poke", "TTO"),
        ("ki", "TSUI"),
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
            commit_raw(&mut engine(&seed), &seed);
        }
    }
    // The adjective's grammatical continuation is ので, using its second
    // author. Other contexts exercise adopted nouns followed by particles.
    for (which, (particle, sentence)) in [
        ("wo", 0),
        ("wo", 0),
        ("wo", 0),
        ("wo", 0),
        ("ni", 0),
        ("node", 1),
    ]
    .iter()
    .enumerate()
    {
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
            let expected = SENTENCES[which].1[*sentence];
            first_page(&ime, expected, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), &seed, expected, CandidateKind::Sentence);
            commit_raw(&mut engine(&seed), &seed);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
        }
    }
}

#[test]
fn dressing_old_592_readings_309_contexts_and_clothing_wash_controls_are_preserved() {
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for &(seed, which) in ALIASES {
        assert_eq!(romaji_to_hiragana(seed), WORDS[which].1, "{seed}");
    }
    assert_ne!(romaji_to_hiragana("fasuna"), WORDS[0].1);
    assert_ne!(romaji_to_hiragana("fasunaa"), WORDS[0].1);
    assert_ne!(romaji_to_hiragana("poketo"), WORDS[4].1);
    assert_eq!(romaji_to_hiragana("di"), "ぢ");
    assert_ne!(romaji_to_hiragana("dhi"), "でぃ");
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 592 && ja.continuations().count() >= 309);
    fn feed(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    let mut readings_hash = 0xcbf29ce484222325;
    for entry in &ja.readings()[..592] {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for field in [&*entry.reading, &*entry.text, kind] {
            feed(&mut readings_hash, field.bytes().chain([0]));
        }
        feed(&mut readings_hash, [u8::from(entry.require_separators)]);
    }
    assert_eq!(readings_hash, 0x262e04146fc84283);
    let mut contexts_hash = 0xcbf29ce484222325;
    let mut sentence_count = 0;
    for (context, sentences) in ja.continuations().take(309) {
        feed(&mut contexts_hash, context.bytes().chain([0]));
        for sentence in sentences {
            feed(&mut contexts_hash, sentence.bytes().chain([0]));
        }
        feed(&mut contexts_hash, [0xff]);
        sentence_count += sentences.len();
    }
    assert_eq!(sentence_count, 617);
    assert_eq!(contexts_hash, 0xf50687c443b5b7d0);
    for (seed, word) in [
        ("kutsushita", "靴下"),
        ("eri", "襟"),
        ("suso", "裾"),
        ("sekken", "石鹸"),
        ("haburashi", "歯ブラシ"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, word, CandidateKind::Word);
        commit_raw(&mut engine(seed), seed);
    }
    // 靴ひも can enrich the old 靴 pool without replacing its Word or pair.
    for (romaji, kana, word, sentences) in [
        (
            "kutsu",
            "くつ",
            "靴",
            [
                "靴を履いたら、すぐに出ます。",
                "靴は歩きやすいものを選びます。",
            ],
        ),
        (
            "fuku",
            "ふく",
            "服",
            ["服を選んでから出かけます。", "服は動きやすいものにします。"],
        ),
        (
            "uwagi",
            "うわぎ",
            "上着",
            [
                "上着を持っていくので、少し待ってください。",
                "上着は鞄に入れておきます。",
            ],
        ),
        (
            "sengan",
            "せんがん",
            "洗顔",
            [
                "洗顔を済ませてから着替えます。",
                "洗顔が終わったら声をかけます。",
            ],
        ),
        (
            "shanpu-",
            "しゃんぷー",
            "シャンプー",
            [
                "シャンプーの詰め替えはありますか。",
                "シャンプーが少なくなったので、買い物のメモに書いておきます。",
            ],
        ),
    ] {
        for seed in [romaji, kana, word] {
            let ime = engine(seed);
            draft(&ime, seed);
            let kind = if seed == word {
                raw_kind(&ime, seed)
            } else {
                CandidateKind::Word
            };
            first_page(&ime, word, kind);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
                commit_once(&mut engine(seed), seed, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(seed), seed, word, kind);
            commit_raw(&mut engine(seed), seed);
        }
    }
}

#[test]
fn dressing_protected_unknown_unicode_long_context_and_seven_lines_keep_raw_choices() {
    let mut seeds = [
        "https://example.invalid/fasuna-",
        "user+botan@example.invalid",
        "xyzkutsuhimo",
        "🙂ポケットni",
        "袖\u{200b}wo",
        "ｆａｓｕｎａ－",
        "fasunaa",
        "ボタンxyz",
        "きつい\u{fe0f}",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("fa{boundary}suna-"));
        seeds.push(format!("袖{boundary}wo"));
        seeds.push(format!("くつ{boundary}himo"));
    }
    for tail in ["botan", "袖wo", "ポケットni"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        no_dressing_authors(&ime, &seed);
        commit_raw(&mut ime, &seed);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-daily-dressing-synthetic-model-error"
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
fn dressing_missing_timeout_and_empty_models_preserve_each_entire_local_pool() {
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
