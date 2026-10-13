//! Final-syllable progress through fixed authored sentences. These are editable
//! engine seed updates, not physical keyboard, desktop or personal-data tests.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_JAPANESE_ROMAJI_PROGRESS_CHILD";
const STUDY: &str = "日本語を勉強したいです。";
const DONE: &str = "完了したら連絡してください。";
const QUOTE: &str = "引用した部分が分かるように印を付けます。";
const TAILS: [&str; 5] = ["s", "sh", "shi", "shit", "shita"];
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
    let fixture = Fixture(
        std::env::temp_dir().join(format!("suzaku-ja-romaji-progress-{}", std::process::id())),
    );
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

fn stages(ime: &mut XRTabletImeEngine, prefix: &str, expected: &str) {
    for uppercase in [false, true] {
        for space in ["", "\t", "\u{3000}"] {
            for tail in TAILS {
                let seed = format!("{prefix}{tail}{space}");
                let seed = if uppercase {
                    seed.to_ascii_uppercase()
                } else {
                    seed
                };
                ime.seed(&seed);
                draft(ime, &seed);
                // Reachable within the unchanged two-page budget, not an
                // assumption that every old or new choice must be on page one.
                index(ime, expected, CandidateKind::Sentence);
                for forbidden in ["勉強下", "完了下"] {
                    assert!(
                        ime.candidates().iter().all(|c| !c.text.contains(forbidden)),
                        "{seed:?}: {:?}",
                        ime.candidates()
                    );
                }
                // The exact noun 下 must not replace the authored した tail.
                // Longer valid words such as 下書き are still explicit choices.
                assert!(ime.candidates().iter().all(|c| c.text != "引用下"));
                commit_once(&mut engine(&seed), expected, CandidateKind::Sentence);
                commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
            }
        }
    }
}

fn adopted(reading: &str, word: &str) -> XRTabletImeEngine {
    let mut ime = engine(reading);
    ime.select_candidate(index(&ime, word, CandidateKind::Word));
    assert_eq!(ime.selected_completion_text(true), Some(word));
    assert!(ime.snapshot().committed_text.is_empty());
    // The host adopts the selected word into an editable draft, not a commit.
    ime.seed(word);
    draft(&ime, word);
    ime
}

#[test]
fn builtin_authored_sentences_survive_each_final_romaji_keystage() {
    fresh(
        "builtin_authored_sentences_survive_each_final_romaji_keystage",
        false,
        || {
            stages(&mut adopted("nihongo", "日本語"), "日本語を勉強", STUDY);
            stages(&mut adopted("kanryou", "完了"), "完了", DONE);
            for (prefix, sentence) in [("nihongo wo benkyou", STUDY), ("kanryou ", DONE)] {
                stages(&mut engine(prefix), prefix, sentence);
            }
        },
    );
}

#[test]
fn nine_pack_quote_progress_survives_pending_syllables_and_reading_variants() {
    fresh(
        "nine_pack_quote_progress_survives_pending_syllables_and_reading_variants",
        true,
        || {
            stages(&mut adopted("in'you", "引用"), "引用 ", QUOTE);
            for prefix in ["in'you", "in’you "] {
                stages(&mut engine(prefix), prefix, QUOTE);
            }
            let seed = "引用 shit";
            let ime = engine(seed);
            draft(&ime, seed);
            index(&ime, QUOTE, CandidateKind::Sentence);
            assert_eq!(ime.candidates()[0].text, "引用しt");
            assert_eq!(ime.candidates()[0].kind, CandidateKind::Unspecified);
            assert_eq!(ime.candidates()[0].score, 100.0);
            assert!(ime.candidates().iter().all(|c| c.text != "引用下"));
            for word in ["引用下書き", "引用下ごしらえ"] {
                index(&ime, word, CandidateKind::Word);
                commit_once(&mut engine(seed), word, CandidateKind::Word);
            }
            commit_once(&mut engine(seed), seed, CandidateKind::Literal);
        },
    );
}

fn protected(prefix: &str, romaji: &str) {
    let mut seeds = vec![
        format!("xyz{prefix}sh"),
        format!("🙂{prefix}sh"),
        format!("{prefix}shx"),
        format!("{prefix}shitxyz"),
        format!("https://example.invalid/{romaji}sh"),
        format!("let x = \"{romaji}\"; sh"),
        format!("{}{prefix}sh", "前文。".repeat(90)),
        // No invented reading for the next authored kanji (部分, 連絡, etc.).
        format!("{prefix}shitabu"),
    ];
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("{prefix}{boundary}sh"));
        seeds.push(format!("{prefix}sh{boundary}"));
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates()
                .iter()
                .all(|c| { [STUDY, DONE, QUOTE].iter().all(|s| !c.text.ends_with(s)) }),
            "unexpected authored recovery from {seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

#[test]
fn builtin_pending_syllables_do_not_recover_across_protected_boundaries() {
    fresh(
        "builtin_pending_syllables_do_not_recover_across_protected_boundaries",
        false,
        || {
            protected("日本語を勉強", "nihongowobenkyou");
            protected("完了", "kanryou");
        },
    );
}

#[test]
fn nine_pack_pending_syllables_do_not_recover_across_protected_boundaries() {
    fresh(
        "nine_pack_pending_syllables_do_not_recover_across_protected_boundaries",
        true,
        || {
            protected("引用", "in'you");
        },
    );
}
