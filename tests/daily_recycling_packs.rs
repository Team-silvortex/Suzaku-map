//! Finite recycling dialogue alongside private packs, never a personal pack store.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_RECYCLING_PACKS_CHILD";
const NAME: &str =
    "recycling_words_authors_and_old_household_choices_coexist_in_sixteen_private_packs";

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
    assert!(ime.candidates().iter().all(|c| {
        c.label == c.text
            && c.source == CandidateSource::Local
            && c.score.is_finite()
            && (0.0..=100.0).contains(&c.score)
    }));
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
    let repeated = ime.commit(CommitOptions { force: true });
    assert!(!repeated.ok && repeated.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
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
    let kind = ime
        .candidates()
        .iter()
        .find(|c| c.text == seed)
        .unwrap()
        .kind;
    assert!(matches!(kind, CandidateKind::Literal | CandidateKind::Word));
    commit(language, seed, seed, kind);
}

#[test]
fn recycling_words_authors_and_old_household_choices_coexist_in_sixteen_private_packs() {
    if std::env::var(CHILD).as_deref() != Ok("enabled") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
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
        assert!(out.contains("RECYCLING_OLD_CONTROLS_OK:16"));
        assert!(out.contains("RECYCLING_NEW_CHOICES_OK:16"));
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "suzaku-recycling-packs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let owned = Owned(directory);
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    assert!(store.list().unwrap().is_empty());
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

    // Check immutable old builtin and optional-family paths before any new data.
    for (seed, word, sentence) in [
        (
            "i'll take the trash ou",
            "i'll take the trash out",
            "i'll take the trash out after dinner.",
        ),
        (
            "please empty the bi",
            "please empty the bin",
            "please empty the bin before it gets full.",
        ),
        (
            "please separate the recyclin",
            "please separate the recycling",
            "please separate the recycling.",
        ),
        (
            "can you take care of the bin",
            "can you take care of the bins",
            "can you take care of the bins this evening?",
        ),
    ] {
        choices("en", seed, &[word], &[sentence]);
    }
    for (seed, word, sentences) in [
        (
            "dao la ji",
            "倒垃圾",
            ["倒垃圾的时候顺便带上这个袋子。", "倒垃圾的时间到了。"],
        ),
        (
            "la ji dai man le",
            "垃圾袋满了",
            [
                "垃圾袋满了，换一个新的吧。",
                "垃圾袋满了，我出门时顺手带下去。",
            ],
        ),
        (
            "jia wu fen gong",
            "家务分工",
            [
                "家务分工先一起商量，不用一个人全做。",
                "家务分工说好了，我们就分别开始。",
            ],
        ),
    ] {
        choices("zh-Hans", seed, &[word], &sentences);
    }
    choices("zh-Hans", "la ji dai", &["垃圾袋"], &[]);
    choices("zh-Hans", "zhi xiang", &["纸箱"], &[]);
    println!("RECYCLING_OLD_CONTROLS_OK:16");

    for (context, words, partials, sentences) in [
        (
            "these empty cartons can wait",
            ["here", "outside"],
            ["her", "outsid"],
            [
                "these empty cartons can wait here until we take them downstairs.",
                "these empty cartons can wait outside with the cardboard.",
            ],
        ),
        (
            "the cardboard is ready for",
            ["collection", "recycling"],
            ["collectio", "recyclin"],
            [
                "the cardboard is ready for collection by the door.",
                "the cardboard is ready for recycling and i will carry it downstairs.",
            ],
        ),
        (
            "i have left the empty cans",
            ["here", "downstairs"],
            ["her", "downstair"],
            [
                "i have left the empty cans here with the bottles.",
                "i have left the empty cans downstairs next to the bins.",
            ],
        ),
        (
            "these bins still need new",
            ["liners", "labels"],
            ["liner", "label"],
            [
                "these bins still need new liners before we use them.",
                "these bins still need new labels so we can find the right one.",
            ],
        ),
        (
            "the spare bin liners are",
            ["under", "inside"],
            ["unde", "insid"],
            [
                "the spare bin liners are under the sink with the spare bags.",
                "the spare bin liners are inside the cupboard in the kitchen.",
            ],
        ),
        (
            "i can take these bags",
            ["down", "outside"],
            ["dow", "outsid"],
            [
                "i can take these bags down when i leave.",
                "i can take these bags outside after dinner.",
            ],
        ),
    ] {
        let seed = format!("{context} ");
        let full_words = words.map(|word| format!("{seed}{word}"));
        choices(
            "en",
            &seed,
            &full_words.iter().map(String::as_str).collect::<Vec<_>>(),
            &sentences,
        );
        for branch in 0..2 {
            assert!(words[branch].starts_with(partials[branch]));
            assert_ne!(words[branch], partials[branch]);
            for seed in [
                format!("{context} {}", partials[branch]),
                full_words[branch].clone(),
            ] {
                choices("en", &seed, &[&full_words[branch]], &[sentences[branch]]);
            }
        }
    }
    for (reading, partial, word, sentences) in [
        (
            "zhi xiang xian ya ping",
            "zhi xiang xian ya pin",
            "纸箱先压平",
            [
                "纸箱先压平，我来把它们放好。",
                "纸箱先压平，等会儿再一起搬下去。",
            ],
        ),
        (
            "kong ping fang zai zhe bian",
            "kong ping fang zai zhe bia",
            "空瓶放在这边",
            [
                "空瓶放在这边，我先找个袋子。",
                "空瓶放在这边，别和还没喝完的放一起。",
            ],
        ),
        (
            "zhe xie bao zhuang xian bie reng",
            "zhe xie bao zhuang xian bie ren",
            "这些包装先别扔",
            [
                "这些包装先别扔，我再确认一下。",
                "这些包装先别扔，里面可能还有配件。",
            ],
        ),
        (
            "dai kou xian ji hao",
            "dai kou xian ji ha",
            "袋口先系好",
            ["袋口先系好，我来拿另一袋。", "袋口先系好，放在门边等我。"],
        ),
        (
            "hui shou ri shi na tian",
            "hui shou ri shi na tia",
            "回收日是哪天",
            [
                "回收日是哪天？我先记下来。",
                "回收日是哪天？我们再看一下通知。",
            ],
        ),
        (
            "zhe dai la ji wo lai ti",
            "zhe dai la ji wo lai t",
            "这袋垃圾我来提",
            [
                "这袋垃圾我来提，你帮我开一下门。",
                "这袋垃圾我来提，另外那袋先放着。",
            ],
        ),
    ] {
        for seed in [reading, partial] {
            choices("zh-Hans", seed, &[word], &sentences);
        }
    }
    println!("RECYCLING_NEW_CHOICES_OK:16");
}
