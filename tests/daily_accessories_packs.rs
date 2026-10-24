//! Finite accessory-location dialogue alongside private packs, never a personal pack store.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_ACCESSORIES_PACKS_CHILD";
const NAME: &str = "accessory_words_authors_and_old_workflows_coexist_in_sixteen_private_packs";

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
fn accessory_words_authors_and_old_workflows_coexist_in_sixteen_private_packs() {
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
            .env_remove("IBUS_ADDRESS")
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{out}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(out.contains("running 1 test"));
        assert!(out.contains("ACCESSORIES_OLD_CONTROLS_OK:16"));
        assert!(out.contains("ACCESSORIES_NEW_CHOICES_OK:16"));
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "suzaku-accessories-packs-{}-{}",
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
    let expected_ids = [
        "org.suzaku.en.outdoors",
        "org.suzaku.zh-hans.outdoors",
        "org.suzaku.en.study",
        "org.suzaku.zh-hans.study",
        "org.suzaku.en.cooking",
        "org.suzaku.zh-hans.cooking",
        "org.suzaku.en.travel",
        "org.suzaku.zh-hans.travel",
        "org.suzaku.en.work",
        "org.suzaku.zh-hans.work",
        "org.suzaku.en.family",
        "org.suzaku.zh-hans.family",
        "org.suzaku.en.shopping",
        "org.suzaku.zh-hans.shopping",
        "org.suzaku.en.internet",
        "org.suzaku.zh-hans.internet",
    ];
    let selected: Vec<_> = packs::recommended()
        .into_iter()
        .filter(|p| matches!(p.manifest.language.as_str(), "en" | "zh-Hans"))
        .collect();
    assert_eq!(
        selected
            .iter()
            .map(|p| p.manifest.id.as_str())
            .collect::<Vec<_>>(),
        expected_ids
    );
    for language in ["en", "zh-Hans"] {
        assert_eq!(
            selected
                .iter()
                .filter(|p| p.manifest.language == language)
                .count(),
            8
        );
    }
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let installed = store.list().unwrap();
    assert_eq!(installed.len(), 16);
    assert!(installed.iter().all(|p| p.enabled));
    assert_eq!(
        installed
            .iter()
            .map(|p| p.package.manifest.id.as_str())
            .collect::<Vec<_>>(),
        expected_ids
    );
    let registry = fs::read(store.path()).unwrap();
    let report = packs::initialize(&store).unwrap();
    assert!(
        report.errors.is_empty() && report.disabled.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.loaded.len(), 16);
    assert_eq!(
        report
            .loaded
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>(),
        expected_ids.into_iter().collect::<HashSet<_>>()
    );

    // Old complete charging, family and recycling branches run before new data.
    for (seed, word, sentence) in [
        (
            "could i borrow your char",
            "could i borrow your charger",
            "could i borrow your charger until tonight?",
        ),
        (
            "i need to bring my char",
            "i need to bring my charger",
            "i need to bring my charger before we leave.",
        ),
        (
            "the charging cable is loo",
            "the charging cable is loose",
            "the charging cable is loose near the phone.",
        ),
        (
            "can you take care of the bin",
            "can you take care of the bins",
            "can you take care of the bins this evening?",
        ),
        (
            "the cardboard is ready for collectio",
            "the cardboard is ready for collection",
            "the cardboard is ready for collection by the door.",
        ),
    ] {
        choices("en", seed, &[word], &[sentence]);
    }
    for (seed, word, sentences) in [
        (
            "chong dian qi",
            "充电器",
            ["充电器可以借用一下吗？", "充电器放在包里了。"],
        ),
        (
            "chong dian xian",
            "充电线",
            ["充电线已经插好了。", "充电线可以借我用一下吗？"],
        ),
        (
            "zhi xiang xian ya ping",
            "纸箱先压平",
            [
                "纸箱先压平，我来把它们放好。",
                "纸箱先压平，等会儿再一起搬下去。",
            ],
        ),
        (
            "hui shou ri shi na tian",
            "回收日是哪天",
            [
                "回收日是哪天？我先记下来。",
                "回收日是哪天？我们再看一下通知。",
            ],
        ),
    ] {
        choices("zh-Hans", seed, &[word], &sentences);
    }
    for (reading, word) in [
        ("zhuan jie tou", "转接头"),
        ("er ji he", "耳机盒"),
        ("dian yuan", "电源"),
    ] {
        choices("zh-Hans", reading, &[word], &[]);
    }
    assert_eq!(fs::read(store.path()).unwrap(), registry);
    println!("ACCESSORIES_OLD_CONTROLS_OK:16");

    for (context, words, partials, sentences) in [
        (
            "the spare charging cable is",
            ["here", "inside"],
            ["her", "insid"],
            [
                "the spare charging cable is here on the desk.",
                "the spare charging cable is inside the small pouch.",
            ],
        ),
        (
            "i put the adapter in the",
            ["drawer", "pouch"],
            ["drawe", "pouc"],
            [
                "i put the adapter in the drawer with the spare cables.",
                "i put the adapter in the pouch next to my earbuds.",
            ],
        ),
        (
            "my earbuds are still in the",
            ["case", "pouch"],
            ["cas", "pouc"],
            [
                "my earbuds are still in the case next to my notebook.",
                "my earbuds are still in the pouch i brought yesterday.",
            ],
        ),
        (
            "the connectors for these cables are",
            ["here", "there"],
            ["her", "ther"],
            [
                "the connectors for these cables are here in the top drawer.",
                "the connectors for these cables are there in the small box.",
            ],
        ),
        (
            "i left the spare cords",
            ["here", "upstairs"],
            ["her", "upstair"],
            [
                "i left the spare cords here with the chargers.",
                "i left the spare cords upstairs in the desk drawer.",
            ],
        ),
        (
            "these small pouches can hold",
            ["cables", "adapters"],
            ["cable", "adapter"],
            [
                "these small pouches can hold cables while we travel.",
                "these small pouches can hold adapters and the spare connector.",
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
            "cha xian ban xian fang zhe bian",
            "cha xian ban xian fang zhe bia",
            "插线板先放这边",
            [
                "插线板先放这边，我来腾出一点位置。",
                "插线板先放这边，等整理好再收起来。",
            ],
        ),
        (
            "yan chang xian shou zai na le",
            "yan chang xian shou zai na l",
            "延长线收在哪了",
            [
                "延长线收在哪了？我只找到旁边的袋子。",
                "延长线收在哪了？你看到后告诉我。",
            ],
        ),
        (
            "zhuan jie tou bang wo liu zhe",
            "zhuan jie tou bang wo liu zh",
            "转接头帮我留着",
            [
                "转接头帮我留着，我等会儿来拿。",
                "转接头帮我留着，放在小盒子里就行。",
            ],
        ),
        (
            "dian yuan xian bie na hun le",
            "dian yuan xian bie na hun l",
            "电源线别拿混了",
            [
                "电源线别拿混了，我再看一下标签。",
                "电源线别拿混了，这根上面贴了标签。",
            ],
        ),
        (
            "er ji he fang zai zhuo bian",
            "er ji he fang zai zhuo bia",
            "耳机盒放在桌边",
            [
                "耳机盒放在桌边，和钥匙放在一起。",
                "耳机盒放在桌边，我出门前会拿走。",
            ],
        ),
        (
            "zhe xie xian xian fen kai fang",
            "zhe xie xian xian fen kai fan",
            "这些线先分开放",
            [
                "这些线先分开放，我来找几个收纳袋。",
                "这些线先分开放，标签朝外比较好找。",
            ],
        ),
    ] {
        for seed in [reading, partial] {
            choices("zh-Hans", seed, &[word], &sentences);
        }
        choices("zh-Hans", word, &[], &sentences);
    }
    for (reading, word) in [
        ("cha xian ban", "插线板"),
        ("yan chang xian", "延长线"),
        ("dian yuan xian", "电源线"),
        ("li xian jia", "理线夹"),
        ("zhuan jie tou", "转接头"),
        ("er ji he", "耳机盒"),
    ] {
        choices("zh-Hans", reading, &[word], &[]);
    }
    assert_eq!(fs::read(store.path()).unwrap(), registry);
    assert!(store.list().unwrap().iter().all(|p| p.enabled));
    println!("ACCESSORIES_NEW_CHOICES_OK:16");
}
