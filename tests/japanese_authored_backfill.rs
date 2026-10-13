//! Authored-tail alternatives only fill spare rows; existing choices stay frozen.
//! These isolated core tests do not exercise native keys or a live model.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, offline},
    ime::{
        Candidate, CommitOptions, EngineConfig, LanguagePlugin, PredictionStatus, XRTabletImeEngine,
    },
    languages::japanese::JapaneseLanguagePlugin,
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_JAPANESE_AUTHORED_BACKFILL_CHILD";
const QUOTE: &str = "引用した部分が分かるように印を付けます。";
const DONE: &str = "完了したら連絡してください。";
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

// Fixed from the pre-fix failing list: backfill must not change these eight rows.
const QUOTE_PRIMARY: [(&str, CandidateKind, f32); 8] = [
    ("引用しt", CandidateKind::Unspecified, 100.0),
    ("引用シt", CandidateKind::Unspecified, 92.0),
    ("引用失礼します", CandidateKind::Word, 91.0),
    ("引用質問", CandidateKind::Word, 90.5),
    ("引用失礼しました", CandidateKind::Word, 90.0),
    (QUOTE, CandidateKind::Sentence, 77.0),
    ("引用試着", CandidateKind::Word, 89.5),
    ("引用 shit", CandidateKind::Literal, 55.0),
];

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fresh(name: &str, body: impl FnOnce()) {
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
    let path = std::env::temp_dir().join(format!(
        "suzaku-ja-authored-backfill-{}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let catalog = packs::recommended();
    for id in JAPANESE_IDS {
        let pack = catalog.iter().find(|pack| pack.manifest.id == id).unwrap();
        store.install(pack.clone(), false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), 9);
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

fn mixed(seed: &str, limit: usize) -> Vec<Candidate> {
    offline(
        "ja",
        seed,
        "",
        JapaneseLanguagePlugin.direct_candidates(seed, 1.0).unwrap(),
        limit,
    )
}

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    ime.candidates()
        .iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
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
    assert!(ime.candidates().iter().all(|candidate| {
        candidate.source == CandidateSource::Local && (0.0..=100.0).contains(&candidate.score)
    }));
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}

fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let original_seed = ime.snapshot().seed_text;
    ime.select_candidate(index(ime, text, kind));
    assert_eq!(ime.snapshot().seed_text, original_seed);
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

#[test]
fn authored_probe_words_fill_spare_rows_without_touching_the_eight_existing_choices() {
    fresh(
        "authored_probe_words_fill_spare_rows_without_touching_the_eight_existing_choices",
        || {
            let seed = "引用 shit";
            let primary = mixed(seed, 8);
            assert_eq!(primary.len(), 8);
            for (candidate, (text, kind, score)) in primary.iter().zip(QUOTE_PRIMARY) {
                assert_eq!(
                    (candidate.text.as_str(), candidate.kind, candidate.score),
                    (text, kind, score)
                );
                assert_eq!(candidate.source, CandidateSource::Local);
            }
            let expanded = mixed(seed, 12);
            assert_eq!(expanded.len(), 12);
            assert_eq!(expanded[..8], primary);
            assert!(expanded[8..].iter().all(|candidate| {
                candidate.kind == CandidateKind::Word
                    && candidate.source == CandidateSource::Local
                    && candidate.score < 55.0
            }));
            let ime = engine(seed);
            draft(&ime, seed);
            assert_eq!(ime.candidates(), expanded);
            for word in ["引用下書き", "引用下ごしらえ"] {
                assert!((8..12).contains(&index(&ime, word, CandidateKind::Word)));
                commit_once(&mut engine(seed), word, CandidateKind::Word);
            }
        },
    );
}

#[test]
fn authored_backfill_uses_whole_context_across_case_and_horizontal_separators() {
    fresh(
        "authored_backfill_uses_whole_context_across_case_and_horizontal_separators",
        || {
            // Both contexts already own authored した continuations. Ranking must
            // not special-case 引用 or a particular optional pack.
            for (prefix, sentence) in [("引用", QUOTE), ("完了", DONE)] {
                for suffix in [" shit", " SHIT", "\tshit", "\u{3000}SHIT", "shit", "shit\t"] {
                    let seed = format!("{prefix}{suffix}");
                    let ime = engine(&seed);
                    draft(&ime, &seed);
                    index(&ime, sentence, CandidateKind::Sentence);
                    assert!(
                        ime.candidates()
                            .iter()
                            .all(|candidate| candidate.text != format!("{prefix}下"))
                    );
                    for tail in ["下書き", "下ごしらえ"] {
                        let word = format!("{prefix}{tail}");
                        index(&ime, &word, CandidateKind::Word);
                        commit_once(&mut engine(&seed), &word, CandidateKind::Word);
                    }
                    commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        },
    );
}

#[test]
fn authored_backfill_obeys_every_requested_capacity_without_hiding_raw_text() {
    fresh(
        "authored_backfill_obeys_every_requested_capacity_without_hiding_raw_text",
        || {
            for seed in ["引用 shit", "引用 SHIT\t", "完了 shit"] {
                for limit in 1..=12 {
                    let candidates = mixed(seed, limit);
                    assert!(!candidates.is_empty());
                    assert!(
                        candidates.len() <= limit,
                        "{seed:?}, limit={limit}: {candidates:?}"
                    );
                    assert_eq!(
                        candidates
                            .iter()
                            .map(|candidate| &candidate.text)
                            .collect::<HashSet<_>>()
                            .len(),
                        candidates.len()
                    );
                    if limit > 1 {
                        assert!(
                            candidates.iter().any(|candidate| candidate.text == seed
                                && candidate.kind == CandidateKind::Literal),
                            "{seed:?}, limit={limit}: {candidates:?}"
                        );
                    }
                }
            }
        },
    );
}

#[test]
fn unrelated_new_full_readings_still_offer_their_dictionary_word() {
    fresh(
        "unrelated_new_full_readings_still_offer_their_dictionary_word",
        || {
            for seed in ["shitsudo", "SHITSUDO", "しつど"] {
                let ime = engine(seed);
                draft(&ime, seed);
                index(&ime, "湿度", CandidateKind::Word);
                commit_once(&mut engine(seed), "湿度", CandidateKind::Word);
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
        },
    );
}

#[test]
fn unknown_or_line_separated_prefixes_cannot_borrow_authored_backfill() {
    fresh(
        "unknown_or_line_separated_prefixes_cannot_borrow_authored_backfill",
        || {
            let mut seeds = vec![
                "xyz引用 shit".to_owned(),
                "https://example.invalid/引用shit".to_owned(),
                "🙂引用shit".to_owned(),
                "引用shxt".to_owned(),
            ];
            for separator in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("引用{separator}shit"));
            }
            for seed in seeds {
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    ime.candidates().iter().all(|candidate| {
                        ![
                            QUOTE,
                            DONE,
                            "引用下書き",
                            "引用下ごしらえ",
                            "完了下書き",
                            "完了下ごしらえ",
                        ]
                        .iter()
                        .any(|forbidden| candidate.text.ends_with(forbidden))
                    }),
                    "{seed:?}: {:?}",
                    ime.candidates()
                );
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}
