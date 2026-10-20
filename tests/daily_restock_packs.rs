//! Private all-nine-pack restock snapshots; no personal stores, audio control or native input.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_DAILY_RESTOCK_PACKS_CHILD";
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
    let path = std::env::temp_dir().join(format!("suzaku-ja-restock-packs-{}", std::process::id()));
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
    ("thisshu", "てぃっしゅ", "ティッシュ", "ティッシュ"),
    (
        "toirettope-pa-",
        "といれっとぺーぱー",
        "トイレットペーパー",
        "トイレットペーパー",
    ),
    ("tsumekae", "つめかえ", "ツメカエ", "詰め替え"),
    ("kaitashi", "かいたし", "カイタシ", "買い足し"),
    ("yobi", "よび", "ヨビ", "予備"),
    (
        "nokorigasukunai",
        "のこりがすくない",
        "ノコリガスクナイ",
        "残りが少ない",
    ),
];
const SENTENCES: &[(&str, [&str; 2])] = &[
    (
        "ティッシュ",
        [
            "ティッシュが残り少なくなりました。",
            "ティッシュを一箱買い足しておきます。",
        ],
    ),
    (
        "トイレットペーパー",
        [
            "トイレットペーパーの予備はありますか。",
            "トイレットペーパーを買い物のメモに追加します。",
        ],
    ),
    (
        "詰め替え",
        [
            "詰め替えを買う前に、容器を確認します。",
            "詰め替えはいつもの棚にあります。",
        ],
    ),
    (
        "買い足し",
        [
            "買い足しが必要なものを教えてください。",
            "買い足しの前に、家にある数を確認します。",
        ],
    ),
    (
        "予備",
        ["予備は戸棚の中にあります。", "予備を一つ買っておきます。"],
    ),
    (
        "残りが少ない",
        [
            "残りが少ないので、買い物のメモに書いておきます。",
            "残りが少ないものから順に補充しましょう。",
        ],
    ),
];
const PARTIALS: &[&str] = &[
    "thissh",
    "toirettope-pa",
    "tsumeka",
    "kaitash",
    "yob",
    "nokorigasukuna",
];

#[test]
fn restock_nine_packs_keep_prior_care_contact_and_route_choices() {
    const NAME: &str = "restock_nine_packs_keep_prior_care_contact_and_route_choices";
    fresh(NAME, || {
        for (seed, word, sentence) in [
            ("kataduke", "片付け", "片付けを手伝ってもらえますか。"),
            ("okibasho", "置き場所", "置き場所を決めておきます。"),
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
        for (seed, word) in [
            ("seiri", "整理"),
            ("yobidashi", "呼び出し"),
            ("yobidash", "呼び出し"),
        ] {
            let mut ime = engine(seed);
            first_page(&ime, word, CandidateKind::Word);
            commit_once(&mut ime, word, CandidateKind::Word);
        }
    });
}

#[test]
fn restock_nine_packs_reach_every_new_word_and_author_without_losing_raw() {
    const NAME: &str = "restock_nine_packs_reach_every_new_word_and_author_without_losing_raw";
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
            // The first two canonical spellings are themselves Katakana.
            // Written input remains the raw Literal (not a duplicate Word),
            // while their authored continuations must also be reachable.
            if kata == word {
                for sentence in SENTENCES[i].1 {
                    let mut ime = engine(kata);
                    draft(&ime, kata);
                    commit_once(&mut ime, sentence, CandidateKind::Sentence);
                }
            }
            let mut ime = engine(kata);
            draft(&ime, kata);
            commit_once(&mut ime, kata, CandidateKind::Literal);
        }
    });
}
