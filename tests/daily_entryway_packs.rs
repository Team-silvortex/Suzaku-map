//! Finite entryway coordination dialogue alongside private packs, never a personal pack store.
use std::{collections::HashSet, fs, path::PathBuf, process::Command};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    lexicon::packs::{self, PackStore},
};

const CHILD: &str = "SUZAKU_ENTRYWAY_PACKS_CHILD";
const NAME: &str = "entryway_words_authors_and_old_workflows_coexist_in_sixteen_private_packs";

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
fn entryway_words_authors_and_old_workflows_coexist_in_sixteen_private_packs() {
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
        assert!(out.contains("ENTRYWAY_OLD_CONTROLS_OK:16"));
        assert!(out.contains("ENTRYWAY_NEW_CHOICES_OK:16"));
        assert!(out.contains("test result: ok. 1 passed; 0 failed; 0 ignored"));
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "suzaku-entryway-packs-{}-{}",
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

    // Old complete keys, umbrella, shoes and accessory authors run first.
    for (seed, word, sentence) in [
        (
            "please put on your sho",
            "please put on your shoes",
            "please put on your shoes before going outside.",
        ),
        (
            "could you help me find my ke",
            "could you help me find my keys",
            "could you help me find my keys?",
        ),
        (
            "i left my umbrella a",
            "i left my umbrella at",
            "i left my umbrella at home.",
        ),
        (
            "the spare charging cable is her",
            "the spare charging cable is here",
            "the spare charging cable is here on the desk.",
        ),
        (
            "i put the adapter in the drawe",
            "i put the adapter in the drawer",
            "i put the adapter in the drawer with the spare cables.",
        ),
    ] {
        choices("en", seed, &[word], &[sentence]);
    }
    // Existing optional shopping/travel Words are not newly invented authors.
    for word in ["shoelace", "shoelaces", "keycard"] {
        choices("en", word, &[word], &[]);
    }
    for (reading, word, sentences) in [
        (
            "bei yong yao shi",
            "备用钥匙",
            ["备用钥匙放在抽屉里。", "备用钥匙可以借我一下吗？"],
        ),
        (
            "wang dai yu san",
            "忘带雨伞",
            ["忘带雨伞了，可以借我一把吗？", "忘带雨伞了，我回去拿一下。"],
        ),
        (
            "xie dai xi hao le",
            "鞋带系好了",
            [
                "鞋带系好了，现在可以出门了。",
                "鞋带系好了，我们慢慢走过去。",
            ],
        ),
        (
            "xie dai you song le",
            "鞋带又松了",
            ["鞋带又松了，我先重新系一下。", "鞋带又松了，等我一下再走。"],
        ),
        (
            "zhun bei chu men le",
            "准备出门了",
            ["准备出门了，东西都带齐了。", "准备出门了，到了再跟你说。"],
        ),
        (
            "er ji he fang zai zhuo bian",
            "耳机盒放在桌边",
            [
                "耳机盒放在桌边，和钥匙放在一起。",
                "耳机盒放在桌边，我出门前会拿走。",
            ],
        ),
    ] {
        choices("zh-Hans", reading, &[word], &sentences);
    }
    for (reading, word) in [
        ("yao shi", "钥匙"),
        ("yu san", "雨伞"),
        ("xie gui", "鞋柜"),
        ("men dian", "门垫"),
        ("yao shi kou", "钥匙扣"),
    ] {
        choices("zh-Hans", reading, &[word], &[]);
    }
    assert_eq!(fs::read(store.path()).unwrap(), registry);
    println!("ENTRYWAY_OLD_CONTROLS_OK:16");

    for (context, words, partials, sentences) in [
        (
            "my shoes are still by the",
            ["door", "doormat"],
            ["doo", "doorma"],
            [
                "my shoes are still by the door with my bag.",
                "my shoes are still by the doormat in the entryway.",
            ],
        ),
        (
            "i left my keycard in the",
            ["pocket", "drawer"],
            ["pocke", "drawe"],
            [
                "i left my keycard in the pocket of my jacket.",
                "i left my keycard in the drawer with the spare key.",
            ],
        ),
        (
            "the umbrella is hanging on the",
            ["hook", "rack"],
            ["hoo", "rac"],
            [
                "the umbrella is hanging on the hook next to my coat.",
                "the umbrella is hanging on the rack in the entryway.",
            ],
        ),
        (
            "one shoelace is caught under the",
            ["shoe", "bag"],
            ["sho", "ba"],
            [
                "one shoelace is caught under the shoe so i need a moment.",
                "one shoelace is caught under the bag i left by the door.",
            ],
        ),
        (
            "i can wait in the",
            ["entryway", "hallway"],
            ["entryw", "hallwa"],
            [
                "i can wait in the entryway while you find your keys.",
                "i can wait in the hallway until everyone is ready.",
            ],
        ),
        (
            "could you pass me my",
            ["keycard", "umbrella"],
            ["keyca", "umbrell"],
            [
                "could you pass me my keycard before we leave?",
                "could you pass me my umbrella from the entryway?",
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
            "xie gui li hai you yi shuang",
            "xie gui li hai you yi shuan",
            "鞋柜里还有一双",
            [
                "鞋柜里还有一双，我帮你拿出来。",
                "鞋柜里还有一双，放在下面那层。",
            ],
        ),
        (
            "men dian pang bian xian liu kong",
            "men dian pang bian xian liu kon",
            "门垫旁边先留空",
            [
                "门垫旁边先留空，袋子我来挪开。",
                "门垫旁边先留空，等我把鞋放好。",
            ],
        ),
        (
            "xie dai wo zai ji yi xia",
            "xie dai wo zai ji yi xi",
            "鞋带我再系一下",
            [
                "鞋带我再系一下，你先把东西拿好。",
                "鞋带我再系一下，系好就一起走。",
            ],
        ),
        (
            "men jin ka xian bang wo na zhe",
            "men jin ka xian bang wo na zh",
            "门禁卡先帮我拿着",
            [
                "门禁卡先帮我拿着，我先把鞋穿好。",
                "门禁卡先帮我拿着，等会儿再递给我。",
            ],
        ),
        (
            "yao shi kou zai xiao he li",
            "yao shi kou zai xiao he l",
            "钥匙扣在小盒里",
            [
                "钥匙扣在小盒里，和备用钥匙放一起。",
                "钥匙扣在小盒里，你打开看看就能找到。",
            ],
        ),
        (
            "zhe die san xian fang san jia",
            "zhe die san xian fang san ji",
            "折叠伞先放伞架",
            [
                "折叠伞先放伞架，我把袋子收起来。",
                "折叠伞先放伞架，出门时再一起拿。",
            ],
        ),
    ] {
        for seed in [reading, partial] {
            choices("zh-Hans", seed, &[word], &sentences);
        }
        choices("zh-Hans", word, &[], &sentences);
    }
    for (reading, word) in [
        ("men jin ka", "门禁卡"),
        ("zhe die san", "折叠伞"),
        ("san jia", "伞架"),
        ("xie gui", "鞋柜"),
        ("men dian", "门垫"),
        ("yao shi kou", "钥匙扣"),
    ] {
        choices("zh-Hans", reading, &[word], &[]);
    }
    assert_eq!(fs::read(store.path()).unwrap(), registry);
    assert!(store.list().unwrap().iter().all(|p| p.enabled));
    println!("ENTRYWAY_NEW_CHOICES_OK:16");
}
