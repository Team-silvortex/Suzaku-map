//! Authored indoor-light, ventilation and comfort dialogue; frozen at 596/315/629.
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

// 換気扇 already has a whole-word reading at old index 389. Only the first
// five readings are new; all six contexts receive their first authored pair.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shoumei", "しょうめい", "ショウメイ", "照明"),
    ("ka-ten", "かーてん", "カーテン", "カーテン"),
    ("kazetooshi", "かぜとおし", "カゼトオシ", "風通し"),
    ("shitsuon", "しつおん", "シツオン", "室温"),
    ("eakon", "えあこん", "エアコン", "エアコン"),
    ("kankisen", "かんきせん", "カンキセン", "換気扇"),
];
const NEW_READINGS: &[(&str, &str)] = &[
    ("しょうめい", "照明"),
    ("かーてん", "カーテン"),
    ("かぜとおし", "風通し"),
    ("しつおん", "室温"),
    ("えあこん", "エアコン"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "照明",
        [
            "照明を少し暗くしてもいいですか。",
            "照明をつけると、手元がよく見えます。",
        ],
    ),
    (
        "カーテン",
        [
            "カーテンを開けると、部屋が明るくなります。",
            "カーテンを閉めてから着替えます。",
        ],
    ),
    (
        "風通し",
        [
            "風通しをよくするために、窓を少し開けます。",
            "風通しがよくて、気持ちがいいですね。",
        ],
    ),
    (
        "室温",
        [
            "室温を確かめてから、エアコンを調節します。",
            "室温がちょうどよくて、過ごしやすいです。",
        ],
    ),
    (
        "エアコン",
        [
            "エアコンの風が直接当たらない席に移ります。",
            "エアコンを少し弱くしてもいいですか。",
        ],
    ),
    (
        "換気扇",
        [
            "換気扇を回して、こもった空気を外に出します。",
            "換気扇を止めてから部屋を出ます。",
        ],
    ),
];
// Only these near-complete branches promise the Word and both authors on page
// one. Broad roots and reverse Katakana-to-Kanji conversion are not promised.
const PARTIALS: &[&str] = &["shoume", "ka-te", "kazetoosh", "shitsuo", "eako", "kankise"];
const ALIASES: &[(&str, usize)] = &[
    ("syoumei", 0),
    ("ka-tenn", 1),
    ("kazetoosi", 2),
    ("situon", 3),
    ("shitsuonn", 3),
    ("eakonn", 4),
    ("kannkisenn", 5),
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
    // An unchanged known Word may share its payload with the explicit raw row.
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
    assert!(ime.candidates().iter().all(|c| {
        c.source == CandidateSource::Local && c.label == c.text && (0.0..=100.0).contains(&c.score)
    }));
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
fn no_room_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected room-comfort author for {seed:?}: {:?}",
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
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
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
fn room_comfort_exact_five_new_readings_six_contexts_and_twelve_authors_append_once() {
    assert_eq!(WORDS.len(), 6);
    assert_eq!(NEW_READINGS.len(), 5);
    assert_eq!(SENTENCES.len(), 6);
    assert_eq!(PARTIALS.len(), 6);
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 601 && ja.continuations().count() >= 321);
    for (entry, &(kana, word)) in ja.readings()[596..601].iter().zip(NEW_READINGS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(ja.readings()[..596].iter().all(|old| old.text != word));
    }
    let reused = &ja.readings()[389];
    assert_eq!((&*reused.reading, &*reused.text), ("かんきせん", "換気扇"));
    assert_eq!(reused.kind, EntryKind::Word);
    assert!(!reused.require_separators);
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
                .take(315)
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
        ja.continuations().skip(315).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            sentences.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(
        ja.continuations()
            .take(321)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        641
    );
}

#[test]
fn room_comfort_full_spellings_six_bounded_partials_and_supported_aliases_commit_once() {
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
            no_room_authors(&ime, katakana);
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
fn room_comfort_six_horizontal_gaps_adopted_particles_and_reseeding_keep_raw() {
    let splits = [
        ("shou", "MEI"),
        ("ka-", "TEN"),
        ("kaze", "TOOSHI"),
        ("shi", "TSUON"),
        ("ea", "KON"),
        ("kan", "KISEN"),
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
    for (which, particle) in ["wo", "wo", "wo", "wo", "no", "wo"].iter().enumerate() {
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
            first_page(&ime, SENTENCES[which].1[0], CandidateKind::Sentence);
            commit_once(
                &mut engine(&seed),
                &seed,
                SENTENCES[which].1[0],
                CandidateKind::Sentence,
            );
            commit_raw(&mut engine(&seed), &seed);
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
        }
    }
}

#[test]
fn room_comfort_old_596_readings_315_contexts_and_ventilation_brightness_weather_stay() {
    for &(romaji, kana, katakana, _) in WORDS {
        assert_eq!(romaji_to_hiragana(romaji), kana, "{romaji}");
        assert_eq!(romaji_to_hiragana(&romaji.to_ascii_uppercase()), kana);
        assert_eq!(romaji_to_hiragana(katakana), katakana);
    }
    for &(seed, which) in ALIASES {
        assert_eq!(romaji_to_hiragana(seed), WORDS[which].1, "{seed}");
    }
    assert_ne!(romaji_to_hiragana("shomei"), WORDS[0].1);
    assert_ne!(romaji_to_hiragana("kaaten"), WORDS[1].1);
    assert_ne!(romaji_to_hiragana("kazetoushi"), WORDS[2].1);
    assert_eq!(romaji_to_hiragana("di"), "ぢ");
    assert_ne!(romaji_to_hiragana("dhi"), "でぃ");
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 596 && ja.continuations().count() >= 315);
    fn feed(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
        for byte in bytes {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    let mut readings_hash = 0xcbf29ce484222325;
    for entry in &ja.readings()[..596] {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for field in [&*entry.reading, &*entry.text, kind] {
            feed(&mut readings_hash, field.bytes().chain([0]));
        }
        feed(&mut readings_hash, [u8::from(entry.require_separators)]);
    }
    assert_eq!(readings_hash, 0xb9914cbd57c5464a);
    let mut contexts_hash = 0xcbf29ce484222325;
    let mut sentence_count = 0;
    for (context, sentences) in ja.continuations().take(315) {
        feed(&mut contexts_hash, context.bytes().chain([0]));
        for sentence in sentences {
            feed(&mut contexts_hash, sentence.bytes().chain([0]));
        }
        feed(&mut contexts_hash, [0xff]);
        sentence_count += sentences.len();
    }
    assert_eq!(sentence_count, 629);
    assert_eq!(contexts_hash, 0x6a9fc416584c8818);
    // 換気扇 was already a Word completion of 換気. Its new authors may enrich
    // that old pool, while the old Word and both old authors stay on page one.
    for (romaji, kana, word, sentences) in [
        (
            "kanki",
            "かんき",
            "換気",
            [
                "換気のために窓を開けます。",
                "換気をして空気を入れ替えましょう。",
            ],
        ),
        (
            "akarusa",
            "あかるさ",
            "明るさ",
            [
                "明るさを少し下げてもいいですか。",
                "明るさはこのくらいで大丈夫です。",
            ],
        ),
        (
            "hadazamui",
            "はだざむい",
            "肌寒い",
            ["肌寒いので上着を羽織ります。", "肌寒い朝が増えてきました。"],
        ),
        (
            "mushiatsui",
            "むしあつい",
            "蒸し暑い",
            [
                "蒸し暑い日は扇風機を使います。",
                "蒸し暑いので少し休みましょう。",
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
fn room_comfort_protected_unicode_unknown_long_drafts_and_seven_lines_keep_raw() {
    let mut seeds = [
        "https://example.invalid/shoumei",
        "user+eakon@example.invalid",
        "xyzka-ten",
        "🙂室温wo",
        "風通し\u{200b}wo",
        "ｓｈｏｕｍｅｉ",
        "kaaten",
        "照明xyz",
        "換気扇\u{fe0f}",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for boundary in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("shou{boundary}mei"));
        seeds.push(format!("カーテン{boundary}wo"));
        seeds.push(format!("かんき{boundary}sen"));
    }
    for tail in ["eakon", "照明wo", "換気扇wo"] {
        let seed = format!("{}{tail}", "前文。".repeat(90));
        assert!(seed.chars().count() > 256);
        seeds.push(seed);
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        no_room_authors(&ime, &seed);
        commit_raw(&mut ime, &seed);
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-room-comfort-synthetic-model-error"
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
fn room_comfort_absent_failed_timeout_and_empty_models_preserve_entire_local_pools() {
    for (which, &(reading, _, _, word)) in WORDS.iter().enumerate() {
        let baseline = engine(reading).candidates().to_vec();
        let mut disabled = engine(reading);
        disabled.configure_prediction(None);
        assert_eq!(disabled.prediction_status(), PredictionStatus::Disabled);
        assert!(disabled.prediction_error().is_none());
        assert_eq!(disabled.candidates(), baseline);
        authors(&disabled, which);
        commit_once(
            &mut disabled,
            reading,
            SENTENCES[which].1[0],
            CandidateKind::Sentence,
        );
        for error in [
            Some(LlmProviderError::NoLocalModel),
            Some(LlmProviderError::Unavailable),
            Some(LlmProviderError::Timeout),
            None,
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(reading);
            draft(&ime, reading);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
            let before = ime.candidates().to_vec();
            assert_eq!(before, baseline);
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
