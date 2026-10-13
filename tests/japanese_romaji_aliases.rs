//! Fixed Japanese spelling compatibility and optional-word workflow regressions.
//! Private engine snapshots only: no model, native service or physical keyboard.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, XRTabletImeEngine},
    languages::japanese::romaji_to_hiragana,
    lexicon::{self, packs},
};

const CHILD_CASE: &str = "SUZAKU_JAPANESE_ROMAJI_ALIAS_CHILD";
const TRAVEL_ID: &str = "org.suzaku.ja.travel";
const WORD: &str = "アメニティ";
const JAPANESE_IDS: [&str; 9] = [
    "org.suzaku.ja.rail",
    "org.suzaku.ja.family",
    "org.suzaku.ja.internet",
    "org.suzaku.ja.shopping",
    TRAVEL_ID,
    "org.suzaku.ja.work",
    "org.suzaku.ja.study",
    "org.suzaku.ja.cooking",
    "org.suzaku.ja.outdoors",
];
// Authored expectations, not generated from the transliterator or pack readings.
const SPELLINGS: [&str; 9] = [
    "amenithi",
    "AMENITHI",
    "amenitexi",
    "ameniteli",
    "あめにてぃ",
    "amenith",
    "AMENITH",
    "amenith ",
    " AMENITH\t",
];

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("suzaku-ja-romaji-aliases-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn store(&self) -> packs::PackStore {
        packs::PackStore::new(self.0.join("packs")).unwrap()
    }

    fn initialize(&self, ids: &[&str], enabled: bool) {
        let store = self.store();
        assert!(store.list().unwrap().is_empty());
        let catalog = packs::recommended();
        for id in ids {
            let pack = catalog.iter().find(|p| p.manifest.id == *id).unwrap();
            store.install(pack.clone(), false).unwrap();
        }
        if !enabled {
            assert_eq!(ids, [TRAVEL_ID]);
            store.set_enabled(TRAVEL_ID, false).unwrap();
        }
        let report = packs::initialize(&store).unwrap();
        assert!(report.errors.is_empty(), "{report:?}");
        if enabled {
            assert_eq!(report.loaded, ids);
            assert!(report.disabled.is_empty());
        } else {
            assert!(report.loaded.is_empty());
            assert_eq!(report.disabled, [TRAVEL_ID]);
        }
        let builtin = lexicon::builtin("ja").unwrap();
        let active = lexicon::active("ja").unwrap();
        assert_eq!(
            builtin.readings(),
            &active.readings()[..builtin.readings().len()]
        );
        assert_eq!(
            builtin.continuations().collect::<Vec<_>>(),
            active
                .continuations()
                .take(builtin.continuations().count())
                .collect::<Vec<_>>()
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fresh(name: &str, body: impl FnOnce(&Fixture)) {
    if std::env::var(CHILD_CASE).as_deref() == Ok(name) {
        body(&Fixture::new());
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_CASE, name)
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

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    assert!(
        ime.candidates()
            .iter()
            .any(|c| c.text == seed && c.kind == CandidateKind::Literal)
    );
    assert!(
        ime.candidates()
            .iter()
            .all(|c| c.source == CandidateSource::Local)
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

fn index(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
    let position = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| {
            panic!(
                "missing {text:?} {kind:?}, seed={:?}: {:?}",
                ime.snapshot().seed_text,
                ime.candidates()
            )
        });
    if kind == CandidateKind::Word {
        assert!(position < PAGE_SIZE, "{text:?}: {:?}", ime.candidates());
    }
    position
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

fn spelling_workflows() {
    for seed in SPELLINGS {
        let mut ime = engine(seed);
        draft(&ime, seed);
        commit_once(&mut ime, WORD, CandidateKind::Word);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for space in [" ", "\t", "\u{a0}", "\u{2003}", "\u{3000}", "\u{202f}"] {
        for seed in [format!("ameni{space}thi"), format!("amenith{space}")] {
            let mut ime = engine(&seed);
            draft(&ime, &seed);
            commit_once(&mut ime, WORD, CandidateKind::Word);
            commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        }
    }
}

fn unavailable_workflows() {
    for seed in SPELLINGS {
        let mut ime = engine(seed);
        draft(&ime, seed);
        // Generic Katakana transliteration may exist; an optional typed Word may not.
        assert!(
            ime.candidates()
                .iter()
                .all(|c| c.text != WORD || c.kind != CandidateKind::Word),
            "{seed}: {:?}",
            ime.candidates()
        );
        commit_once(&mut ime, seed, CandidateKind::Literal);
    }
}

#[test]
fn thi_alias_preserves_legacy_syllables_and_explicit_small_kana() {
    for (seed, expected) in [
        ("thi", "てぃ"),
        ("THI", "てぃ"),
        ("amenithi", "あめにてぃ"),
        ("AMENITHI", "あめにてぃ"),
        ("ti", "ち"),
        ("di", "ぢ"),
        ("tu", "つ"),
        ("du", "づ"),
        ("texi", "てぃ"),
        ("teli", "てぃ"),
        ("amenitexi", "あめにてぃ"),
        ("ameniteli", "あめにてぃ"),
        ("ameniti", "あめにち"),
        ("chi", "ち"),
        ("shi", "し"),
        ("tsu", "つ"),
        ("chekkuin", "ちぇっくいん"),
        ("tcha", "っちゃ"),
        ("shin'you", "しんよう"),
        ("shin’you", "しんよう"),
        ("nn", "ん"),
        ("nna", "んな"),
        ("nnya", "んにゃ"),
        ("Ωテスト123", "Ωテスト123"),
    ] {
        assert_eq!(romaji_to_hiragana(seed), expected, "{seed}");
    }
}

#[test]
fn thi_alias_does_not_invent_a_word_without_the_optional_pack() {
    fresh(
        "thi_alias_does_not_invent_a_word_without_the_optional_pack",
        |fixture| {
            fixture.initialize(&[], true);
            assert!(std::ptr::eq(
                lexicon::active("ja").unwrap(),
                lexicon::builtin("ja").unwrap()
            ));
            unavailable_workflows();
            assert!(!fixture.store().path().exists());
        },
    );
}

#[test]
fn thi_alias_respects_a_disabled_optional_pack() {
    fresh("thi_alias_respects_a_disabled_optional_pack", |fixture| {
        fixture.initialize(&[TRAVEL_ID], false);
        assert!(std::ptr::eq(
            lexicon::active("ja").unwrap(),
            lexicon::builtin("ja").unwrap()
        ));
        unavailable_workflows();
    });
}

#[test]
fn thi_alias_completes_and_commits_the_optional_word_from_full_and_partial_input() {
    fresh(
        "thi_alias_completes_and_commits_the_optional_word_from_full_and_partial_input",
        |fixture| {
            fixture.initialize(&[TRAVEL_ID], true);
            spelling_workflows();
        },
    );
}

#[test]
fn thi_alias_keeps_first_page_access_with_nine_japanese_packs() {
    fresh(
        "thi_alias_keeps_first_page_access_with_nine_japanese_packs",
        |fixture| {
            fixture.initialize(&JAPANESE_IDS, true);
            spelling_workflows();
            for (seed, first, other) in [("kanji", "漢字", "感じ"), ("sentaku", "選択", "洗濯")]
            {
                let ime = engine(seed);
                assert_eq!(ime.candidates()[0].text, first);
                assert!(
                    index(&ime, first, CandidateKind::Word)
                        < index(&ime, other, CandidateKind::Word)
                );
            }
        },
    );
}

#[test]
fn thi_alias_selected_word_can_continue_as_an_uncommitted_draft() {
    fresh(
        "thi_alias_selected_word_can_continue_as_an_uncommitted_draft",
        |fixture| {
            fixture.initialize(&[TRAVEL_ID], true);
            let mut ime = engine("amenithi");
            ime.select_candidate(index(&ime, WORD, CandidateKind::Word));
            assert_eq!(ime.selected_completion_text(true), Some(WORD));
            draft(&ime, "amenithi");
            // The adapter can adopt a selected word into the editable draft.
            // This word has no authored continuation; do not invent a sentence.
            ime.seed("アメニティ ");
            draft(&ime, "アメニティ ");
            ime.seed("アメニティ wo");
            draft(&ime, "アメニティ wo");
            commit_once(&mut ime, "アメニティ wo", CandidateKind::Literal);
            ime.seed("amenith");
            draft(&ime, "amenith");
            index(&ime, WORD, CandidateKind::Word);
        },
    );
}

#[test]
fn thi_alias_preserves_literal_payloads_and_does_not_skip_unknown_boundaries() {
    fresh(
        "thi_alias_preserves_literal_payloads_and_does_not_skip_unknown_boundaries",
        |fixture| {
            fixture.initialize(&[TRAVEL_ID], true);
            let mut seeds: Vec<String> = [
                "https://example.invalid/amenithi?q=thi",
                "amenithi@example.invalid",
                "let amenithi = \"thi\";",
                "C:/files/amenithi.txt",
                "xyzamenithi",
                "🙂amenithi",
                "amenithixyz",
                "アメニティ{}",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            for boundary in [
                "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("ameni{boundary}thi"));
                seeds.push(format!("amenith{boundary}i"));
                seeds.push(format!("アメニティ{boundary}wo"));
            }
            for tail in ["amenithi", "amenith", "アメニティ wo"] {
                let seed = format!("{}{tail}", "前文。".repeat(90));
                assert!(seed.chars().count() > 256);
                seeds.push(seed);
            }
            for seed in seeds {
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    ime.candidates().iter().all(|c| c.text != WORD),
                    "discarded a prefix or boundary from {seed:?}: {:?}",
                    ime.candidates()
                );
                // Other transliterations are allowed, but raw payloads remain exact.
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}
