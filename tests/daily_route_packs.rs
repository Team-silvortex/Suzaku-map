//! Private all-nine-pack route dialogue snapshots; no personal stores, GPS or native input.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_ROUTE_PACKS_CHILD";
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
    let path = std::env::temp_dir().join(format!("suzaku-ja-route-packs-{}", std::process::id()));
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
        "machiawasebasho",
        "machiawasebash",
        "待ち合わせ場所",
        [
            "待ち合わせ場所を駅前に変えてもいいですか。",
            "待ち合わせ場所は、この建物の入口です。",
        ],
    ),
    (
        "imamukatteimasu",
        "imamukatteima",
        "今向かっています",
        [
            "今向かっています。少し遅れそうです。",
            "今向かっていますので、着いたら連絡します。",
        ],
    ),
    (
        "mousugutsukimasu",
        "mousugutsukimas",
        "もうすぐ着きます",
        [
            "もうすぐ着きます。どの入口にいますか。",
            "もうすぐ着きますので、そのまま待っていてください。",
        ],
    ),
    (
        "atogofun",
        "atogofu",
        "あと五分",
        [
            "あと五分くらいで着きます。",
            "あと五分だけ待ってもらえますか。",
        ],
    ),
    (
        "betsuru-to",
        "betsuru-t",
        "別ルート",
        [
            "別ルートで向かっています。",
            "別ルートに変えたので、少し遅れそうです。",
        ],
    ),
    (
        "genzaichi",
        "genzaich",
        "現在地",
        [
            "現在地を送ってもらえますか。",
            "現在地を確認してから向かいます。",
        ],
    ),
];
const SUPPLEMENTS: &[(&str, &str, &str)] = &[
    ("ekimae", "えきまえ", "駅前"),
    ("shoumengenkan", "しょうめんげんかん", "正面玄関"),
    ("kitaguchi", "きたぐち", "北口"),
    ("minamiguchi", "みなみぐち", "南口"),
    ("hodoukyou", "ほどうきょう", "歩道橋"),
    ("toomawari", "とおまわり", "遠回り"),
];

#[test]
fn route_with_nine_packs_keeps_new_words_and_authored_pairs_reachable() {
    fresh(
        "route_with_nine_packs_keeps_new_words_and_authored_pairs_reachable",
        || {
            for &(reading, partial, word, sentences) in CASES {
                for seed in [reading, partial] {
                    let ime = engine(seed);
                    draft(&ime, seed);
                    // The family 居間 reading may retain its greedy branch for
                    // a partial 今... draft. Keep selectable rows, not a blanket page-one promise.
                    index(&ime, word, CandidateKind::Word);
                    commit_once(&mut engine(seed), word, CandidateKind::Word);
                    for sentence in sentences {
                        index(&ime, sentence, CandidateKind::Sentence);
                        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
                    }
                    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
                }
            }
            // Supplements have no new authored context. 駅前 coexists with rail 駅.
            for &(reading, kana, word) in SUPPLEMENTS {
                for seed in [reading, kana] {
                    let ime = engine(seed);
                    draft(&ime, seed);
                    index(&ime, word, CandidateKind::Word);
                    commit_once(&mut engine(seed), word, CandidateKind::Word);
                    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
                }
            }
        },
    );
}

#[test]
fn route_with_nine_packs_preserves_old_meeting_rail_room_and_foreign_literals() {
    fresh(
        "route_with_nine_packs_preserves_old_meeting_rail_room_and_foreign_literals",
        || {
            for (seed, word, sentences) in [
                (
                    "shuugoujikan",
                    "集合時間",
                    [
                        "集合時間をもう一度教えてください。",
                        "集合時間に間に合いそうです。",
                    ],
                ),
                (
                    "ima",
                    "居間",
                    [
                        "居間で一緒に話しましょう。",
                        "居間の窓を開けてもいいですか。",
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
            // The existing rail pack has one 改札 sentence, not a fabricated pair.
            let ime = engine("kaisatsu");
            draft(&ime, "kaisatsu");
            assert_eq!(ime.candidates()[0].text, "改札");
            first_page(&ime, "改札", CandidateKind::Word);
            first_page(&ime, "改札はどこですか。", CandidateKind::Sentence);
            commit_once(&mut engine("kaisatsu"), "改札", CandidateKind::Word);
            commit_once(
                &mut engine("kaisatsu"),
                "改札はどこですか。",
                CandidateKind::Sentence,
            );
            commit_once(&mut engine("kaisatsu"), "kaisatsu", CandidateKind::Literal);

            let mut seeds = [
                "Ω表示",
                "Привет🙂",
                "İstanbul",
                "résumé🙂",
                "https://genzaichi.example.invalid",
                "xyzimamukatteimasu",
                "あと5分",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
            for boundary in [
                "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("machiawase{boundary}basho"));
                seeds.push(format!("現在地{boundary}wo"));
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
                    "unexpected route author for {seed:?}: {:?}",
                    ime.candidates()
                );
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}
