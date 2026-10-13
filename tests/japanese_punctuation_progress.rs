//! Authored grammatical progress stays intact when the user adds punctuation.
//! Fixed expectations in private engine snapshots; no native keyboard or model.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_JAPANESE_PUNCTUATION_PROGRESS_CHILD";
const STUDY: &str = "日本語を勉強したいです。";
const DONE: &str = "完了したら連絡してください。";
const QUOTE: &str = "引用した部分が分かるように印を付けます。";
const QUOTE_SOURCE: &str = "引用の出典をもう一度確かめます。";
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

fn fresh(name: &str, with_packs: bool, body: impl FnOnce()) {
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
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "suzaku-ja-punctuation-progress-{}",
        std::process::id()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let store = PackStore::new(fixture.0.join("packs")).unwrap();
    if with_packs {
        let catalog = packs::recommended();
        for id in JAPANESE_IDS {
            let pack = catalog.iter().find(|p| p.manifest.id == id).unwrap();
            store.install(pack.clone(), false).unwrap();
        }
    }
    let report = packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert!(report.disabled.is_empty());
    assert_eq!(report.loaded.len(), if with_packs { 9 } else { 0 });
    if !with_packs {
        assert!(!store.path().exists());
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
        .position(|c| c.text == text && c.kind == kind)
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

fn no_recovered_authored_sentence(ime: &XRTabletImeEngine) {
    assert!(
        ime.candidates()
            .iter()
            .all(|c| [STUDY, DONE, QUOTE, QUOTE_SOURCE]
                .iter()
                .all(|s| !c.text.ends_with(s))),
        "discarded a boundary or conflict in {:?}: {:?}",
        ime.snapshot().seed_text,
        ime.candidates()
    );
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

fn punctuated(seed: &str, expected: &str) {
    let mut ime = engine(seed);
    draft(&ime, seed);
    assert_eq!(ime.candidates()[0].text, expected, "{seed:?}");
    assert_eq!(ime.candidates()[0].score, 100.0, "{seed:?}");
    let kind = if seed == expected {
        CandidateKind::Literal
    } else {
        CandidateKind::Sentence
    };
    assert_eq!(ime.candidates()[0].kind, kind, "{seed:?}");
    no_recovered_authored_sentence(&ime);
    commit_once(&mut ime, expected, kind);
    commit_once(&mut engine(seed), seed, CandidateKind::Literal);
}

fn editing(prefix: &str, primary: &str, sentence: &str) {
    let mut ime = engine(prefix);
    for mark in ["", "!", "!?", "!", "", "？", "？\"", ""] {
        let seed = format!("{prefix}{mark}");
        ime.seed(&seed);
        draft(&ime, &seed);
        assert_eq!(ime.candidates()[0].text, format!("{primary}{mark}"));
        if mark.is_empty() {
            index(&ime, sentence, CandidateKind::Sentence);
        } else {
            no_recovered_authored_sentence(&ime);
        }
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

fn protected(prefix: &str, roman: &str) {
    let mut seeds = vec![
        format!("xyz{prefix}shita!"),
        format!("🙂{prefix}shita!"),
        format!("{prefix}shitaxyz!"),
        format!("{prefix}shitanai!"),
        format!("{prefix}shita1!"),
        format!("{prefix}shita_!"),
        format!("https://example.invalid/{roman}shita!"),
        format!("let x = \"{roman}shita!\";"),
        format!("{}{prefix}shita!", "前文。".repeat(90)),
        // A punctuation boundary must not turn an explicit nasal back into n.
        format!("{prefix}nn!"),
        format!("{prefix}xn?"),
        format!("{prefix}n'。"),
        format!("{prefix}n’！"),
        format!("{prefix}ん？"),
    ];
    for boundary in [
        "\n", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("{prefix}{boundary}shita!"));
        seeds.push(format!("{prefix}shita{boundary}!"));
    }
    for seed in seeds {
        let mut ime = engine(&seed);
        draft(&ime, &seed);
        no_recovered_authored_sentence(&ime);
        commit_once(&mut ime, &seed, CandidateKind::Literal);
    }
}

#[test]
fn builtin_punctuation_keeps_authored_grammar_and_exact_marks() {
    fresh(
        "builtin_punctuation_keeps_authored_grammar_and_exact_marks",
        false,
        || {
            for (seed, expected) in [
                ("完了shita!", "完了した!"),
                ("完了SHITA？", "完了した？"),
                ("完了shita。", "完了した。"),
                ("完了shita!\"", "完了した!\""),
                ("日本語を勉強shitai。", "日本語を勉強したい。"),
                ("日本語を勉強shitai！」", "日本語を勉強したい！」"),
                ("日本語を勉強したいです。!", "日本語を勉強したいです。!"),
            ] {
                punctuated(seed, expected);
            }
        },
    );
}

#[test]
fn nine_pack_quote_grammar_survives_question_marks_and_closing_quotes() {
    fresh(
        "nine_pack_quote_grammar_survives_question_marks_and_closing_quotes",
        true,
        || {
            for (seed, expected) in [
                ("引用shita?", "引用した?"),
                ("引用shita。", "引用した。"),
                ("引用SHITA？", "引用した？"),
                ("in'youshita!", "引用した!"),
                ("引用shita?」", "引用した?」"),
                ("引用shita。』", "引用した。』"),
            ] {
                punctuated(seed, expected);
            }
        },
    );
}

#[test]
fn builtin_punctuation_insertion_and_deletion_refreshes_the_same_draft() {
    fresh(
        "builtin_punctuation_insertion_and_deletion_refreshes_the_same_draft",
        false,
        || editing("完了shita", "完了した", DONE),
    );
}

#[test]
fn nine_pack_punctuation_edits_restore_only_the_unclosed_sentence_query() {
    fresh(
        "nine_pack_punctuation_edits_restore_only_the_unclosed_sentence_query",
        true,
        || editing("引用shita", "引用した", QUOTE),
    );
}

#[test]
fn builtin_punctuation_does_not_erase_conflicts_or_literal_boundaries() {
    fresh(
        "builtin_punctuation_does_not_erase_conflicts_or_literal_boundaries",
        false,
        || protected("完了", "kanryou"),
    );
}

#[test]
fn nine_pack_punctuation_does_not_recover_sentences_through_protected_text() {
    fresh(
        "nine_pack_punctuation_does_not_recover_sentences_through_protected_text",
        true,
        || protected("引用", "in'you"),
    );
}

#[test]
fn punctuation_does_not_hide_independent_location_and_longer_nouns() {
    fresh(
        "punctuation_does_not_hide_independent_location_and_longer_nouns",
        true,
        || {
            for (seed, expected) in [
                ("shita!", "下!"),
                ("入口noshita。", "入口の下。"),
                ("shitagaki?", "下書き?"),
                ("shitagoshirae！", "下ごしらえ！"),
                ("引用shitagaki?", "引用下書き?"),
            ] {
                punctuated(seed, expected);
            }
        },
    );
}
