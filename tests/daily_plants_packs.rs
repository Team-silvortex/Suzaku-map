//! Plant vocabulary in a private startup catalog, never a personal pack store.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_PLANTS_PACKS_CHILD";
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
fn plants_words_authors_and_old_watering_workflows_coexist_in_sixteen_private_packs() {
    if std::env::var(CHILD).as_deref() != Ok("enabled") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "plants_words_authors_and_old_watering_workflows_coexist_in_sixteen_private_packs",
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
        assert!(out.contains("PLANTS_OLD_CONTROLS_OK:16"));
        assert!(out.contains("PLANTS_NEW_CHOICES_OK:16"));
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "suzaku-plants-packs-{}-{}",
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
            "could you water the plants",
            "could you water the plants while i am away?",
        ),
        (
            "could you water the garden",
            "could you water the garden this evening?",
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
        "jiao shui",
        &["浇水"],
        &["浇水的时间需要调整一下。", "浇水用的水壶在阳台。"],
    );
    choices(
        "zh-Hans",
        "gei hua jiao shui",
        &["给花浇水"],
        &["给花浇水的事就拜托你了。", "给花浇水时别倒太多。"],
    );
    println!("PLANTS_OLD_CONTROLS_OK:16");

    for (seed, words, authors) in [
        (
            "the soil in this pot feels ",
            ["dry", "wet"],
            [
                "the soil in this pot feels dry after the warm afternoon.",
                "the soil in this pot feels wet after watering this morning.",
            ],
        ),
        (
            "this plant on the balcony has ",
            ["new", "small"],
            [
                "this plant on the balcony has new leaves near the top.",
                "this plant on the balcony has small flowers by the window.",
            ],
        ),
        (
            "before watering these plants i'll ",
            ["check", "move"],
            [
                "before watering these plants i'll check the soil in each pot.",
                "before watering these plants i'll move my notebook away from the water.",
            ],
        ),
        (
            "the flower in this pot is ",
            ["white", "yellow"],
            [
                "the flower in this pot is white with a yellow center.",
                "the flower in this pot is yellow and easy to see from the door.",
            ],
        ),
        (
            "the leaf on this plant looks ",
            ["green", "dry"],
            [
                "the leaf on this plant looks green in the sunlight.",
                "the leaf on this plant looks dry along the edge.",
            ],
        ),
        (
            "while you were out i watered the ",
            ["plants", "flowers"],
            [
                "while you were out i watered the plants on the balcony.",
                "while you were out i watered the flowers in the small pots.",
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
            "zhe pen hua xian fang chuang bian",
            "这盆花先放窗边",
            [
                "这盆花先放窗边，我腾个位置出来。",
                "这盆花先放窗边，等会儿再搬到阳台。",
            ],
        ),
        (
            "hua pen di xia you dian shui",
            "花盆底下有点水",
            [
                "花盆底下有点水，我拿块布擦一下。",
                "花盆底下有点水，先把桌面擦干吧。",
            ],
        ),
        (
            "zhe pian ye zi diao xia lai le",
            "这片叶子掉下来了",
            [
                "这片叶子掉下来了，我先捡起来。",
                "这片叶子掉下来了，放进垃圾袋里吧。",
            ],
        ),
        (
            "bang wo na yi xia pen hu",
            "帮我拿一下喷壶",
            [
                "帮我拿一下喷壶，我把它放到花盆旁边。",
                "帮我拿一下喷壶，用完我就放回去。",
            ],
        ),
        (
            "yang tai de hua wo lai jiao",
            "阳台的花我来浇",
            [
                "阳台的花我来浇，我先去拿水壶。",
                "阳台的花我来浇，你先收一下桌面吧。",
            ],
        ),
        (
            "xian bie nuo zhe ge hua pen",
            "先别挪这个花盆",
            [
                "先别挪这个花盆，我来腾一下位置。",
                "先别挪这个花盆，旁边的东西还没收好。",
            ],
        ),
    ] {
        choices("zh-Hans", seed, &[word], &authors);
    }
    println!("PLANTS_NEW_CHOICES_OK:16");
}
