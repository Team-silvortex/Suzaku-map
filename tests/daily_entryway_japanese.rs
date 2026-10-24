//! Finite entryway and before-leaving dialogue, frozen against accessory-inclusive data.
//! No lock, weather, personal-location service or household action is exercised.
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
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    languages::{
        japanese::romaji_to_hiragana,
        llm::{LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError},
    },
    lexicon::{
        EntryKind, builtin,
        packs::{self, PackStore},
    },
};

// Five new written Words. 定期入れ is a written synonym of the existing パスケース,
// not a new meaning. The sixth context reuses 鍵 + の + 置き場所 without a new reading.
const BASES: &[(&str, &str, &str, &str)] = &[
    ("kutsubako", "くつばこ", "クツバコ", "靴箱"),
    ("kasatate", "かさたて", "カサタテ", "傘立て"),
    ("kutsubera", "くつべら", "クツベラ", "靴べら"),
    ("teikiire", "ていきいれ", "テイキイレ", "定期入れ"),
    (
        "genkanmatto",
        "げんかんまっと",
        "ゲンカンマット",
        "玄関マット",
    ),
    (
        "kaginookibasho",
        "かぎのおきばしょ",
        "カギノオキバショ",
        "鍵の置き場所",
    ),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "靴箱",
        [
            "靴箱は玄関の右側にあります。",
            "靴箱の上に置いた袋を取ってもらえますか。",
        ],
    ),
    (
        "傘立て",
        [
            "傘立ては玄関の隅にあります。",
            "傘立ての横に小さな袋を置きました。",
        ],
    ),
    (
        "靴べら",
        [
            "靴べらは靴箱の横に掛けてあります。",
            "靴べらを取ってもらえますか。",
        ],
    ),
    (
        "定期入れ",
        [
            "定期入れは鞄の内ポケットにあります。",
            "定期入れを机に置いたままでした。",
        ],
    ),
    (
        "玄関マット",
        [
            "玄関マットは靴箱の前にあります。",
            "玄関マットの横に置いた荷物を取りました。",
        ],
    ),
    (
        "鍵の置き場所",
        [
            "鍵の置き場所を一緒に確認してもらえますか。",
            "鍵の置き場所は玄関の棚です。",
        ],
    ),
];
// Only these near-complete probes are promised reachable, not arbitrary short-root ranks.
const PARTIALS: &[&str] = &[
    "kutsubak",
    "kasatat",
    "kutsuber",
    "teikiir",
    "genkanmatt",
    "kaginookibash",
];
const ALIASES: &[(&str, usize)] = &[
    ("kutubako", 0),
    ("kutubera", 2),
    ("genkannmatto", 4),
    ("genkan'matto", 4),
];
const SPLITS: &[(&str, &str)] = &[
    ("kutsu", "bako"),
    ("kasa", "tate"),
    ("kutsu", "bera"),
    ("teiki", "ire"),
    ("genkan", "matto"),
    ("kagi", "nookibasho"),
];

const CHILD: &str = "SUZAKU_ENTRYWAY_JAPANESE_CATALOG";

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
                stdout.contains(&format!("completed entryway {name} with {mode}")),
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
    let path =
        std::env::temp_dir().join(format!("suzaku-entryway-ja-{}-{nonce}", std::process::id()));
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
    println!("completed entryway {name} with {mode}");
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
fn no_entryway_authors(ime: &XRTabletImeEngine, seed: &str) {
    assert!(
        ime.candidates().iter().all(|c| !SENTENCES
            .iter()
            .flat_map(|(_, ss)| ss)
            .any(|sentence| c.text.contains(*sentence))),
        "unexpected entryway author for {seed:?}: {:?}",
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

fn composed_kind(which: usize) -> CandidateKind {
    if which < 5 {
        CandidateKind::Word
    } else {
        CandidateKind::Sentence
    }
}
// The partial composed context can come from the existing 置き場所 suffix Word
// completion without adding its entire written span to dictionary membership.
fn partial_kind(_: usize) -> CandidateKind {
    CandidateKind::Word
}

fn reachable_authors(ime: &XRTabletImeEngine, which: usize) {
    for sentence in SENTENCES[which].1 {
        index(ime, sentence, CandidateKind::Sentence);
    }
}

fn old_controls() {
    for (seed, word, sentences) in [
        (
            "kutsu",
            "靴",
            [
                "靴を履いたら、すぐに出ます。",
                "靴は歩きやすいものを選びます。",
            ],
        ),
        (
            "mochimono",
            "持ち物",
            [
                "持ち物を出かける前に確認します。",
                "持ち物は机の上にまとめてあります。",
            ],
        ),
        (
            "kutsuhimo",
            "靴ひも",
            [
                "靴ひもを結び直してから歩きます。",
                "靴ひもがほどけたので、少し待ってください。",
            ],
        ),
        (
            "oritatamigasa",
            "折り畳み傘",
            [
                "折り畳み傘を鞄に入れました。",
                "折り畳み傘を忘れずに持って行きます。",
            ],
        ),
        (
            "kagi",
            "鍵",
            [
                "鍵を少し貸してもらえますか。",
                "鍵が見つからないので、一緒に探してください。",
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
        commit_raw(&mut engine(seed), seed);
    }
    // These old carry Words have no continuation pair; do not invent one.
    for (seed, word) in [
        ("ki-ke-su", "キーケース"),
        ("pasuke-su", "パスケース"),
        ("kaban", "鞄"),
        ("saifu", "財布"),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        first_page(&ime, word, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, word, CandidateKind::Word);
        commit_raw(&mut engine(seed), seed);
    }
    if std::env::var(CHILD).as_deref() == Ok("packs") {
        let ime = engine("genkan");
        draft(&ime, "genkan");
        first_page(&ime, "玄関", CandidateKind::Word);
        for sentence in ["玄関で待っています。", "玄関の靴をそろえてください。"]
        {
            first_page(&ime, sentence, CandidateKind::Sentence);
            commit_once(
                &mut engine("genkan"),
                "genkan",
                sentence,
                CandidateKind::Sentence,
            );
        }
        commit_once(&mut engine("genkan"), "genkan", "玄関", CandidateKind::Word);
        commit_raw(&mut engine("genkan"), "genkan");
    }
    println!("ENTRYWAY_JA_OLD_CONTROLS_OK");
}

fn old_source() {
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 633 && ja.continuations().count() >= 361);
    let mut h = 0xcbf29ce484222325;
    for entry in &ja.readings()[..633] {
        let kind = match entry.kind {
            EntryKind::Word => "word",
            EntryKind::Sentence => "sentence",
        };
        for value in [&*entry.reading, &*entry.text, kind] {
            feed(&mut h, value.bytes().chain([0]));
        }
        feed(&mut h, [u8::from(entry.require_separators)]);
    }
    assert_eq!(h, 0x6e7ceaeb47943409);
    let (mut h, mut count) = (0xcbf29ce484222325, 0);
    for (context, sentences) in ja.continuations().take(361) {
        feed(&mut h, context.bytes().chain([0]));
        for sentence in sentences {
            feed(&mut h, sentence.bytes().chain([0]));
        }
        feed(&mut h, [0xff]);
        count += sentences.len();
    }
    assert_eq!((count, h), (721, 0xe9385a4dd097fbb9));
    let raw = include_str!("../data/lexicons/ja.json");
    let reading_anchor =
        "    {\"reading\": \"でんげんたっぷ\", \"text\": \"電源タップ\", \"kind\": \"word\"}";
    let context_anchor = "    [\n      \"充電器の置き場所\",\n      [\"充電器の置き場所を教えてもらえますか。\", \"充電器の置き場所は机の右側です。\"]\n    ]";
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
    assert_eq!(before_raw.len(), 100992);
    let hex = ring::digest::digest(&ring::digest::SHA256, before_raw.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        hex,
        "14ca791a3de16c9c404e380bdcf3769892ad08d51f29835747b06475f1f8791f"
    );
}

#[test]
fn entryway_exact_five_new_readings_six_contexts_twelve_authors_and_composed_key_location() {
    old_source();
    assert_eq!((BASES.len(), SENTENCES.len(), PARTIALS.len()), (6, 6, 6));
    let ja = builtin("ja").unwrap();
    assert!(ja.readings().len() >= 638 && ja.continuations().count() >= 367);
    for (entry, &(_, kana, _, word)) in ja.readings()[633..638].iter().zip(&BASES[..5]) {
        assert_eq!((&*entry.reading, &*entry.text), (kana, word));
        assert_eq!(entry.kind, EntryKind::Word);
        assert!(!entry.require_separators);
        assert!(
            ja.readings()[..633]
                .iter()
                .all(|old| old.text != word && old.reading != kana)
        );
        assert_eq!(ja.readings().iter().filter(|e| e.text == word).count(), 1);
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
        }
    }
    let (_, composed_kana, _, composed) = BASES[5];
    assert!(
        ja.readings()
            .iter()
            .all(|e| e.reading != composed_kana && e.text != composed)
    );
    for word in ["鍵", "置き場所"] {
        assert_eq!(
            ja.readings()
                .iter()
                .filter(|e| e.text == word && e.kind == EntryKind::Word)
                .count(),
            1
        );
    }
    for ((context, values), &(word, expected)) in
        ja.continuations().skip(361).take(6).zip(SENTENCES)
    {
        assert_eq!(context, word);
        assert!(ja.continuations().take(361).all(|(old, _)| old != word));
        assert_eq!(ja.continuations().filter(|(c, _)| *c == word).count(), 1);
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
                    .all(|(old, ss)| old != word
                        && ss.iter().all(|s| !expected.contains(&s.as_str())))
            );
        }
    }
    assert_eq!(
        ja.continuations()
            .take(367)
            .map(|(_, ss)| ss.len())
            .sum::<usize>(),
        733
    );
}

#[test]
fn entryway_old_633_readings_flags_361_contexts_721_authors_raw_and_device_workflows_stay() {
    const NAME: &str =
        "entryway_old_633_readings_flags_361_contexts_721_authors_raw_and_device_workflows_stay";
    catalogs(NAME, || {
        old_source();
        old_controls();
        for &(roman, kana, katakana, _) in BASES {
            assert_eq!(romaji_to_hiragana(roman), kana);
            assert_eq!(romaji_to_hiragana(&roman.to_ascii_uppercase()), kana);
            assert_eq!(romaji_to_hiragana(katakana), katakana);
        }
        for &(alias, which) in ALIASES {
            let kana = alias
                .split_whitespace()
                .map(romaji_to_hiragana)
                .collect::<String>();
            assert_eq!(kana, BASES[which].1, "{alias}");
        }
        assert_eq!(romaji_to_hiragana("teikire"), "ていきれ");
        assert_eq!(romaji_to_hiragana("genkanmato"), "げんかんまと");
        assert_eq!(romaji_to_hiragana("kiikeesu"), "きいけえす");
        assert_eq!(romaji_to_hiragana("genkanamatto"), "げんかなまっと");
    });
}

#[test]
fn entryway_whole_case_kana_composed_sentence_aliases_and_six_near_partials_commit_once() {
    const NAME: &str =
        "entryway_whole_case_kana_composed_sentence_aliases_and_six_near_partials_commit_once";
    catalogs(NAME, || {
        old_controls();
        for (which, &(roman, kana, katakana, context)) in BASES.iter().enumerate() {
            for seed in [
                roman.to_owned(),
                roman.to_ascii_uppercase(),
                kana.to_owned(),
                context.to_owned(),
            ] {
                let ime = engine(&seed);
                draft(&ime, &seed);
                let kind = if seed == context {
                    raw_kind(&ime, &seed)
                } else {
                    composed_kind(which)
                };
                if which < 5 {
                    first_page(&ime, context, kind);
                } else {
                    index(&ime, context, kind);
                }
                reachable_authors(&ime, which);
                commit_once(&mut engine(&seed), &seed, context, kind);
                for sentence in SENTENCES[which].1 {
                    commit_once(&mut engine(&seed), &seed, sentence, CandidateKind::Sentence);
                }
                commit_raw(&mut engine(&seed), &seed);
            }
            let ime = engine(katakana);
            draft(&ime, katakana);
            if katakana == context {
                reachable_authors(&ime, which);
            } else {
                no_entryway_authors(&ime, katakana);
            }
            commit_raw(&mut engine(katakana), katakana);
            let partial = PARTIALS[which];
            let ime = engine(partial);
            draft(&ime, partial);
            index(&ime, context, partial_kind(which));
            reachable_authors(&ime, which);
            commit_once(&mut engine(partial), partial, context, partial_kind(which));
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
            first_page(&ime, BASES[which].3, CandidateKind::Word);
            reachable_authors(&ime, which);
            commit_once(&mut engine(seed), seed, BASES[which].3, CandidateKind::Word);
            for sentence in SENTENCES[which].1 {
                commit_once(&mut engine(seed), seed, sentence, CandidateKind::Sentence);
            }
            commit_raw(&mut engine(seed), seed);
        }
    });
}

#[test]
fn entryway_six_horizontal_gaps_and_adopted_words_composed_context_reseed_keep_raw() {
    const NAME: &str =
        "entryway_six_horizontal_gaps_and_adopted_words_composed_context_reseed_keep_raw";
    catalogs(NAME, || {
        old_controls();
        for separator in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
            for (which, (left, right)) in SPLITS.iter().enumerate() {
                let seed = format!(
                    "{separator}{left}{separator}{}{separator}",
                    right.to_ascii_uppercase()
                );
                let ime = engine(&seed);
                draft(&ime, &seed);
                index(&ime, BASES[which].3, composed_kind(which));
                reachable_authors(&ime, which);
                commit_once(
                    &mut engine(&seed),
                    &seed,
                    BASES[which].3,
                    composed_kind(which),
                );
                commit_raw(&mut engine(&seed), &seed);
            }
        }
        for (which, particle) in ["ha", "ha", "ha", "ha", "ha", "wo"].iter().enumerate() {
            let (reading, _, _, context) = BASES[which];
            let mut ime = engine(reading);
            ime.select_candidate(index(&ime, context, composed_kind(which)));
            assert_eq!(ime.selected_completion_text(true), Some(context));
            assert_eq!(ime.snapshot().seed_text, reading);
            assert!(ime.snapshot().committed_text.is_empty());
            for separator in ["", " ", "\u{3000}"] {
                let seed = format!("{context}{separator}{particle}");
                ime.seed(&seed);
                draft(&ime, &seed);
                index(&ime, SENTENCES[which].1[0], CandidateKind::Sentence);
                commit_once(
                    &mut engine(&seed),
                    &seed,
                    SENTENCES[which].1[0],
                    CandidateKind::Sentence,
                );
                commit_raw(&mut engine(&seed), &seed);
                ime.seed(reading);
                draft(&ime, reading);
                index(&ime, context, composed_kind(which));
                reachable_authors(&ime, which);
            }
        }
        // Reuse the old Word, not a new dictionary entry for the whole context.
        let mut ime = engine("kagi");
        ime.select_candidate(index(&ime, "鍵", CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some("鍵"));
        assert_eq!(ime.snapshot().seed_text, "kagi");
        assert!(ime.snapshot().committed_text.is_empty());
        for separator in ["", " ", "\u{3000}"] {
            let seed = format!("鍵{separator}nookibasho");
            ime.seed(&seed);
            draft(&ime, &seed);
            index(&ime, BASES[5].3, CandidateKind::Sentence);
            reachable_authors(&ime, 5);
            for sentence in SENTENCES[5].1 {
                commit_once(&mut engine(&seed), &seed, sentence, CandidateKind::Sentence);
            }
            commit_raw(&mut engine(&seed), &seed);
        }
    });
}

#[test]
fn entryway_unknown_nasal_boundaries_long_vowels_long_drafts_and_seven_lines_stay_raw() {
    const NAME: &str =
        "entryway_unknown_nasal_boundaries_long_vowels_long_drafts_and_seven_lines_stay_raw";
    catalogs(NAME, || {
        old_controls();
        let mut seeds = [
            "https://example.invalid/kutsubako",
            "user+teikiire@example.invalid",
            "xyzkasatate",
            "🙂靴べらha",
            "玄関マット\u{200b}ha",
            "ｔｅｉｋｉｉｒｅ",
            "teikire",
            "ていきれ",
            "genkanmato",
            "げんかんまと",
            "genkanamatto",
            "げんかなまっと",
            "kiikeesu",
            "クツバコ",
            "カサタテ",
            "クツベラ",
            "テイキイレ",
            "ゲンカンマット",
            "カギノオキバショ",
            "鍵の置き場所xyz",
            "玄関マット\u{fe0f}",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        for boundary in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            for (which, (left, right)) in SPLITS.iter().enumerate() {
                seeds.push(format!("{left}{boundary}{right}"));
                seeds.push(format!("{}{boundary}ha", BASES[which].3));
            }
        }
        for &(reading, _, _, context) in BASES {
            for tail in [reading.to_owned(), format!("{context}ha")] {
                let seed = format!("{}{tail}", "前文。".repeat(90));
                assert!(seed.chars().count() > 256);
                seeds.push(seed);
            }
        }
        for seed in seeds {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            no_entryway_authors(&ime, &seed);
            commit_raw(&mut ime, &seed);
        }
    });
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-entryway-synthetic-model-error"
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
fn entryway_pending_disabled_five_typed_failures_and_empty_models_preserve_full_local_pools() {
    const NAME: &str =
        "entryway_pending_disabled_five_typed_failures_and_empty_models_preserve_full_local_pools";
    catalogs(NAME, || {
        old_controls();
        for (which, &(reading, _, _, context)) in BASES.iter().enumerate() {
            let baseline = engine(reading).candidates().to_vec();
            let mut disabled = engine(reading);
            disabled.configure_prediction(None);
            assert_eq!(disabled.prediction_status(), PredictionStatus::Disabled);
            assert!(disabled.prediction_error().is_none());
            assert_eq!(disabled.candidates(), baseline);
            reachable_authors(&disabled, which);
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
                None,
            ] {
                let mut ime = engine("");
                ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
                ime.seed(reading);
                assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
                assert!(ime.prediction_pending() && ime.prediction_error().is_none());
                draft(&ime, reading);
                index(&ime, context, composed_kind(which));
                reachable_authors(&ime, which);
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
