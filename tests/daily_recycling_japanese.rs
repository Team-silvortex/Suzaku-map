//! Finite recycling dialogue, frozen against the actual 0.9.5 source.
//! This supplies input expressions, not local collection rules or live services.
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

const WORDS: &[(&str, &str, &str, &str)] = &[
    ("shuushuubi", "しゅうしゅうび", "シュウシュウビ", "収集日"),
    ("kaishuubako", "かいしゅうばこ", "カイシュウバコ", "回収箱"),
    ("danbo-ru", "だんぼーる", "ダンボール", "段ボール"),
    (
        "pettobotoru",
        "ぺっとぼとる",
        "ペットボトル",
        "ペットボトル",
    ),
    ("namagomi", "なまごみ", "ナマゴミ", "生ごみ"),
    ("sodaigomi", "そだいごみ", "ソダイゴミ", "粗大ごみ"),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "収集日",
        [
            "収集日を確認してからごみを出します。",
            "収集日はカレンダーに書いてあります。",
        ],
    ),
    (
        "回収箱",
        [
            "回収箱の場所を確認します。",
            "回収箱に入れられる物を確かめます。",
        ],
    ),
    (
        "段ボール",
        [
            "段ボールをたたんで片付けます。",
            "段ボールの出し方を確認します。",
        ],
    ),
    (
        "ペットボトル",
        [
            "ペットボトルの分別方法を確認します。",
            "ペットボトルを入れる袋はどこですか。",
        ],
    ),
    (
        "生ごみ",
        [
            "生ごみを入れた袋の口を閉じます。",
            "生ごみの出し方を確認します。",
        ],
    ),
    (
        "粗大ごみ",
        [
            "粗大ごみの出し方を調べます。",
            "粗大ごみの回収について問い合わせます。",
        ],
    ),
];
// Only these unambiguous near-complete probes are promised; short roots such as
// shuu compete with older Words and are deliberately not ranked by this suite.
const PARTIALS: &[&str] = &[
    "shuushuub",
    "kaishuubak",
    "danbo-r",
    "pettobotor",
    "namagom",
    "sodaigom",
];
const ALIASES: &[(&str, usize)] = &[
    ("syuusyuubi", 0),
    ("kaisyuubako", 1),
    ("dannbo-ru", 2),
    ("dan'bo-ru", 2),
];
const SPLITS: &[(&str, &str)] = &[
    ("shuushuu", "bi"),
    ("kaishuu", "bako"),
    ("dan", "bo-ru"),
    ("petto", "botoru"),
    ("nama", "gomi"),
    ("sodai", "gomi"),
];
const CHILD: &str = "SUZAKU_RECYCLING_JAPANESE_CATALOG";

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
                stdout.contains(&format!("completed recycling {name} with {mode}")),
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
    let path = std::env::temp_dir().join(format!(
        "suzaku-recycling-ja-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
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
    println!("completed recycling {name} with {mode}");
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
fn no_recycling_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected recycling author for {seed:?}: {:?}",
        ime.candidates()
    );
}
fn commit_once(ime: &mut XRTabletImeEngine, seed: &str, text: &str, kind: CandidateKind) {
    assert_eq!(ime.snapshot().seed_text, seed);
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
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
fn recycling_exact_six_new_readings_six_contexts_and_twelve_authors() {
    assert_eq!((WORDS.len(), SENTENCES.len(), PARTIALS.len()), (6, 6, 6));
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 628 && ja.continuations().count() >= 355);
    for (entry, &(_, kana, _, word)) in ja.readings()[622..628].iter().zip(WORDS) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(
            ja.readings()[..622]
                .iter()
                .all(|old| old.text != word && old.reading != kana)
        );
        assert_eq!(ja.readings().iter().filter(|e| e.text == word).count(), 1);
        assert!(
            ja.continuations()
                .take(349)
                .all(|(context, _)| context != word)
        );
        assert_eq!(
            ja.continuations()
                .filter(|(context, _)| *context == word)
                .count(),
            1
        );
        for p in packs::recommended()
            .into_iter()
            .filter(|p| p.manifest.language == "ja")
        {
            assert!(
                p.lexicon
                    .readings()
                    .iter()
                    .all(|e| e.text != word && e.reading != kana)
            );
            assert!(
                p.lexicon
                    .continuations()
                    .all(|(context, _)| context != word)
            );
        }
    }
    for ((context, values), &(word, expected)) in
        ja.continuations().skip(349).take(6).zip(SENTENCES)
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
        for p in packs::recommended()
            .into_iter()
            .filter(|p| p.manifest.language == "ja")
        {
            assert!(
                p.lexicon
                    .continuations()
                    .all(|(_, old)| old.iter().all(|s| !expected.contains(&s.as_str())))
            );
        }
    }
    assert_eq!(
        ja.continuations()
            .take(355)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        709
    );
}

#[test]
fn recycling_old_622_readings_flags_349_contexts_697_authors_raw_and_protected_pools_stay() {
    const NAME: &str =
        "recycling_old_622_readings_flags_349_contexts_697_authors_raw_and_protected_pools_stay";
    catalogs(NAME, || {
        let ja = builtin("ja").unwrap();
        assert!(ja.readings().len() >= 622 && ja.continuations().count() >= 349);
        let mut h = 0xcbf29ce484222325;
        for entry in &ja.readings()[..622] {
            let kind = match entry.kind {
                EntryKind::Word => "word",
                EntryKind::Sentence => "sentence",
            };
            for value in [&*entry.reading, &*entry.text, kind] {
                feed(&mut h, value.bytes().chain([0]));
            }
            feed(&mut h, [u8::from(entry.require_separators)]);
        }
        assert_eq!(h, 0x619ac1d84a5860a7);
        let (mut h, mut count) = (0xcbf29ce484222325, 0);
        for (context, sentences) in ja.continuations().take(349) {
            feed(&mut h, context.bytes().chain([0]));
            for sentence in sentences {
                feed(&mut h, sentence.bytes().chain([0]));
            }
            feed(&mut h, [0xff]);
            count += sentences.len();
        }
        assert_eq!((count, h), (697, 0x5fc45bfbf3441b4c));
        let raw = include_str!("../data/lexicons/ja.json");
        let reading_anchor =
            "    {\"reading\": \"ぱじゃま\", \"text\": \"パジャマ\", \"kind\": \"word\"}";
        let context_anchor = "    [\n      \"目覚まし\",\n      [\"目覚ましをベッドの横に置きます。\", \"目覚ましの時刻を確かめてから寝ます。\"]\n    ]";
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
        assert_eq!(before_raw.len(), 98197);
        let hex = ring::digest::digest(&ring::digest::SHA256, before_raw.as_bytes())
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "8d4c2f1d8a48eddcf046ec271808bf12441744428c666a48b504b6fd02290dd5"
        );
        for &(roman, kana, katakana, _) in WORDS {
            assert_eq!(romaji_to_hiragana(roman), kana);
            assert_eq!(romaji_to_hiragana(&roman.to_ascii_uppercase()), kana);
            assert_eq!(romaji_to_hiragana(katakana), katakana);
        }
        for &(alias, which) in ALIASES {
            assert_eq!(romaji_to_hiragana(alias), WORDS[which].1);
        }
        assert_eq!(romaji_to_hiragana("danbooru"), "だんぼおる");
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
                "gomidashi",
                "ごみ出し",
                [
                    "ごみ出しは朝のうちに済ませます。",
                    "ごみ出しの日を確認しましょう。",
                ],
            ),
            (
                "bunbetsu",
                "分別",
                ["分別の方法を確認します。", "分別してから袋に入れましょう。"],
            ),
            (
                "shigengomi",
                "資源ごみ",
                [
                    "資源ごみの出し方を確認しました。",
                    "資源ごみの回収日はいつですか。",
                ],
            ),
            (
                "moufu",
                "毛布",
                [
                    "毛布をたたんでベッドの足元に置きます。",
                    "毛布は押し入れの上の段にあります。",
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
            for (seed, word) in [
                ("gomibako", "ごみ箱"),
                ("shokutaku", "食卓"),
                ("makura", "枕"),
            ] {
                let ime = engine(seed);
                draft(&ime, seed);
                first_page(&ime, word, CandidateKind::Word);
                commit_once(&mut engine(seed), seed, word, CandidateKind::Word);
            }
        }
    });
}

#[test]
fn recycling_whole_case_kana_canonical_aliases_and_six_bounded_partials_commit_once() {
    const NAME: &str =
        "recycling_whole_case_kana_canonical_aliases_and_six_bounded_partials_commit_once";
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
                no_recycling_authors(&ime, katakana);
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
fn recycling_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw() {
    const NAME: &str =
        "recycling_six_horizontal_gaps_and_known_word_adoption_reseed_particles_keep_raw";
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
        for (which, particle) in ["wo", "no", "wo", "no", "wo", "no"].iter().enumerate() {
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
fn recycling_protected_unknown_long_vowels_katakana_long_drafts_and_seven_lines_stay_raw() {
    const NAME: &str =
        "recycling_protected_unknown_long_vowels_katakana_long_drafts_and_seven_lines_stay_raw";
    catalogs(NAME, || {
        let mut seeds = [
            "https://example.invalid/pettobotoru",
            "user+danbo-ru@example.invalid",
            "xyznamagomi",
            "🙂粗大ごみno",
            "回収箱\u{200b}no",
            "ｐｅｔｔｏｂｏｔｏｒｕ",
            "danbooru",
            "だんぼおる",
            "ダンボオル",
            "シュウシュウビ",
            "カイシュウバコ",
            "ダンボール",
            "ナマゴミ",
            "ソダイゴミ",
            "生ごみxyz",
            "収集日\u{fe0f}",
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
            no_recycling_authors(&ime, &seed);
            commit_raw(&mut ime, &seed);
        }
    });
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-recycling-synthetic-model-error"
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
fn recycling_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools() {
    const NAME: &str = "recycling_pending_absent_typed_failures_timeouts_and_empty_models_preserve_full_local_pools";
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
