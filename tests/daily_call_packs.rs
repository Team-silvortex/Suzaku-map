//! Private all-nine-pack call-recovery snapshots; no personal stores, audio control or native input.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_CALL_PACKS_CHILD";
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("running 1 test"), "{stdout}");
        assert!(stdout.contains(&format!("test {name} ... ok")), "{stdout}");
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
            "{stdout}"
        );
        return;
    }
    let path = std::env::temp_dir().join(format!("suzaku-ja-call-packs-{}", std::process::id()));
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

const WORDS: &[(&str, &str, &str, &str)] = &[
    ("kikoenikui", "きこえにくい", "キコエニクイ", "聞こえにくい"),
    (
        "myu-tokaijo",
        "みゅーとかいじょ",
        "ミュートカイジョ",
        "ミュート解除",
    ),
    (
        "gamengatomatteimasu",
        "がめんがとまっています",
        "ガメンガトマッテイマス",
        "画面が止まっています",
    ),
    (
        "otogazureteimasu",
        "おとがずれています",
        "オトガズレテイマス",
        "音がずれています",
    ),
    (
        "hairinaoshimasu",
        "はいりなおします",
        "ハイリナオシマス",
        "入り直します",
    ),
    (
        "tsunaginaoshimasu",
        "つなぎなおします",
        "ツナギナオシマス",
        "つなぎ直します",
    ),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "聞こえにくい",
        [
            "聞こえにくいので、もう少し大きな声で話してもらえますか。",
            "聞こえにくいので、チャットに書いてもらえますか。",
        ],
    ),
    (
        "ミュート解除",
        [
            "ミュート解除をお願いできますか。",
            "ミュート解除のボタンを押してから話します。",
        ],
    ),
    (
        "画面が止まっています",
        [
            "画面が止まっていますが、声は聞こえます。",
            "画面が止まっていますので、一度入り直します。",
        ],
    ),
    (
        "音がずれています",
        [
            "音がずれています。映像より少し遅れて聞こえます。",
            "音がずれていますので、一度つなぎ直します。",
        ],
    ),
    (
        "入り直します",
        [
            "入り直しますので、このまま少し待っていてください。",
            "入り直します。つながったら続きを話しましょう。",
        ],
    ),
    (
        "つなぎ直します",
        [
            "つなぎ直しますので、少しお待ちください。",
            "つなぎ直します。音が戻ったか教えてください。",
        ],
    ),
];
const PARTIALS: &[&str] = &[
    "kikoeniku",
    "myu-tokaij",
    "gamengatomatteima",
    "otogazureteima",
    "hairinaoshima",
    "tsunaginaoshima",
];

#[test]
fn call_nine_packs_keep_prior_contact_and_route_choices() {
    const NAME: &str = "call_nine_packs_keep_prior_contact_and_route_choices";
    fresh(NAME, || {
        for (seed, word, sentence) in [
            ("tsuuwa", "通話", "通話は今できますか。"),
            ("denpa", "電波", "電波が弱いので、少し待ってください。"),
            ("gamenkyouyuu", "画面共有", "画面共有を始めてもいいですか。"),
            (
                "machiawasebasho",
                "待ち合わせ場所",
                "待ち合わせ場所を駅前に変えてもいいですか。",
            ),
            ("betsuru-to", "別ルート", "別ルートで向かっています。"),
        ] {
            let mut ime = engine(seed);
            draft(&ime, seed);
            first_page(&ime, word, CandidateKind::Word);
            index(&ime, sentence, CandidateKind::Sentence);
            commit_once(&mut ime, sentence, CandidateKind::Sentence);
            let mut ime = engine(seed);
            commit_once(&mut ime, word, CandidateKind::Word);
            let mut ime = engine(seed);
            commit_once(&mut ime, seed, CandidateKind::Literal);
        }
        // The existing internet-pack Word remains selectable, but its short
        // prefix can legitimately gain the new, longer ミュート解除 completion.
        let mut ime = engine("myu-to");
        index(&ime, "ミュート", CandidateKind::Word);
        commit_once(&mut ime, "ミュート", CandidateKind::Word);
    });
}

#[test]
fn call_nine_packs_reach_every_new_word_and_author_without_losing_raw() {
    const NAME: &str = "call_nine_packs_reach_every_new_word_and_author_without_losing_raw";
    fresh(NAME, || {
        for (i, &(romaji, kana, kata, word)) in WORDS.iter().enumerate() {
            assert_eq!(word, SENTENCES[i].0);
            for seed in [
                romaji.to_owned(),
                romaji.to_ascii_uppercase(),
                kana.into(),
                PARTIALS[i].into(),
            ] {
                let mut ime = engine(&seed);
                draft(&ime, &seed);
                first_page(&ime, word, CandidateKind::Word);
                for sentence in SENTENCES[i].1 {
                    index(&ime, sentence, CandidateKind::Sentence);
                    let mut ime = engine(&seed);
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
                commit_once(&mut ime, word, CandidateKind::Word);
                let mut ime = engine(&seed);
                commit_once(&mut ime, &seed, CandidateKind::Literal);
                let adopted = format!("{word} ");
                for sentence in SENTENCES[i].1 {
                    // Japanese reading separators are not authored payload.
                    // The canonical sentence has no inserted space; selecting
                    // Literal below must still preserve that space exactly.
                    let mut ime = engine(&adopted);
                    draft(&ime, &adopted);
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
                let mut ime = engine(&adopted);
                commit_once(&mut ime, &adopted, CandidateKind::Literal);
            }
            // Katakana is deliberately only a lossless literal here.
            let mut ime = engine(kata);
            draft(&ime, kata);
            commit_once(&mut ime, kata, CandidateKind::Literal);
        }
    });
}
