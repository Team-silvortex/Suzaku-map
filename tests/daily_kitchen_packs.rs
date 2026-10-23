//! Kitchen vocabulary in a private startup catalog, never a personal pack store.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_KITCHEN_PACKS_CHILD";
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
fn kitchen_words_authors_and_old_mealtime_workflows_coexist_in_sixteen_private_packs() {
    if std::env::var(CHILD).as_deref() != Ok("enabled") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "kitchen_words_authors_and_old_mealtime_workflows_coexist_in_sixteen_private_packs",
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
        assert!(out.contains("KITCHEN_OLD_CONTROLS_OK:16"));
        assert!(out.contains("KITCHEN_NEW_CHOICES_OK:16"));
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let path = std::env::temp_dir().join(format!(
        "suzaku-kitchen-packs-{}-{}",
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
    println!("KITCHEN_OLD_CONTROLS_OK:16");
    for (seed, words, authors) in [
        (
            "on the cutting board i've left the ",
            ["vegetables", "bread"],
            [
                "on the cutting board i've left the vegetables you asked me to bring.",
                "on the cutting board i've left the bread for our lunch.",
            ],
        ),
        (
            "the clean bowls are ready for ",
            ["soup", "rice"],
            [
                "the clean bowls are ready for soup when you bring it over.",
                "the clean bowls are ready for rice and i'll carry them to the table.",
            ],
        ),
        (
            "next to the plates i've put the ",
            ["spoons", "forks"],
            [
                "next to the plates i've put the spoons for everyone.",
                "next to the plates i've put the forks from the kitchen drawer.",
            ],
        ),
        (
            "my apron is hanging near the ",
            ["door", "sink"],
            [
                "my apron is hanging near the door where i can find it.",
                "my apron is hanging near the sink with the clean towels.",
            ],
        ),
        (
            "the drying rack still has the ",
            ["cups", "bowls"],
            [
                "the drying rack still has the cups we used at lunch.",
                "the drying rack still has the bowls from our lunch.",
            ],
        ),
        (
            "these lids belong to the ",
            ["containers", "pots"],
            [
                "these lids belong to the containers on the tray.",
                "these lids belong to the pots in the cupboard.",
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
            "wo xian ba cai xi hao",
            "我先把菜洗好",
            [
                "我先把菜洗好，你把盘子拿出来。",
                "我先把菜洗好，洗完再叫你过来。",
            ],
        ),
        (
            "qie hao de cai fang zhe bian",
            "切好的菜放这边",
            [
                "切好的菜放这边，我等会儿来拿。",
                "切好的菜放这边，空盘子放另一边。",
            ],
        ),
        (
            "bang wo na yi xia cai ban",
            "帮我拿一下菜板",
            [
                "帮我拿一下菜板，我腾不出手。",
                "帮我拿一下菜板，就在下面的柜子里。",
            ],
        ),
        (
            "pan zi zai na liang ge",
            "盘子再拿两个",
            [
                "盘子再拿两个，还有两个人没拿到。",
                "盘子再拿两个，我来摆好。",
            ],
        ),
        (
            "chi wan wo lai shou zhuo zi",
            "吃完我来收桌子",
            [
                "吃完我来收桌子，你先坐着休息。",
                "吃完我来收桌子，碗筷留给我就好。",
            ],
        ),
        (
            "xi hao de wan fang zhe bian",
            "洗好的碗放这边",
            [
                "洗好的碗放这边，我来擦干。",
                "洗好的碗放这边，柜子里还有空位。",
            ],
        ),
    ] {
        choices("zh-Hans", seed, &[word], &authors);
    }
    println!("KITCHEN_NEW_CHOICES_OK:16");
}
