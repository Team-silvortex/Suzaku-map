//! Finite bedding dialogue; frozen against the kitchen-inclusive 619/345/689 baseline.
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

// Reuse the existing 目覚まし Word at index 322; do not duplicate it.
const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shi-tsu", "しーつ", "シーツ", "シーツ"),
    ("moufu", "もうふ", "モウフ", "毛布"),
    ("pajama", "ぱじゃま", "パジャマ", "パジャマ"),
    ("mezamashi", "めざまし", "メザマシ", "目覚まし"),
];
const NEW_READINGS: &[(&str, &str)] = &[
    ("しーつ", "シーツ"),
    ("もうふ", "毛布"),
    ("ぱじゃま", "パジャマ"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "シーツ",
        [
            "シーツを取り替えてから布団を整えます。",
            "シーツの端をそろえてたたみます。",
        ],
    ),
    (
        "毛布",
        [
            "毛布をたたんでベッドの足元に置きます。",
            "毛布は押し入れの上の段にあります。",
        ],
    ),
    (
        "パジャマ",
        [
            "パジャマを出してから着替えます。",
            "パジャマは引き出しの手前にあります。",
        ],
    ),
    (
        "目覚まし",
        [
            "目覚ましをベッドの横に置きます。",
            "目覚ましの時刻を確かめてから寝ます。",
        ],
    ),
];
// All four finite branches retain authors at 3/4 in the existing six-row pool.
const PARTIALS: &[&str] = &["shi-ts", "mouf", "pajam", "mezamash"];
const ALIASES: &[(&str, usize)] = &[
    ("shi-tu", 0),
    ("si-tsu", 0),
    ("si-tu", 0),
    ("pazyama", 2),
    ("pajyama", 2),
    ("mezamasi", 3),
];
const SPLITS: &[(&str, &str)] = &[
    ("shi", "-tsu"),
    ("mou", "fu"),
    ("pa", "jama"),
    ("meza", "mashi"),
];
const CHILD: &str = "SUZAKU_BEDDING_JAPANESE_CATALOG";

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalogs(name: &str, body: impl FnOnce()) {
    let Ok(mode) = std::env::var(CHILD) else {
        let outputs = ["builtin", "packs"].map(|mode| {
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
        let details = outputs
            .iter()
            .map(|(mode, o)| {
                format!(
                    "{mode}: {}\n{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(outputs.iter().all(|(_, o)| o.status.success()), "{details}");
        for (mode, output) in outputs {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
                "{stdout}"
            );
            assert!(
                stdout.contains(&format!("completed bedding {name} with {mode}")),
                "{stdout}"
            );
        }
        return;
    };
    assert!(matches!(mode.as_str(), "builtin" | "packs"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("suzaku-bedding-ja-{}-{nonce}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let owned = Owned(directory);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    assert!(store.list().unwrap().is_empty());
    let selected = packs::recommended()
        .into_iter()
        .filter(|p| mode == "packs" && p.manifest.language == "ja")
        .collect::<Vec<_>>();
    let expected = if mode == "packs" { 9 } else { 0 };
    let ids = selected
        .iter()
        .map(|p| p.manifest.id.clone())
        .collect::<HashSet<_>>();
    assert_eq!((selected.len(), ids.len()), (expected, expected));
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
    println!("completed bedding {name} with {mode}");
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
fn no_bedding_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected bedding author for {seed:?}: {:?}",
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
fn feed(hash: &mut u64, bytes: impl IntoIterator<Item = u8>) {
    for byte in bytes {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
}

#[test]
fn bedding_exact_three_new_readings_four_contexts_eight_authors_and_reused_alarm() {
    assert_eq!(
        (
            WORDS.len(),
            NEW_READINGS.len(),
            SENTENCES.len(),
            PARTIALS.len()
        ),
        (4, 3, 4, 4)
    );
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 622 && ja.continuations().count() >= 349);
    for (entry, &(kana, word)) in ja.readings()[619..622].iter().zip(NEW_READINGS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(
            ja.readings()[..619]
                .iter()
                .all(|old| old.text != word && old.reading != kana)
        );
    }
    let old = &ja.readings()[322];
    assert_eq!(
        (&*old.reading, &*old.text, old.kind, old.require_separators),
        ("めざまし", "目覚まし", EntryKind::Word, false)
    );
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
                .take(345)
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
        ja.continuations().skip(345).take(4).zip(SENTENCES)
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
        for recommended in packs::recommended()
            .into_iter()
            .filter(|p| p.manifest.language == "ja")
        {
            assert!(
                recommended
                    .lexicon
                    .continuations()
                    .all(|(_, old)| old.iter().all(|s| !expected.contains(&s.as_str())))
            );
        }
    }
    assert_eq!(
        ja.continuations()
            .take(349)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        697
    );
}

#[test]
fn bedding_old_619_readings_flags_345_contexts_689_authors_raw_and_protected_pools_stay() {
    const NAME: &str =
        "bedding_old_619_readings_flags_345_contexts_689_authors_raw_and_protected_pools_stay";
    catalogs(NAME, || {
        let ja = builtin("ja").unwrap();
        assert!(ja.readings().len() >= 619 && ja.continuations().count() >= 345);
        let mut h = 0xcbf29ce484222325;
        for entry in &ja.readings()[..619] {
            let kind = match entry.kind {
                EntryKind::Word => "word",
                EntryKind::Sentence => "sentence",
            };
            for value in [&*entry.reading, &*entry.text, kind] {
                feed(&mut h, value.bytes().chain([0]));
            }
            feed(&mut h, [u8::from(entry.require_separators)]);
        }
        assert_eq!(h, 0x2e317ce28ccc066e);
        let (mut h, mut count) = (0xcbf29ce484222325, 0);
        for (context, sentences) in ja.continuations().take(345) {
            feed(&mut h, context.bytes().chain([0]));
            for sentence in sentences {
                feed(&mut h, sentence.bytes().chain([0]));
            }
            feed(&mut h, [0xff]);
            count += sentences.len();
        }
        assert_eq!((count, h), (689, 0xb43dd84189080fe5));
        // Project only the append tails after the exact old final entries.
        // This locks original bytes, including whitespace/flags/order, while
        // remaining a historical-prefix check for later finite batches.
        let raw = include_str!("../data/lexicons/ja.json");
        let reading_anchor =
            "    {\"reading\": \"みずきりかご\", \"text\": \"水切りかご\", \"kind\": \"word\"}";
        let context_anchor = "    [\n      \"水切りかご\",\n      [\"水切りかごに洗った食器を並べます。\", \"水切りかごの食器を棚に戻します。\"]\n    ]";
        assert_eq!(raw.matches(reading_anchor).count(), 1);
        assert_eq!(raw.matches(context_anchor).count(), 1);
        let reading_end = raw.find(reading_anchor).unwrap() + reading_anchor.len();
        let context_start = raw.find("  \"continuations\": [").unwrap();
        let context_end = raw.find(context_anchor).unwrap() + context_anchor.len();
        let before_raw = format!(
            "{}\n  ],\n{}\n  ]\n}}\n",
            &raw[..reading_end],
            &raw[context_start..context_end]
        );
        assert_eq!(before_raw.len(), 97385);
        let digest = ring::digest::digest(&ring::digest::SHA256, before_raw.as_bytes());
        let hex = digest
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "d9775473653d87e7726b931b5bde0bb579c694e927f14f64074d5356741df87b"
        );
        for &(roman, kana, katakana, _) in WORDS {
            assert_eq!(romaji_to_hiragana(roman), kana);
            assert_eq!(romaji_to_hiragana(&roman.to_ascii_uppercase()), kana);
            assert_eq!(romaji_to_hiragana(katakana), katakana);
        }
        for &(alias, which) in ALIASES {
            assert_eq!(romaji_to_hiragana(alias), WORDS[which].1);
        }
        assert_eq!(romaji_to_hiragana("shiitsu"), "しいつ");
        assert_eq!(romaji_to_hiragana("moofu"), "もおふ");
        let builtin_hashes = [
            [0x841106870d8427ce, 0x9081163b2960ca0b, 0x5b437e702b3ec28d],
            [0x1ecd6873b9bda734, 0x61f50f4dd8b2eef7, 0x1338d234f22a1b1d],
            [0x8ccfc5062ae38478, 0x8ccfc5062ae38478, 0x8ccfc5062ae38478],
            [0x65bbe9e06f6ee96c, 0x1d0dcbedb02292e3, 0x1d0dcbedb02292e3],
            [0xa9dc30a73aac7c01, 0x26bf1cd449a150a6, 0xf2975326e6116471],
        ];
        let pack_hashes = [
            [0x841106870d8427ce, 0x9081163b2960ca0b, 0xc0ef0013ebe19615],
            [0x1ecd6873b9bda734, 0x61f50f4dd8b2eef7, 0x1338d234f22a1b1d],
            [0x8ccfc5062ae38478, 0x8ccfc5062ae38478, 0x8ccfc5062ae38478],
            [0x5e3642fec2a057a8, 0xc9dd4b61183cfa46, 0x23f015bbc8c7dc22],
            [0xf8786188b49aee78, 0xff60243ff461c78f, 0x55718c205adb25b8],
        ];
        let expected = if std::env::var(CHILD).as_deref() == Ok("packs") {
            pack_hashes
        } else {
            builtin_hashes
        };
        for (which, seed) in ["ま", "ma", "マ", "まど", "mado"].iter().enumerate() {
            for (column, limit) in [6, 9, 12].into_iter().enumerate() {
                let pool = offline(
                    "ja",
                    seed,
                    "",
                    JapaneseLanguagePlugin.direct_candidates(seed, 1.0).unwrap(),
                    limit,
                );
                let mut h = 0xcbf29ce484222325;
                for row in pool {
                    for field in [
                        row.text.as_str(),
                        row.label.as_str(),
                        row.kind.id(),
                        row.source.id(),
                    ] {
                        feed(&mut h, field.bytes().chain([0]));
                    }
                    feed(&mut h, row.score.to_bits().to_le_bytes());
                    feed(&mut h, [0xff]);
                }
                assert_eq!(h, expected[which][column], "protected {seed}/{limit}");
            }
        }
        for (seed, word, sentences) in [
            (
                "shokki",
                "食器",
                ["食器を一緒に片付けましょう。", "食器は私が洗います。"],
            ),
            (
                "mizuyari",
                "水やり",
                [
                    "水やりをしたら、じょうろを片付けます。",
                    "水やりの順番を家族と決めます。",
                ],
            ),
        ] {
            let ime = engine(seed);
            draft(&ime, seed);
            first_page(&ime, word, CandidateKind::Word);
            for sentence in sentences {
                first_page(&ime, sentence, CandidateKind::Sentence);
            }
            commit_once(&mut engine(seed), seed, word, CandidateKind::Word);
        }
        if std::env::var(CHILD).as_deref() == Ok("packs") {
            for (seed, word, sentences) in [
                (
                    "manaita",
                    "まな板",
                    ["まな板を出しておきます。", "まな板はどこにしまいましたか。"],
                ),
                (
                    "furaipan",
                    "フライパン",
                    [
                        "フライパンはどこにありますか。",
                        "フライパンで野菜を炒めます。",
                    ],
                ),
            ] {
                let ime = engine(seed);
                draft(&ime, seed);
                first_page(&ime, word, CandidateKind::Word);
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                }
            }
        }
    });
}

#[test]
fn bedding_whole_case_kana_canonical_aliases_and_four_bounded_partials_commit_once() {
    const NAME: &str =
        "bedding_whole_case_kana_canonical_aliases_and_four_bounded_partials_commit_once";
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
                no_bedding_authors(&ime, katakana);
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
            assert!(
                six.iter()
                    .any(|c| c.text == word && c.kind == CandidateKind::Word)
            );
            assert!(
                six.iter()
                    .any(|c| c.text == partial && c.kind == CandidateKind::Literal)
            );
            for (offset, sentence) in SENTENCES[which].1.iter().enumerate() {
                assert_eq!(
                    six.iter()
                        .position(|c| c.text == *sentence && c.kind == CandidateKind::Sentence),
                    Some(3 + offset),
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
    });
}

#[test]
fn bedding_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw() {
    const NAME: &str =
        "bedding_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw";
    catalogs(NAME, || {
        for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
            for (which, (left, right)) in SPLITS.iter().enumerate() {
                let seed = format!(
                    "{separator}{left}{separator}{}{separator}",
                    right.to_ascii_uppercase()
                );
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
        for (which, particle) in ["wo", "wo", "wo", "wo"].iter().enumerate() {
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
fn bedding_protected_unknown_long_vowels_katakana_long_drafts_and_seven_lines_stay_raw() {
    const NAME: &str =
        "bedding_protected_unknown_long_vowels_katakana_long_drafts_and_seven_lines_stay_raw";
    catalogs(NAME, || {
        let mut seeds = [
            "https://example.invalid/moufu",
            "user+shi-tsu@example.invalid",
            "xyzmezamashi",
            "🙂毛布wo",
            "シーツ\u{200b}wo",
            "ｐａｊａｍａ",
            "shiitsu",
            "しいつ",
            "シイツ",
            "moofu",
            "もおふ",
            "モーフ",
            "モウフ",
            "メザマシ",
            "目覚ましxyz",
            "毛布\u{fe0f}",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            for (which, (left, right)) in SPLITS.iter().enumerate() {
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
            no_bedding_authors(&ime, &seed);
            commit_raw(&mut ime, &seed);
        }
    });
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-bedding-synthetic-model-error"
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
fn bedding_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools() {
    const NAME: &str =
        "bedding_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools";
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
                assert!(ime.prediction_pending() && ime.prediction_error().is_none());
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
