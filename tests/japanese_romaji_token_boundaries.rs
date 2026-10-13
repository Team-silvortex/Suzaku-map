//! Reading separators preserve independent unfinished syllables. Editing below
//! uses engine seed snapshots, not physical keystrokes or a desktop input method.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_JAPANESE_ROMAJI_BOUNDARIES_CHILD";
const STUDY: &str = "日本語を勉強したいです。";
const DONE: &str = "完了したら連絡してください。";
const QUOTE: &str = "引用した部分が分かるように印を付けます。";
const HORIZONTAL: [&str; 7] = [
    " ", "\t", "\u{a0}", "\u{2003}", "\u{202f}", "\u{3000}", " \t",
];
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
        "suzaku-ja-romaji-token-boundaries-{}",
        std::process::id()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    if with_packs {
        let catalog = packs::recommended();
        for id in JAPANESE_IDS {
            store
                .install(
                    catalog
                        .iter()
                        .find(|p| p.manifest.id == id)
                        .unwrap()
                        .clone(),
                    false,
                )
                .unwrap();
        }
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), if with_packs { 9 } else { 0 });
    if !with_packs {
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
                "missing {text:?}/{kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        })
}

fn draft(ime: &XRTabletImeEngine, seed: &str, progress: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert!(ime.candidates().len() <= 12);
    index(ime, seed, CandidateKind::Literal);
    assert_eq!(ime.candidates()[0].text, progress, "{seed:?}");
    assert_eq!(ime.candidates()[0].score, 100.0, "{seed:?}");
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

fn separated(prefix: &str, written: &str) {
    for space in HORIZONTAL {
        for (left, right, progress) in [
            ("s", "h", "sh"),
            ("s", "hi", "sひ"),
            ("sh", "t", "sht"),
            ("sh", "ta", "shた"),
        ] {
            for trailing in ["", space] {
                for uppercase in [false, true] {
                    let seed = format!("{prefix}{left}{space}{right}{trailing}");
                    let seed = if uppercase {
                        seed.to_ascii_uppercase()
                    } else {
                        seed
                    };
                    let mut ime = engine(&seed);
                    draft(&ime, &seed, &format!("{written}{progress}"));
                    assert_eq!(ime.candidates()[0].kind, CandidateKind::Unspecified);
                    // Only exact authored sentences are excluded. Legitimate
                    // longer words such as 下書き are not banned by substring.
                    assert!(
                        ime.candidates()
                            .iter()
                            .all(|c| ![STUDY, DONE, QUOTE].contains(&c.text.as_str())),
                        "merged independent syllables from {seed:?}: {:?}",
                        ime.candidates()
                    );
                    commit_once(&mut ime, &seed, CandidateKind::Literal);
                }
            }
        }
    }
}

fn editing(reading: &str, word: &str, written: &str, sentence: &str) {
    let mut ime = engine(reading);
    ime.select_candidate(index(&ime, word, CandidateKind::Word));
    assert_eq!(ime.selected_completion_text(true), Some(word));
    assert!(ime.snapshot().committed_text.is_empty());
    // Delete the final i, then h, then separator; reinsert h and the separator.
    // Keeping a complete shi before a separator must still allow final t -> ta.
    for (tail, progress, visible) in [
        ("s hi", "sひ", false),
        ("s h", "sh", false),
        ("s ", "s", true),
        ("s", "s", true),
        ("sh", "sh", true),
        ("s h", "sh", false),
        (" sh", "sh", true),
        (" shi t", "しt", true),
        (" shi t\t", "しt", true),
        (" sh t", "sht", false),
        (" sh ta", "shた", false),
        (" shit", "しt", true),
        (" shita", "した", true),
    ] {
        let seed = format!("{written}{tail}");
        ime.seed(&seed);
        draft(&ime, &seed, &format!("{written}{progress}"));
        assert_eq!(
            ime.candidates()
                .iter()
                .any(|c| c.text == sentence && c.kind == CandidateKind::Sentence),
            visible,
            "{seed:?}: {:?}",
            ime.candidates()
        );
        if visible {
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
    for space in HORIZONTAL {
        for tail in [
            format!("{space}sh{space}"),
            format!("{space}shi{space}t{space}"),
        ] {
            let seed = format!("{written}{tail}");
            let mut ime = engine(&seed);
            index(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

#[test]
fn builtin_syllables_cannot_merge_across_horizontal_reading_separators() {
    fresh(
        "builtin_syllables_cannot_merge_across_horizontal_reading_separators",
        false,
        || {
            for (reading, written) in [
                ("完了", "完了"),
                ("kanryou ", "完了"),
                ("日本語を勉強", "日本語を勉強"),
                ("nihongo wo benkyou ", "日本語を勉強"),
            ] {
                separated(reading, written);
            }
        },
    );
}

#[test]
fn nine_pack_syllables_cannot_merge_across_horizontal_reading_separators() {
    fresh(
        "nine_pack_syllables_cannot_merge_across_horizontal_reading_separators",
        true,
        || {
            for reading in ["引用", "in'you "] {
                separated(reading, "引用");
            }
        },
    );
}

#[test]
fn builtin_separator_edits_refresh_candidates_without_losing_exact_drafts() {
    fresh(
        "builtin_separator_edits_refresh_candidates_without_losing_exact_drafts",
        false,
        || {
            editing("kanryou", "完了", "完了", DONE);
            editing("nihongo", "日本語", "日本語を勉強", STUDY);
        },
    );
}

#[test]
fn nine_pack_separator_edits_refresh_candidates_without_losing_exact_drafts() {
    fresh(
        "nine_pack_separator_edits_refresh_candidates_without_losing_exact_drafts",
        true,
        || {
            editing("in'you", "引用", "引用", QUOTE);
        },
    );
}
