//! Bedding vocabulary in a private startup catalog, never a personal pack store.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_BEDDING_PACKS_CHILD";
struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn engine(language: &str, seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: language.into(),
        ..Default::default()
    });
    ime.enable_ibus_candidate_mix();
    ime.seed(seed);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates().iter().all(|c| c.label == c.text
            && c.source == CandidateSource::Local
            && c.score.is_finite())
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|c| &c.text)
            .collect::<HashSet<_>>()
            .len()
    );
    assert!(ime.candidates().iter().any(|c| c.text == seed));
    ime
}
fn commit(language: &str, seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(language, seed);
    let at = ime
        .candidates()
        .iter()
        .position(|c| c.text == text && c.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?} {text:?}: {:?}", ime.candidates()));
    if matches!(kind, CandidateKind::Word | CandidateKind::Sentence) {
        assert!(at < PAGE_SIZE, "{text:?} left page one for {seed:?}");
    }
    ime.select_candidate(at);
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert_eq!(ime.selected_completion_text(true), Some(text));
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
    assert!(!ime.commit(CommitOptions { force: true }).ok);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}
fn choices(language: &str, seed: &str, words: &[&str], sentences: &[&str]) {
    for word in words {
        commit(language, seed, word, CandidateKind::Word);
    }
    for sentence in sentences {
        commit(language, seed, sentence, CandidateKind::Sentence);
    }
    let ime = engine(language, seed);
    let raw = ime
        .candidates()
        .iter()
        .find(|c| c.text == seed)
        .unwrap()
        .kind;
    assert!(matches!(raw, CandidateKind::Literal | CandidateKind::Word));
    commit(language, seed, seed, raw);
}

#[test]
fn bedding_words_authors_and_old_bedlinen_workflows_coexist_in_sixteen_private_packs() {
    if std::env::var(CHILD).as_deref() != Ok("enabled") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bedding_words_authors_and_old_bedlinen_workflows_coexist_in_sixteen_private_packs",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "enabled")
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("SUZAKU_IME_CONFIG")
            .env_remove("SUZAKU_LINUX_IME_SOCKET")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{out}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out.contains("running 1 test"));
        assert!(out.contains("BEDDING_OLD_CONTROLS_OK:16"));
        assert!(out.contains("BEDDING_NEW_CHOICES_OK:16"));
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "suzaku-bedding-packs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    let owned = Owned(path);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    let selected: Vec<_> = packs::recommended()
        .into_iter()
        .filter(|p| matches!(p.manifest.language.as_str(), "en" | "zh-Hans"))
        .collect();
    assert_eq!(selected.len(), 16);
    for language in ["en", "zh-Hans"] {
        assert_eq!(
            selected
                .iter()
                .filter(|p| p.manifest.language == language)
                .count(),
            8
        );
    }
    let expected: HashSet<_> = selected.iter().map(|p| p.manifest.id.clone()).collect();
    assert_eq!(expected.len(), 16);
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(
        report.errors.is_empty() && report.disabled.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.loaded.len(), 16);
    assert_eq!(
        report.loaded.iter().cloned().collect::<HashSet<_>>(),
        expected
    );
    for (seed, sentence) in [
        (
            "i can wash the dishes",
            "i can wash the dishes after dinner.",
        ),
        (
            "i can wash the cups",
            "i can wash the cups while you clear the table.",
        ),
        (
            "the paperback on my desk is ope",
            "the paperback on my desk is open at the page i need.",
        ),
    ] {
        choices("en", seed, &[], &[sentence]);
    }
    choices(
        "zh-Hans",
        "wo lai xi wan",
        &["我来洗碗"],
        &["我来洗碗，你把桌子擦一下。", "我来洗碗，洗完就过去找你。"],
    );
    choices(
        "zh-Hans",
        "ba wan kuai bai hao",
        &["把碗筷摆好"],
        &["把碗筷摆好，我们准备吃饭。", "把碗筷摆好，汤也端上来了。"],
    );
    for (seed, sentence) in [
        (
            "we need to change the bedsheets",
            "we need to change the bedsheets today.",
        ),
        (
            "we need to change the pillowcases",
            "we need to change the pillowcases before our visitors arrive.",
        ),
        (
            "could you fold the bedsheets",
            "could you fold the bedsheets before putting them away?",
        ),
    ] {
        choices("en", seed, &[], &[sentence]);
    }
    println!("BEDDING_OLD_CONTROLS_OK:16");
    for (seed, words, authors) in [
        (
            "our spare bed still needs the ",
            ["sheet", "pillows"],
            [
                "our spare bed still needs the sheet from the clean laundry.",
                "our spare bed still needs the pillows you left in the bedroom.",
            ],
        ),
        (
            "these sheets can go in the ",
            ["cupboard", "basket"],
            [
                "these sheets can go in the cupboard with the spare pillowcases.",
                "these sheets can go in the basket until i put them away.",
            ],
        ),
        (
            "my slippers and pajamas are ready ",
            ["upstairs", "here"],
            [
                "my slippers and pajamas are ready upstairs in the bedroom.",
                "my slippers and pajamas are ready here on the chair.",
            ],
        ),
        (
            "the bedside lamp is on the ",
            ["table", "shelf"],
            [
                "the bedside lamp is on the table next to the book i'm reading.",
                "the bedside lamp is on the shelf where i left my glasses.",
            ],
        ),
    ] {
        let full_words = words.map(|word| format!("{seed}{word}"));
        choices(
            "en",
            seed,
            &full_words.iter().map(String::as_str).collect::<Vec<_>>(),
            &authors,
        );
    }
    for (seed, word, authors) in [
        (
            "zhen tou fang zai chuang tou",
            "枕头放在床头",
            [
                "枕头放在床头，我来整理被子。",
                "枕头放在床头，别压住旁边的书。",
            ],
        ),
        (
            "bei zi xian die qi lai",
            "被子先叠起来",
            [
                "被子先叠起来，我把床单拉平。",
                "被子先叠起来，等会儿再放进柜子。",
            ],
        ),
        (
            "chuang dan huan hao le ma",
            "床单换好了吗",
            [
                "床单换好了吗？我把枕套拿过来。",
                "床单换好了吗？旧的我来收起来。",
            ],
        ),
        (
            "shui qian ba chuang pu hao",
            "睡前把床铺好",
            [
                "睡前把床铺好，我先去洗漱。",
                "睡前把床铺好，衣服放到椅子上。",
            ],
        ),
    ] {
        choices("zh-Hans", seed, &[word], &authors);
    }
    println!("BEDDING_NEW_CHOICES_OK:16");
}
