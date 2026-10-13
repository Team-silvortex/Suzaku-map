//! Complete cleaning-related readings with nine optional Japanese collections.
//! Private engine snapshots only: no personal packs, native input or live model.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_JAPANESE_CLEANING_COEXISTENCE_CHILD";
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
        "suzaku-ja-cleaning-coexistence-{}",
        std::process::id()
    ));
    // Claim cleanup ownership only after exclusive creation succeeds.
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
    assert_eq!(report.loaded.len(), JAPANESE_IDS.len());
    for id in JAPANESE_IDS {
        assert!(report.loaded.iter().any(|loaded| loaded == id));
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

fn first_page(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert!(
        index(ime, text, kind) < PAGE_SIZE,
        "{text:?} is not on page one: {:?}",
        ime.candidates()
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
fn nine_pack_cleaning_neighbors_keep_complete_optional_words_selectable() {
    fresh(
        "nine_pack_cleaning_neighbors_keep_complete_optional_words_selectable",
        || {
            // Existing complete words, not promises for arbitrarily short prefixes.
            for (romaji, kana, word) in [
                ("souji", "そうじ", "掃除"),
                ("gomibako", "ごみばこ", "ごみ箱"),
                ("burauza-", "ぶらうざー", "ブラウザー"),
                ("fukin", "ふきん", "布巾"),
            ] {
                for seed in [
                    romaji.to_owned(),
                    romaji.to_ascii_uppercase(),
                    kana.to_owned(),
                ] {
                    let ime = engine(&seed);
                    draft(&ime, &seed);
                    first_page(&ime, word, CandidateKind::Word);
                    commit_once(&mut engine(&seed), word, CandidateKind::Word);
                    commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        },
    );
}

#[test]
fn nine_pack_cleaning_builtin_words_preserve_longer_words_and_natural_kana() {
    fresh(
        "nine_pack_cleaning_builtin_words_preserve_longer_words_and_natural_kana",
        || {
            for (romaji, kana, word) in [
                ("soujiki", "そうじき", "掃除機"),
                ("kanki", "かんき", "換気"),
                ("hokori", "ほこり", "ほこり"),
                ("houki", "ほうき", "ほうき"),
                ("kankisen", "かんきせん", "換気扇"),
            ] {
                for seed in [
                    romaji.to_owned(),
                    romaji.to_ascii_uppercase(),
                    kana.to_owned(),
                ] {
                    let ime = engine(&seed);
                    draft(&ime, &seed);
                    // Same-text natural Kana remains the one untouched Literal;
                    // Roman letters still offer that natural spelling as a Word.
                    let kind = if seed == word {
                        CandidateKind::Literal
                    } else {
                        CandidateKind::Word
                    };
                    first_page(&ime, word, kind);
                    if word == "換気扇" {
                        assert_eq!(ime.candidates()[0].text, word);
                    }
                    commit_once(&mut engine(&seed), word, kind);
                    commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        },
    );
}

#[test]
fn nine_pack_cleaning_keeps_existing_souji_sentences_during_pending_syllables() {
    fresh(
        "nine_pack_cleaning_keeps_existing_souji_sentences_during_pending_syllables",
        || {
            // These two first-page choices already belong to the family workflow.
            for seed in ["souji", "SOUJI", "そうじ"] {
                let ime = engine(seed);
                draft(&ime, seed);
                first_page(&ime, "掃除", CandidateKind::Word);
                for sentence in ["掃除を一緒にしませんか。", "掃除が終わったら休みましょう。"]
                {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
                }
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
            // A longer newly appended word must not take away the two existing
            // first-page sentences of the shorter pending-syllable completion.
            // Keep the observed primary words and unconverted draft unchanged;
            // the new word's two sentences must still fit in the twelve choices.
            for separator in ["", " ", "\t", "\u{2007}", "\u{202f}", "\u{3000}"] {
                for reading in ["souj", "SOUJ", "そうj"] {
                    let seed = format!("{separator}{reading}{separator}");
                    let ime = engine(&seed);
                    draft(&ime, &seed);
                    // Existing same-text dedup keeps the bare Kana/ASCII draft
                    // as Literal; surrounding separators remain in the raw row.
                    let kana_kind = if seed == "そうj" {
                        CandidateKind::Literal
                    } else {
                        CandidateKind::Unspecified
                    };
                    let primary = [
                        ("そうj", kana_kind, 100.0),
                        ("ソウj", CandidateKind::Unspecified, 92.0),
                        ("掃除機", CandidateKind::Word, 91.0),
                        ("掃除", CandidateKind::Word, 90.5),
                    ];
                    assert!(ime.candidates().len() >= primary.len());
                    for (candidate, (text, kind, score)) in ime.candidates().iter().zip(primary) {
                        assert_eq!(
                            (candidate.text.as_str(), candidate.kind, candidate.score),
                            (text, kind, score),
                            "seed={seed:?}: {:?}",
                            ime.candidates()
                        );
                    }
                    for sentence in ["掃除を一緒にしませんか。", "掃除が終わったら休みましょう。"]
                    {
                        first_page(&ime, sentence, CandidateKind::Sentence);
                        commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                    }
                    for sentence in [
                        "掃除機をかけてから出かけます。",
                        "掃除機のごみを捨てました。",
                    ] {
                        index(&ime, sentence, CandidateKind::Sentence);
                        commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
                    }
                    commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
                }
            }
        },
    );
}
