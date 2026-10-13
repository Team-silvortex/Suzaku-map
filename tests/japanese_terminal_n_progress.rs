//! A raw final n may be a pending na/ni/no without changing its primary ん.
//! Fixed authored sentences and private snapshots, not native keyboard delivery.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::japanese::romaji_to_hiragana,
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_JAPANESE_TERMINAL_N_PROGRESS_CHILD";
const BUSY: &str = "準備中なので、終わったら連絡します。";
const HOTEL: &str = "ホテルに着いたら連絡します。";
const QUOTE: &str = "引用の出典をもう一度確かめます。";
const HORIZONTAL: [&str; 7] = ["", " ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"];
const JAPANESE_IDS: [&str; 9] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    "org.suzaku.ja.travel",
    "org.suzaku.ja.work",
    "org.suzaku.ja.study",
    "org.suzaku.ja.cooking",
    "org.suzaku.ja.outdoors",
];

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fresh(name: &str, with_packs: bool, body: impl FnOnce()) {
    if std::env::var(CHILD).as_deref() != Ok(name) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fresh {name} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "suzaku-ja-terminal-n-progress-{}",
        std::process::id()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    if with_packs {
        let catalog = packs::recommended();
        for id in JAPANESE_IDS {
            let pack = catalog.iter().find(|p| p.manifest.id == id).unwrap();
            store.install(pack.clone(), false).unwrap();
        }
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert!(report.disabled.is_empty());
    if with_packs {
        assert_eq!(report.loaded, JAPANESE_IDS);
    } else {
        assert!(report.loaded.is_empty());
        assert!(!store.path().exists());
    }
    body();
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
        .unwrap_or_else(|| {
            panic!(
                "missing {kind:?} {text:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert!(ime.candidates().len() <= 12);
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

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let duplicate = ime.commit(CommitOptions { force: true });
    assert!(!duplicate.ok && duplicate.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn progress(prefix: &str, word: &str, completed: &str, kana: &str, sentence: &str) {
    let mut ime = engine(prefix);
    for uppercase in [false, true] {
        for space in HORIZONTAL {
            // Type the vowel, delete it, complete an explicit nn, then delete
            // back to a pending n. Every stage reuses the same editable engine.
            for tail in ["n", completed, "n", "nn", "n"] {
                let seed = format!("{prefix}{tail}{space}");
                let seed = if uppercase {
                    seed.to_ascii_uppercase()
                } else {
                    seed
                };
                ime.seed(&seed);
                draft(&ime, &seed);
                let expected_primary =
                    format!("{word}{}", if tail == completed { kana } else { "ん" });
                assert_eq!(ime.candidates()[0].text, expected_primary, "{seed:?}");
                assert_eq!(ime.candidates()[0].score, 100.0, "{seed:?}");
                if tail == "nn" {
                    assert!(
                        ime.candidates().iter().all(|c| c.text != sentence),
                        "reinterpreted explicit nn in {seed:?}: {:?}",
                        ime.candidates()
                    );
                } else {
                    // A matching authored sentence must be reachable within
                    // the unchanged two-page budget, not necessarily page one.
                    index(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                }
                commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

fn explicit_nasals(prefix: &str, sentence: &str) {
    for tail in ["nn", "xn", "n'", "n’", "ん", "ン"] {
        for space in HORIZONTAL {
            for uppercase in [false, true] {
                let seed = format!("{prefix}{tail}{space}");
                let seed = if uppercase {
                    seed.to_ascii_uppercase()
                } else {
                    seed
                };
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    ime.candidates().iter().all(|c| c.text != sentence),
                    "reinterpreted an explicit nasal in {seed:?}: {:?}",
                    ime.candidates()
                );
                // Other normal word/sentence candidates remain legal.
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        }
    }
}

fn protected(prefix: &str, roman: &str, vowel: &str, sentence: &str) {
    let mut seeds: Vec<String> = [
        format!("xyz{prefix}n"),
        format!("🙂{prefix}n"),
        format!("{prefix}nxyz"),
        format!("{prefix}n!"),
        format!("https://example.invalid/{roman}n"),
        format!("{roman}n@example.invalid"),
        format!("let x = \"{roman}\"; n"),
        format!("{}{prefix}n", "前文。".repeat(90)),
    ]
    .into_iter()
    .collect();
    for space in &HORIZONTAL[1..] {
        // The n and vowel belong to different reading tokens. They may not
        // be joined into na/ni/no to recover the authored sentence.
        seeds.push(format!("{prefix}n{space}{vowel}"));
    }
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("{prefix}{boundary}n"));
        seeds.push(format!("{prefix}n{boundary}"));
        seeds.push(format!("{prefix}n{boundary}{vowel}"));
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates().iter().all(|c| !c.text.ends_with(sentence)),
            "recovered an authored sentence across {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

#[test]
fn terminal_n_primary_conversion_preserves_explicit_nasal_spellings() {
    for (raw, expected) in [
        ("n", "ん"),
        ("N", "ん"),
        ("nn", "ん"),
        ("NN", "ん"),
        ("xn", "ん"),
        ("XN", "ん"),
        ("n'", "ん"),
        ("n’", "ん"),
        ("ん", "ん"),
        ("ン", "ン"),
        ("na", "な"),
        ("ni", "に"),
        ("no", "の"),
        ("引用n", "引用ん"),
        ("引用nn", "引用ん"),
        ("引用n o", "引用ん お"),
        ("引用n\no", "引用ん\nお"),
    ] {
        assert_eq!(romaji_to_hiragana(raw), expected, "{raw:?}");
    }
}

#[test]
fn builtin_pending_n_sentence_survives_vowel_completion_and_backtracking() {
    fresh(
        "builtin_pending_n_sentence_survives_vowel_completion_and_backtracking",
        false,
        || {
            for prefix in ["準備中", "junbichuu", "junbi chuu "] {
                progress(prefix, "準備中", "na", "な", BUSY);
            }
        },
    );
}

#[test]
fn nine_pack_pending_n_preserves_ni_and_no_sentence_choices() {
    fresh(
        "nine_pack_pending_n_preserves_ni_and_no_sentence_choices",
        true,
        || {
            for prefix in ["ホテル", "hoteru", "hoteru "] {
                progress(prefix, "ホテル", "ni", "に", HOTEL);
            }
            for prefix in ["引用", "in'you", "in’you "] {
                progress(prefix, "引用", "no", "の", QUOTE);
            }
        },
    );
}

#[test]
fn builtin_explicit_nasal_spellings_do_not_become_pending_particles() {
    fresh(
        "builtin_explicit_nasal_spellings_do_not_become_pending_particles",
        false,
        || {
            for prefix in ["準備中", "junbichuu"] {
                explicit_nasals(prefix, BUSY);
            }
        },
    );
}

#[test]
fn nine_pack_explicit_nasals_do_not_recover_the_ni_or_no_sentence() {
    fresh(
        "nine_pack_explicit_nasals_do_not_recover_the_ni_or_no_sentence",
        true,
        || {
            for prefix in ["ホテル", "hoteru"] {
                explicit_nasals(prefix, HOTEL);
            }
            for prefix in ["引用", "in'you"] {
                explicit_nasals(prefix, QUOTE);
            }
        },
    );
}

#[test]
fn builtin_pending_n_does_not_cross_reading_or_literal_boundaries() {
    fresh(
        "builtin_pending_n_does_not_cross_reading_or_literal_boundaries",
        false,
        || protected("準備中", "junbichuu", "a", BUSY),
    );
}

#[test]
fn nine_pack_pending_n_preserves_protected_literals_and_boundaries() {
    fresh(
        "nine_pack_pending_n_preserves_protected_literals_and_boundaries",
        true,
        || {
            protected("ホテル", "hoteru", "i", HOTEL);
            protected("引用", "in'you", "o", QUOTE);
        },
    );
}
