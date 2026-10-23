//! Freeze a unique whole-reading terminal-n completion without changing its greedy conversion.
//! Keep existing long-vowel/root/progress and terminal-n particle policies independent.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE, offline},
    ime::{Candidate, CommitOptions, EngineConfig, LanguagePlugin, XRTabletImeEngine},
    languages::japanese::JapaneseLanguagePlugin,
    lexicon::packs::{self, PackStore},
};

const AUTHORS: [&str; 2] = [
    "換気扇を回して、こもった空気を外に出します。",
    "換気扇を止めてから部屋を出ます。",
];
const WORDS: [(&str, f32); 4] = [
    ("換気扇", 90.5),
    ("換気世界", 90.0),
    ("換気選択", 89.5),
    ("換気説明", 89.0),
];
const CHILD: &str = "SUZAKU_JAPANESE_NASAL_SLOTS_CHILD";

fn seeds() -> Vec<String> {
    let mut seeds = ["kankise", "KANKISE", "かんきせ"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for gap in [" ", "\t", "\u{a0}", "\u{2007}", "\u{202f}", "\u{3000}"] {
        seeds.push(format!("KAN{gap}KISE"));
    }
    seeds
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
fn position(pool: &[Candidate], text: &str, kind: CandidateKind) -> usize {
    pool.iter()
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {pool:?}"))
}
fn metadata(pool: &[Candidate]) {
    assert!(pool.iter().all(|candidate| {
        candidate.label == candidate.text
            && candidate.source == CandidateSource::Local
            && candidate.score.is_finite()
            && (0.0..=100.0).contains(&candidate.score)
    }));
    assert_eq!(
        pool.len(),
        pool.iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
}
fn primary(pool: &[Candidate]) {
    assert_eq!(pool[0].text, "換気せ");
    assert_eq!(pool[0].label, "換気せ");
    assert_eq!(pool[0].kind, CandidateKind::Sentence);
    assert_eq!(pool[0].source, CandidateSource::Local);
    assert_eq!(pool[0].score.to_bits(), 100.0_f32.to_bits());
}
fn authors_on_first_page(pool: &[Candidate]) {
    for (text, score) in AUTHORS.into_iter().zip([76.5_f32, 76.0]) {
        let at = position(pool, text, CandidateKind::Sentence);
        assert!(at < PAGE_SIZE, "author outside first page: {pool:?}");
        assert_eq!(pool[at].score.to_bits(), score.to_bits());
    }
}
fn four_words_are_unchanged(pool: &[Candidate]) {
    let mut previous = 0;
    for (text, score) in WORDS {
        let at = position(pool, text, CandidateKind::Word);
        assert!(at > previous, "Word order changed: {pool:?}");
        assert_eq!(pool[at].score.to_bits(), score.to_bits());
        previous = at;
    }
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
    metadata(ime.candidates());
    position(ime.candidates(), seed, CandidateKind::Literal);
}
fn commit_once(ime: &mut XRTabletImeEngine, text: &str, kind: CandidateKind) {
    let original = ime.snapshot().seed_text;
    ime.select_candidate(position(ime.candidates(), text, kind));
    assert_eq!(ime.snapshot().seed_text, original);
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
fn whole_reading_nasal_partial_keeps_both_authors_within_real_zero_to_twelve_limits() {
    for seed in seeds() {
        for limit in 0..=12 {
            let pool = mixed(&seed, limit);
            assert!(!pool.is_empty() && pool.len() <= limit.clamp(1, 12));
            metadata(&pool);
            primary(&pool);
            if limit >= 2 {
                position(&pool, &seed, CandidateKind::Literal);
            } else {
                assert_eq!(pool.len(), 1);
            }
            if limit >= PAGE_SIZE {
                authors_on_first_page(&pool);
                assert!(
                    pool.iter()
                        .filter(|candidate| candidate.kind == CandidateKind::Word)
                        .count()
                        >= 2
                );
            }
            if limit == 12 {
                four_words_are_unchanged(&pool);
            }
        }
    }
    // The actual pre-fix Roman pool has twelve rows. Pure Hiragana input instead
    // deduplicates its Kana row with Literal, so no fabricated twelve-row promise.
    let pool = mixed("kankise", 12);
    assert_eq!(pool.len(), 12);
    for (text, kind, score) in [
        ("換気せ", CandidateKind::Sentence, 100.0_f32),
        ("かんきせ", CandidateKind::Unspecified, 92.0),
        ("換気扇", CandidateKind::Word, 90.5),
        ("換気世界", CandidateKind::Word, 90.0),
        ("カンキセ", CandidateKind::Unspecified, 90.0),
        (AUTHORS[0], CandidateKind::Sentence, 76.5),
        ("換気選択", CandidateKind::Word, 89.5),
        ("換気説明", CandidateKind::Word, 89.0),
        (AUTHORS[1], CandidateKind::Sentence, 76.0),
        ("kankise", CandidateKind::Literal, 55.0),
        ("換気先週", CandidateKind::Word, 54.5),
        ("換気接続", CandidateKind::Word, 54.0),
    ] {
        let at = position(&pool, text, kind);
        assert_eq!(pool[at].label, text);
        assert_eq!(pool[at].source, CandidateSource::Local);
        assert_eq!(pool[at].score.to_bits(), score.to_bits());
    }
    assert_eq!(
        pool.iter()
            .filter(|candidate| !AUTHORS.contains(&candidate.text.as_str()))
            .map(|candidate| candidate.text.as_str())
            .collect::<Vec<_>>(),
        [
            "換気せ",
            "かんきせ",
            "換気扇",
            "換気世界",
            "カンキセ",
            "換気選択",
            "換気説明",
            "kankise",
            "換気先週",
            "換気接続",
        ]
    );
}

#[test]
fn nasal_partial_adoption_continuation_exact_literal_and_undo_never_commit_early() {
    for seed in seeds() {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        primary(ime.candidates());
        authors_on_first_page(ime.candidates());
        ime.select_candidate(position(ime.candidates(), "換気扇", CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some("換気扇"));
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        for gap in ["", " ", "\u{3000}"] {
            let continued = format!("換気扇{gap}wo");
            ime.seed(&continued);
            draft(&ime, &continued);
            assert!(position(ime.candidates(), AUTHORS[0], CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(&mut engine(&continued), AUTHORS[0], CandidateKind::Sentence);
            commit_once(&mut engine(&continued), &continued, CandidateKind::Literal);
            ime.seed(&seed);
            draft(&ime, &seed);
            primary(ime.candidates());
            authors_on_first_page(ime.candidates());
        }
        for (word, _) in WORDS {
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
        }
        for sentence in AUTHORS {
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
        // This explicitly selects the editable spelling, including any gap,
        // rather than committing the displayed conversion or selected Word.
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn private_nine_japanese_packs_keep_the_whole_reading_nasal_pair_on_page_one() {
    const NAME: &str = "private_nine_japanese_packs_keep_the_whole_reading_nasal_pair_on_page_one";
    const DONE: &str = "completed private Japanese nasal-slot body with nine packs";
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
        assert!(stdout.contains(DONE), "{stdout}");
        assert!(stdout.contains(&format!("test {NAME} ...")), "{stdout}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{stdout}"
        );
        return;
    }
    let path = std::env::temp_dir().join(format!("suzaku-ja-nasal-slots-{}", std::process::id()));
    // No cleanup ownership exists until this exclusive creation succeeds.
    fs::create_dir(&path).unwrap();
    let fixture = Fixture(path);
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    let selected: Vec<_> = packs::recommended()
        .into_iter()
        .filter(|pack| pack.manifest.language == "ja")
        .collect();
    assert_eq!(selected.len(), 9);
    let ids: HashSet<_> = selected
        .iter()
        .map(|pack| pack.manifest.id.clone())
        .collect();
    assert_eq!(ids.len(), 9);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), 9);
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    for seed in seeds() {
        let ime = engine(&seed);
        draft(&ime, &seed);
        primary(ime.candidates());
        authors_on_first_page(ime.candidates());
        four_words_are_unchanged(ime.candidates());
        for limit in [PAGE_SIZE, 12] {
            let pool = mixed(&seed, limit);
            assert!(pool.len() <= limit);
            primary(&pool);
            metadata(&pool);
            position(&pool, &seed, CandidateKind::Literal);
            authors_on_first_page(&pool);
        }
        for (word, _) in WORDS {
            commit_once(&mut engine(&seed), word, CandidateKind::Word);
        }
        for sentence in AUTHORS {
            commit_once(&mut engine(&seed), sentence, CandidateKind::Sentence);
        }
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
    println!("{DONE}");
}
