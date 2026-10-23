//! Finite reading dialogue, frozen against the actual 605/327/653 vocabulary.
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, LanguagePlugin, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    },
    languages::{
        japanese::{JapaneseLanguagePlugin, romaji_to_hiragana},
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{
        EntryKind, builtin,
        packs::{self, PackStore},
    },
};

// The last two whole Words already exist at indices 309/310. Only their
// book-reading contexts are missing; do not append duplicate readings for them.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shiori", "しおり", "シオリ", "栞"),
    ("hyoushi", "ひょうし", "ヒョウシ", "表紙"),
    ("hondana", "ほんだな", "ホンダナ", "本棚"),
    ("yomikake", "よみかけ", "ヨミカケ", "読みかけ"),
    ("kidoku", "きどく", "キドク", "既読"),
    ("midoku", "みどく", "ミドク", "未読"),
];
const NEW_READINGS: &[(&str, &str)] = &[
    ("しおり", "栞"),
    ("ひょうし", "表紙"),
    ("ほんだな", "本棚"),
    ("よみかけ", "読みかけ"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "栞",
        [
            "栞を挟んでから本を閉じます。",
            "栞が見つからないので、付箋を目印にします。",
        ],
    ),
    (
        "表紙",
        [
            "表紙を見て、読みたい本を選びます。",
            "表紙が折れないように、かばんに入れます。",
        ],
    ),
    (
        "本棚",
        [
            "本棚に読み終わった本を戻します。",
            "本棚の上の段を少し整理します。",
        ],
    ),
    (
        "読みかけ",
        [
            "読みかけの本に栞を挟んでおきます。",
            "読みかけの本は机の端に置いてあります。",
        ],
    ),
    (
        "既読",
        [
            "既読の本は棚の右側にまとめます。",
            "既読の本をもう一度読み返します。",
        ],
    ),
    (
        "未読",
        [
            "未読の本を週末に読もうと思います。",
            "未読の本は棚の左側にまとめます。",
        ],
    ),
];
// Only these near-complete branches promise a Word and two real authors.
// Broad roots and arbitrary Katakana-to-Kanji recovery are not promised.
const PARTIALS: &[&str] = &["shior", "hyoush", "honda", "yomikak", "kidok", "midok"];
const ALIASES: &[(&str, usize)] = &[
    ("siori", 0),
    ("hyousi", 1),
    ("honndana", 2),
    ("hon'dana", 2),
];
const CHILD: &str = "SUZAKU_READING_JAPANESE_CATALOG";

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalogs(name: &str, body: impl FnOnce()) {
    let mode = std::env::var(CHILD);
    if mode.is_err() {
        // Capture both catalogs even in the real before run: a builtin failure
        // must not prevent the independent nine-pack missing-data observation.
        let results = ["builtin", "packs"].map(|mode| {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", name, "--nocapture", "--test-threads=1"])
                .env(CHILD, mode)
                .env_remove("SUZAKU_LEXICON_DIR")
                .env_remove("SUZAKU_IME_CONFIG")
                .env_remove("SUZAKU_LINUX_IME_SOCKET")
                .env_remove("DISPLAY")
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("DBUS_SESSION_BUS_ADDRESS")
                .output()
                .unwrap();
            (mode, output)
        });
        let details = results
            .iter()
            .map(|(mode, output)| {
                format!(
                    "{mode}: {}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            results.iter().all(|(_, output)| output.status.success()),
            "{details}"
        );
        for (mode, output) in &results {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
                "{stdout}"
            );
            assert!(
                stdout.contains(&format!("completed reading {name} with {mode}")),
                "{stdout}"
            );
        }
        return;
    }
    let mode = mode.unwrap();
    assert!(matches!(mode.as_str(), "builtin" | "packs"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("suzaku-reading-ja-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    let selected = packs::recommended()
        .into_iter()
        .filter(|p| mode == "packs" && p.manifest.language == "ja")
        .collect::<Vec<_>>();
    let expected = if mode == "packs" { 9 } else { 0 };
    assert_eq!(selected.len(), expected);
    let ids = selected
        .iter()
        .map(|p| p.manifest.id.clone())
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), expected);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(
        report.errors.is_empty() && report.disabled.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.loaded.len(), expected);
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    body();
    println!("completed reading {name} with {mode}");
}
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
    let row = ime
        .candidates()
        .iter()
        .find(|c| c.text == seed)
        .unwrap_or_else(|| panic!("missing raw {seed:?}: {:?}", ime.candidates()));
    assert!(matches!(
        row.kind,
        CandidateKind::Literal | CandidateKind::Word
    ));
    row.kind
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 2 * PAGE_SIZE);
    raw_kind(ime, seed);
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local
                && c.label == c.text
                && c.score.is_finite()
                && (0.0..=100.0).contains(&c.score))
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
fn no_reading_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected reading author for {seed:?}: {:?}",
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
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn commit_raw(ime: &mut XRTabletImeEngine, seed: &str) {
    let kind = raw_kind(ime, seed);
    commit_once(ime, seed, seed, kind);
}

#[test]
fn reading_exact_four_new_readings_six_contexts_twelve_authors_and_two_reused_words() {
    assert_eq!(
        (
            WORDS.len(),
            NEW_READINGS.len(),
            SENTENCES.len(),
            PARTIALS.len()
        ),
        (6, 4, 6, 6)
    );
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 609 && ja.continuations().count() >= 333);
    for (entry, &(kana, word)) in ja.readings()[605..609].iter().zip(NEW_READINGS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(ja.readings()[..605].iter().all(|old| old.text != word));
    }
    for (old_index, which) in [(309, 4), (310, 5)] {
        let entry = &ja.readings()[old_index];
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
                .filter(|e| e.reading == kana && e.text == word)
                .count(),
            1
        );
        assert_eq!(ja.readings().iter().filter(|e| e.text == word).count(), 1);
        assert!(
            ja.continuations()
                .take(327)
                .all(|(context, _)| context != word)
        );
        assert_eq!(
            ja.continuations()
                .filter(|(context, _)| *context == word)
                .count(),
            1
        );
    }
    for ((context, values), &(word, expected)) in
        ja.continuations().skip(327).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert_eq!(
            values.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
        assert_ne!(expected[0], expected[1]);
        assert!(
            expected
                .iter()
                .all(|s| s.starts_with(word) && s.len() > word.len())
        );
    }
    assert_eq!(
        ja.continuations()
            .take(333)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        665
    );
}

#[test]
fn reading_old_605_readings_flags_327_contexts_653_authors_and_neighbor_controls_stay() {
    const NAME: &str =
        "reading_old_605_readings_flags_327_contexts_653_authors_and_neighbor_controls_stay";
    catalogs(NAME, || {
        fn feed(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
            for byte in bytes {
                *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        let ja = builtin("ja").unwrap();
        assert!(ja.readings().len() >= 605 && ja.continuations().count() >= 327);
        let mut h = 0xcbf29ce484222325;
        for entry in &ja.readings()[..605] {
            let kind = match entry.kind {
                EntryKind::Word => "word",
                EntryKind::Sentence => "sentence",
            };
            for value in [&*entry.reading, &*entry.text, kind] {
                feed(&mut h, value.bytes().chain([0]));
            }
            feed(&mut h, [u8::from(entry.require_separators)]);
        }
        assert_eq!(h, 0x3341141ee614a7fe);
        let mut h = 0xcbf29ce484222325;
        let mut count = 0;
        for (context, sentences) in ja.continuations().take(327) {
            feed(&mut h, context.bytes().chain([0]));
            for sentence in sentences {
                feed(&mut h, sentence.bytes().chain([0]));
            }
            feed(&mut h, [0xff]);
            count += sentences.len();
        }
        assert_eq!((count, h), (653, 0xd327e64d89a209c5));
        for &(roman, kana, katakana, _) in WORDS {
            assert_eq!(romaji_to_hiragana(roman), kana);
            assert_eq!(romaji_to_hiragana(&roman.to_ascii_uppercase()), kana);
            assert_eq!(romaji_to_hiragana(katakana), katakana);
        }
        for &(alias, which) in ALIASES {
            assert_eq!(romaji_to_hiragana(alias), WORDS[which].1);
        }
        assert_ne!(romaji_to_hiragana("hyoshi"), "ひょうし");
        assert_eq!(romaji_to_hiragana("hondan"), "ほんだん");
        assert_ne!(romaji_to_hiragana("hondan"), "ほんだな");
        assert_eq!(romaji_to_hiragana("di"), "ぢ");
        for (roman, word, sentences) in [
            (
                "dokusho",
                "読書",
                ["読書をして過ごしたいです。", "読書の時間を作りたいです。"],
            ),
            (
                "toshokan",
                "図書館",
                ["図書館で本を探したいです。", "図書館に寄ってから帰ります。"],
            ),
            (
                "bunbougu",
                "文房具",
                [
                    "文房具を少し借りてもいいですか。",
                    "文房具は使った後で戻します。",
                ],
            ),
            (
                "kankisen",
                "換気扇",
                [
                    "換気扇を回して、こもった空気を外に出します。",
                    "換気扇を止めてから部屋を出ます。",
                ],
            ),
        ] {
            let ime = engine(roman);
            draft(&ime, roman);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(roman), roman, word, CandidateKind::Word);
        }
        for (seed, expected) in [
            ("en", &["演奏", "鉛筆", "延長"][..]),
            ("keshi", &["景色", "消しゴム"][..]),
            ("jou", &["条件", "情報", "乗車", "乗車券"][..]),
        ] {
            let ime = engine(seed);
            draft(&ime, seed);
            let positions = expected
                .iter()
                .map(|word| index(&ime, word, CandidateKind::Word))
                .collect::<Vec<_>>();
            assert!(positions.windows(2).all(|v| v[0] < v[1]));
            commit_raw(&mut engine(seed), seed);
        }
        if std::env::var(CHILD).as_deref() == Ok("packs") {
            for (reading, word, sentences) in [
                (
                    "pe-ji",
                    "ページ",
                    [
                        "ページがまだ読み込み中です。",
                        "ページの下の方に説明があります。",
                    ],
                ),
                (
                    "dokkai",
                    "読解",
                    [
                        "読解の問題は短い文章から始めます。",
                        "読解では段落ごとの要点をまとめます。",
                    ],
                ),
            ] {
                let ime = engine(reading);
                draft(&ime, reading);
                first_page(&ime, word, CandidateKind::Word);
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                }
            }
            first_page(&engine("mokuji"), "目次", CandidateKind::Word);
        }
        let ime = engine("joushake");
        first_page(&ime, "乗車券", CandidateKind::Word);
        assert_eq!(
            index(
                &ime,
                "乗車券を確認してから乗りましょう。",
                CandidateKind::Sentence
            ),
            8
        );
        commit_once(
            &mut engine("joushake"),
            "joushake",
            "乗車券を確認してから乗りましょう。",
            CandidateKind::Sentence,
        );
    });
}

#[test]
fn reading_whole_case_kana_canonical_aliases_and_six_bounded_partials_commit_once_in_both_catalogs()
{
    const NAME: &str = "reading_whole_case_kana_canonical_aliases_and_six_bounded_partials_commit_once_in_both_catalogs";
    catalogs(NAME, || {
        for (which, &(roman, kana, katakana, word)) in WORDS.iter().enumerate() {
            for seed in [
                roman.to_owned(),
                roman.to_ascii_uppercase(),
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
            if katakana == word {
                authors(&ime, which);
            } else {
                no_reading_authors(&ime, katakana);
            }
            commit_raw(&mut engine(katakana), katakana);
            let partial = PARTIALS[which];
            let ime = engine(partial);
            draft(&ime, partial);
            first_page(&ime, word, CandidateKind::Word);
            authors(&ime, which);
            let six = offline(
                "ja",
                partial,
                "",
                JapaneseLanguagePlugin
                    .direct_candidates(partial, 1.0)
                    .unwrap(),
                PAGE_SIZE,
            );
            assert!(six.len() <= PAGE_SIZE);
            for (text, kind) in [
                (word, CandidateKind::Word),
                (SENTENCES[which].1[0], CandidateKind::Sentence),
                (SENTENCES[which].1[1], CandidateKind::Sentence),
                (partial, CandidateKind::Literal),
            ] {
                assert!(
                    six.iter().any(|c| c.text == text && c.kind == kind),
                    "{partial}: {six:?}"
                );
            }
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
        // Terminal n remains ん in the primary conversion. Existing bounded
        // completion hints may still suggest 本棚; this is not a full alias.
        let ime = engine("hondan");
        draft(&ime, "hondan");
        assert_eq!(ime.candidates()[0].text, "ほんだん");
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Unspecified);
        assert_eq!(ime.candidates()[0].score.to_bits(), 100.0_f32.to_bits());
        first_page(&ime, "本棚", CandidateKind::Word);
        authors(&ime, 2);
        commit_once(&mut engine("hondan"), "hondan", "本棚", CandidateKind::Word);
        commit_raw(&mut engine("hondan"), "hondan");
    });
}

#[test]
fn reading_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw() {
    const NAME: &str =
        "reading_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw";
    catalogs(NAME, || {
        let splits = [
            ("shi", "ORI"),
            ("hyou", "SHI"),
            ("hon", "DANA"),
            ("yomi", "KAKE"),
            ("ki", "DOKU"),
            ("mi", "DOKU"),
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
        for (which, particle) in ["wo", "wo", "ni", "no", "no", "no"].iter().enumerate() {
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
    });
}

#[test]
fn reading_protected_unknown_unicode_long_drafts_and_seven_lines_never_recover_authors() {
    const NAME: &str =
        "reading_protected_unknown_unicode_long_drafts_and_seven_lines_never_recover_authors";
    catalogs(NAME, || {
        let mut seeds = [
            "https://example.invalid/shiori",
            "user+midoku@example.invalid",
            "xyzshiori",
            "🙂栞wo",
            "未読\u{200b}no",
            "ｈｙｏｕｓｈｉ",
            "hyoshi",
            "hondax",
            "本棚xyz",
            "読みかけ\u{fe0f}",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let splits = [
            ("shi", "ori"),
            ("hyou", "shi"),
            ("hon", "dana"),
            ("yomi", "kake"),
            ("ki", "doku"),
            ("mi", "doku"),
        ];
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            for (which, (left, right)) in splits.iter().enumerate() {
                seeds.push(format!("{left}{boundary}{right}"));
                seeds.push(format!("{}{boundary}wo", WORDS[which].3));
            }
        }
        for &(reading, _, _, word) in WORDS {
            for tail in [reading.to_owned(), format!("{word}wo")] {
                let seed = format!("{}{tail}", "前文。".repeat(90));
                assert!(seed.chars().count() > 256);
                seeds.push(seed);
            }
        }
        for seed in seeds {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            no_reading_authors(&ime, &seed);
            commit_raw(&mut ime, &seed);
        }
    });
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-reading-synthetic-model-error"
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
fn reading_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools() {
    const NAME: &str =
        "reading_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools";
    catalogs(NAME, || {
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
                Some(LlmProviderError::HttpStatus(503)),
                Some(LlmProviderError::InvalidResponse),
                Some(LlmProviderError::ResponseTooLarge),
                Some(LlmProviderError::MissingCredentials),
                None,
            ] {
                let mut ime = engine("");
                ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
                ime.seed(reading);
                assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
                assert!(ime.prediction_pending());
                assert!(ime.prediction_error().is_none());
                draft(&ime, reading);
                first_page(&ime, word, CandidateKind::Word);
                authors(&ime, which);
                assert_eq!(ime.candidates(), baseline);
                let deadline = Instant::now() + Duration::from_secs(2);
                while ime.prediction_pending() && Instant::now() < deadline {
                    ime.poll_prediction();
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(!ime.prediction_pending());
                assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
                assert_eq!(
                    ime.prediction_error(),
                    Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
                );
                assert_eq!(ime.candidates(), baseline);
                commit_once(
                    &mut ime,
                    reading,
                    SENTENCES[which].1[0],
                    CandidateKind::Sentence,
                );
            }
        }
    });
}
