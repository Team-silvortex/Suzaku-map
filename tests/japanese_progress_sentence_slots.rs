//! Preserve authored sentence slots after an unfinished local Japanese conversion.
//! Synthetic core drafts only: no native keys, personal stores or live models.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    ime::{
        Candidate, CommitOptions, EngineConfig, LanguagePlugin, PredictionStatus, XRTabletImeEngine,
    },
    languages::japanese::JapaneseLanguagePlugin,
    lexicon::packs::{self, PackStore},
};

const AUTHORS: [&str; 2] = [
    "汚れが気になるので拭いておきます。",
    "汚れがきれいに落ちました。",
];
const WORDS: [&str; 4] = [
    "汚れ学校",
    "汚れ画面共有",
    "汚れ画像サイズ",
    "汚れ画面が止まっています",
];
const SEEDS: [&str; 7] = [
    "汚れga",
    "汚れ ga",
    "汚れ\u{3000}ga",
    "汚れ GA",
    "yogore ga",
    "YOGORE GA",
    "よごれ が",
];
const CHILD: &str = "SUZAKU_JAPANESE_PROGRESS_SLOTS_CHILD";
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
fn position(pool: &[Candidate], text: &str, kind: CandidateKind) -> usize {
    pool.iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {pool:?}"))
}
fn raw_and_metadata(pool: &[Candidate], seed: &str) {
    position(pool, seed, CandidateKind::Literal);
    assert!(pool.len() <= 12);
    assert!(pool.iter().all(|c| {
        c.label == c.text
            && c.source == CandidateSource::Local
            && c.score.is_finite()
            && (0.0..=100.0).contains(&c.score)
    }));
    assert_eq!(
        pool.len(),
        pool.iter().map(|c| &c.text).collect::<HashSet<_>>().len()
    );
}
fn primary_unchanged(pool: &[Candidate]) {
    let primary = &pool[0];
    assert_eq!(primary.text, "汚れが");
    assert_eq!(primary.label, "汚れが");
    assert_eq!(primary.kind, CandidateKind::Sentence);
    assert_eq!(primary.source, CandidateSource::Local);
    assert_eq!(primary.score.to_bits(), 100.0_f32.to_bits());
}
fn authored_pair_on_page_one(pool: &[Candidate]) {
    for sentence in AUTHORS {
        assert!(
            position(pool, sentence, CandidateKind::Sentence) < PAGE_SIZE,
            "author outside page one: {sentence:?}: {pool:?}"
        );
    }
}
fn all_words_keep_original_relative_order(pool: &[Candidate], seed: &str) {
    let positions: Vec<_> = WORDS
        .iter()
        .map(|word| position(pool, word, CandidateKind::Word))
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    // Written and phonetic inputs have different pre-existing additions indices.
    // Freeze only the four scores actually established for written 汚れ+ga.
    if seed.starts_with("汚れ") {
        for (&at, score) in positions.iter().zip([91.0_f32, 90.5, 90.0, 89.5]) {
            assert_eq!(pool[at].score.to_bits(), score.to_bits());
        }
    }
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    raw_and_metadata(ime.candidates(), seed);
}
fn check_progress(seed: &str) {
    let ime = engine(seed);
    draft(&ime, seed);
    primary_unchanged(ime.candidates());
    all_words_keep_original_relative_order(ime.candidates(), seed);
    authored_pair_on_page_one(ime.candidates());
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let seed = ime.snapshot().seed_text;
    ime.select_candidate(position(ime.candidates(), text, kind));
    assert_eq!(ime.snapshot().seed_text, seed);
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn unfinished_conversion_does_not_consume_either_existing_author_slot() {
    for seed in SEEDS {
        check_progress(seed);
    }
}

#[test]
fn adopted_progress_words_both_authors_and_raw_commit_once_without_early_commit() {
    let mut adopted = engine("yogore");
    adopted.select_candidate(position(adopted.candidates(), "汚れ", CandidateKind::Word));
    assert_eq!(adopted.selected_completion_text(true), Some("汚れ"));
    assert_eq!(adopted.snapshot().seed_text, "yogore");
    assert!(adopted.snapshot().committed_text.is_empty());
    for gap in ["", " ", "\u{3000}"] {
        let seed = format!("汚れ{gap}ga");
        adopted.seed(&seed);
        draft(&adopted, &seed);
        primary_unchanged(adopted.candidates());
        authored_pair_on_page_one(adopted.candidates());
        for word in WORDS {
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
        }
        for sentence in AUTHORS {
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
        adopted.seed("yogore");
        draft(&adopted, "yogore");
        assert_eq!(adopted.candidates()[0].text, "汚れ");
    }
}

#[test]
fn explicit_zero_through_twelve_limits_keep_primary_raw_and_six_slot_author_capacity() {
    for seed in ["汚れ ga", "yogore ga"] {
        for limit in 0..=12 {
            let pool = mixed(seed, limit);
            assert!(!pool.is_empty());
            assert!(pool.len() <= limit.clamp(1, 12));
            primary_unchanged(&pool);
            if limit >= 2 {
                raw_and_metadata(&pool, seed);
            } else {
                assert_eq!(pool.len(), 1);
            }
            // At six slots: primary + two Words + two authors + exact raw fit.
            // Lower capacities must not invent a larger limit to satisfy this.
            if limit >= PAGE_SIZE {
                authored_pair_on_page_one(&pool);
                assert!(
                    pool.iter()
                        .filter(|c| c.kind == CandidateKind::Word)
                        .count()
                        >= 2
                );
            }
        }
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn nine_private_packs_preserve_progress_pair_and_every_explicit_commit_choice() {
    const NAME: &str = "nine_private_packs_preserve_progress_pair_and_every_explicit_commit_choice";
    const COMPLETED: &str = "completed private nine-pack progress body";
    if std::env::var(CHILD).as_deref() != Ok(NAME) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env(CHILD, NAME)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.contains("running 1 test"), "{stdout}");
        assert!(stdout.contains(COMPLETED), "{stdout}");
        assert!(stdout.contains(&format!("test {NAME} ...")), "{stdout}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{stdout}"
        );
        return;
    }
    let path =
        std::env::temp_dir().join(format!("suzaku-ja-progress-slots-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let recommended = packs::recommended();
    for id in JAPANESE_IDS {
        let pack = recommended
            .iter()
            .find(|pack| pack.manifest.id == id)
            .unwrap();
        store.install(pack.clone(), false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), JAPANESE_IDS.len());
    for id in JAPANESE_IDS {
        assert!(report.loaded.iter().any(|loaded| loaded == id));
    }
    for seed in SEEDS {
        check_progress(seed);
        for word in WORDS {
            commit_once(&mut engine(seed), word, CandidateKind::Word);
        }
        for sentence in AUTHORS {
            commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for limit in 0..=12 {
        let pool = mixed("汚れ ga", limit);
        assert!(!pool.is_empty() && pool.len() <= limit.clamp(1, 12));
        primary_unchanged(&pool);
        if limit >= 2 {
            raw_and_metadata(&pool, "汚れ ga");
        }
        if limit >= PAGE_SIZE {
            authored_pair_on_page_one(&pool);
        }
    }
    println!("{COMPLETED}");
}

#[test]
fn partial_single_authors_unknown_tokens_and_seven_lines_cannot_create_a_pair() {
    for (seed, matching, absent) in [
        ("汚れが気", AUTHORS[0], AUTHORS[1]),
        ("汚れがきれい", AUTHORS[1], AUTHORS[0]),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(position(ime.candidates(), matching, CandidateKind::Sentence) < PAGE_SIZE);
        assert!(ime.candidates().iter().all(|c| c.text != absent));
        commit_once(&mut engine(seed), matching, CandidateKind::Sentence);
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for sentence in AUTHORS {
        let ime = engine(sentence);
        draft(&ime, sentence);
        assert_eq!(ime.candidates()[0].text, sentence);
        assert_eq!(ime.candidates()[0].kind, CandidateKind::Literal);
        commit_once(&mut engine(sentence), sentence, CandidateKind::Literal);
    }
    let mut seeds = [
        "https://example.invalid/汚れga",
        "xyz汚れga",
        "汚れgaxyz",
        "汚れ gh",
        "🙂汚れga",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("汚れ{line}ga"));
    }
    for seed in seeds {
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            ime.candidates()
                .iter()
                .all(|c| !AUTHORS.iter().any(|author| c.text.contains(author))),
            "{seed:?}: {:?}",
            ime.candidates()
        );
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}
