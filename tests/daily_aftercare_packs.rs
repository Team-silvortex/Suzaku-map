//! Fresh-process aftercare vocabulary alongside all nine optional Japanese packs.
//! Only a private temporary pack store is used; no personal packs or native input.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_AFTERCARE_PACKS_CHILD";
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
    let path =
        std::env::temp_dir().join(format!("suzaku-ja-aftercare-packs-{}", std::process::id()));
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
        "hoshousho",
        "hoshoush",
        "保証書",
        [
            "保証書を持ってきました。",
            "保証書はこの封筒に入っています。",
        ],
    ),
    (
        "kounyuubi",
        "kounyuub",
        "購入日",
        [
            "購入日を確認してから問い合わせます。",
            "購入日が分かるものを持っていけばいいですか。",
        ],
    ),
    (
        "fuzokuhin",
        "fuzokuhi",
        "付属品",
        [
            "付属品は全部そろっています。",
            "付属品も一緒に持っていけばいいですか。",
        ],
    ),
    (
        "hensouhouhou",
        "hensouhouho",
        "返送方法",
        [
            "返送方法を教えてもらえますか。",
            "返送方法を確認してから荷物を送ります。",
        ],
    ),
    (
        "konpouzai",
        "konpouza",
        "梱包材",
        [
            "梱包材はそのまま残してあります。",
            "梱包材を使って包み直してもいいですか。",
        ],
    ),
];

#[test]
fn aftercare_with_nine_packs_keeps_new_choices_reachable() {
    fresh(
        "aftercare_with_nine_packs_keeps_new_choices_reachable",
        || {
            for &(reading, partial, word, sentences) in CASES {
                for seed in [reading, partial] {
                    let ime = engine(seed);
                    draft(&ime, seed);
                    // Finite pool reachability, not a promise that every choice is on page one.
                    index(&ime, word, CandidateKind::Word);
                    commit_once(&mut engine(seed), word, CandidateKind::Word);
                    for sentence in sentences {
                        index(&ime, sentence, CandidateKind::Sentence);
                        commit_once(&mut engine(seed), sentence, CandidateKind::Sentence);
                    }
                    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
                }
            }
            // Baseline henkin had only kana. A missing common Word is intentionally filled.
            for seed in ["henkin", "henkinn", "へんきん"] {
                let ime = engine(seed);
                draft(&ime, seed);
                assert_eq!(ime.candidates()[0].text, "返金");
                first_page(&ime, "返金", CandidateKind::Word);
                commit_once(&mut engine(seed), "返金", CandidateKind::Word);
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
        },
    );
}

#[test]
fn aftercare_with_nine_packs_keeps_old_shopping_authors_and_boundaries() {
    fresh(
        "aftercare_with_nine_packs_keeps_old_shopping_authors_and_boundaries",
        || {
            for (seed, word, sentences) in [
                (
                    "reshi-to",
                    "レシート",
                    [
                        "レシートを袋に入れてください。",
                        "レシートは交換するときに必要ですか。",
                    ],
                ),
                (
                    "henpin",
                    "返品",
                    ["返品できますか。", "返品の期限を教えてください。"],
                ),
                (
                    "ryoushuusho",
                    "領収書",
                    ["領収書をお願いします。", "領収書に名前を入れてください。"],
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
            for (seed, word) in [
                ("koukan", "交換"),
                ("housou", "包装"),
                ("henpinkigen", "返品期限"),
                ("shouhinbangou", "商品番号"),
            ] {
                let ime = engine(seed);
                draft(&ime, seed);
                assert_eq!(ime.candidates()[0].text, word);
                commit_once(&mut engine(seed), word, CandidateKind::Word);
                commit_once(&mut engine(seed), seed, CandidateKind::Literal);
            }
            let mut seeds = ["https://hoshousho.example.invalid", "xyzhoshousho"]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            for boundary in [
                "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            ] {
                seeds.push(format!("hoshou{boundary}sho"));
                seeds.push(format!("保証書{boundary}ha"));
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
                    "unexpected aftercare author for {seed:?}: {:?}",
                    ime.candidates()
                );
                commit_once(&mut ime, &seed, CandidateKind::Literal);
            }
        },
    );
}
