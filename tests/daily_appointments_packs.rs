//! Fresh-process appointment vocabulary alongside all nine optional Japanese packs.
//! Only a private temporary pack store is used; no personal packs or native input.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_APPOINTMENTS_PACKS_CHILD";
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
        "suzaku-ja-appointments-packs-{}",
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

const CASES: &[(&str, &str, &str, [&str; 2])] = &[
    (
        "yoyakuhenkou",
        "yoyakuhenko",
        "予約変更",
        [
            "予約変更をお願いできますか。",
            "予約変更ができるか確認します。",
        ],
    ),
    (
        "raitenjikan",
        "raitenjika",
        "来店時間",
        [
            "来店時間を少し遅らせてもいいですか。",
            "来店時間が決まったら連絡します。",
        ],
    ),
    (
        "machijikan",
        "machijika",
        "待ち時間",
        [
            "待ち時間はどのくらいですか。",
            "待ち時間に少し外へ出てもいいですか。",
        ],
    ),
    (
        "eigyoujikan",
        "eigyoujika",
        "営業時間",
        [
            "営業時間を教えてください。",
            "営業時間を確認してから行きます。",
        ],
    ),
    (
        "seiriken",
        "seirike",
        "整理券",
        [
            "整理券はどこでもらえますか。",
            "整理券を受け取ってから順番を待ちます。",
        ],
    ),
    (
        "teikyuubi",
        "teikyuub",
        "定休日",
        [
            "定休日は何曜日ですか。",
            "定休日だったので、別の日に行きます。",
        ],
    ),
];

#[test]
fn appointments_with_nine_packs_keep_full_and_specific_partial_choices_reachable() {
    fresh(
        "appointments_with_nine_packs_keep_full_and_specific_partial_choices_reachable",
        || {
            for &(reading, partial, word, sentences) in CASES {
                for seed in [reading, partial] {
                    let ime = engine(seed);
                    draft(&ime, seed);
                    // Family 整理 and travel 予約番号 may retain competing rows.
                    // Reachability within the bounded pool is not a page-one promise.
                    index(&ime, word, CandidateKind::Word);
                    commit_once(&mut engine(seed), word, CandidateKind::Word);
                    for sentence in sentences {
                        index(&ime, sentence, CandidateKind::Sentence);
                        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
                    }
                    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
                }
            }
            // This supplementary word has no new authored context.
            for seed in ["yoyakukakunin", "yoyakukakuni"] {
                let ime = engine(seed);
                draft(&ime, seed);
                index(&ime, "予約確認", CandidateKind::Word);
                commit_once(&mut engine(seed), "予約確認", CandidateKind::Word);
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
        },
    );
}

#[test]
fn appointments_with_nine_packs_keep_old_roots_travel_authors_and_raw_boundaries() {
    fresh(
        "appointments_with_nine_packs_keep_old_roots_travel_authors_and_raw_boundaries",
        || {
            for seed in ["seiri", "せいり"] {
                let ime = engine(seed);
                draft(&ime, seed);
                assert_eq!(ime.candidates()[0].text, "整理");
                first_page(&ime, "整理", CandidateKind::Word);
                commit_once(&mut engine(seed), "整理", CandidateKind::Word);
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
            for (seed, word, sentences) in [
                (
                    "yoyakubangou",
                    "予約番号",
                    [
                        "予約番号を確認します。",
                        "予約番号はどこに書いてありますか。",
                    ],
                ),
                (
                    "hoteru",
                    "ホテル",
                    [
                        "ホテルまでの行き方を教えてください。",
                        "ホテルに着いたら連絡します。",
                    ],
                ),
            ] {
                let ime = engine(seed);
                draft(&ime, seed);
                assert_eq!(ime.candidates()[0].text, word);
                first_page(&ime, word, CandidateKind::Word);
                commit_once(&mut engine(seed), word, CandidateKind::Word);
                for sentence in sentences {
                    first_page(&ime, sentence, CandidateKind::Sentence);
                    commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
                }
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
            let mut seeds = ["https://seiriken.example.invalid", "xyzyoyakukakunin"]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            for boundary in [
                "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("seiri{boundary}ken"));
                seeds.push(format!("整理券{boundary}ha"));
            }
            for seed in seeds {
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                assert!(
                    ime.candidates().iter().all(|candidate| {
                        !CASES
                            .iter()
                            .flat_map(|(_, _, _, sentences)| sentences)
                            .any(|sentence| candidate.text.contains(*sentence))
                    }),
                    "unexpected appointment author for {seed:?}: {:?}",
                    ime.candidates()
                );
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}
