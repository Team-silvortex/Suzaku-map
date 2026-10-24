//! Actual compound tails, not only bare syllables, keep literal boundaries.
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource},
    },
    lexicon::packs::{OfflinePack, PackStore},
};

#[test]
fn optional_compound_sources_do_not_erase_quotes_numbers_tokens_or_lines() {
    const NAME: &str = "optional_compound_sources_do_not_erase_quotes_numbers_tokens_or_lines";
    const FLAG: &str = "SUZAKU_COMPOUND_BOUNDARY_PRIVATE_CHILD";
    const DONE: &str = "SUZAKU_COMPOUND_BOUNDARY_COMPLETED";
    if std::env::var(FLAG).ok().as_deref() != Some("enabled") {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env(FLAG, "enabled")
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(stdout.matches(DONE).count(), 1);
        return;
    }
    let dir = std::env::temp_dir().join(format!("suzaku-compound-boundary-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let owned = Owned(dir);
    let pack = OfflinePack::from_json(&serde_json::json!({
        "format_version":1,
        "manifest":{"id":"org.suzaku.test.compound-boundary","version":"1.0.0",
            "name":"Private compound boundary fixture","description":"Task-local synthetic data only.",
            "language":"zh-Hans","topics":["test"],"license":"MIT","authors":["Suzaku tests"]},
        "lexicon":{"format_version":1,"language":"zh-Hans",
            "readings":[{"reading":"na","text":"那","kind":"word"}]}
    }).to_string()).unwrap();
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    store.install(pack, false).unwrap();
    let report = suzaku_map::lexicon::packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty());
    assert_eq!(report.loaded.len(), 1);
    assert_eq!(
        suzaku_map::lexicon::active("zh-Hans")
            .unwrap()
            .readings()
            .len(),
        3442
    );
    let mut seeds: Vec<String> = [
        "na guo lai'",
        "na guo lai4.5",
        "na guo lai2:30",
        "na guo lai40",
        "https://example.invalid/na-guo-lai",
        "na_guo_lai",
        "na.guolai@example.invalid",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        seeds.push(format!("na guo lai{line}"));
        seeds.push(format!("na guo{line}lai"));
    }
    for raw in seeds {
        let mut ime = XRTabletImeEngine::new(EngineConfig {
            default_language: "zh-Hans".into(),
            ..Default::default()
        });
        ime.enable_ibus_candidate_mix();
        ime.seed(&raw);
        assert_eq!(ime.snapshot().seed_text, raw);
        assert!(ime.snapshot().committed_text.is_empty());
        assert!(ime.candidates().len() <= 12);
        assert!(
            ime.candidates()
                .iter()
                .all(|c| c.source == CandidateSource::Local)
        );
        for forbidden in [
            "拿过来吧，我帮你看看。",
            "拿过来放在这里就好。",
            "拿国来不及了，我们改天再约吧。",
            "拿国来不及了，你们先去吧。",
            "拿国来得及吗？我们现在就准备出门。",
            "拿国来得及吗？不行就换个时间。",
        ] {
            assert!(
                !ime.candidates().iter().any(|c| c.text == forbidden),
                "{raw:?}: {forbidden}"
            );
        }
        let index = ime
            .candidates()
            .iter()
            .position(|c| c.text == raw && c.kind == CandidateKind::Literal)
            .unwrap();
        ime.select_candidate(index);
        let committed = ime.commit(CommitOptions { force: true });
        assert!(committed.ok);
        assert_eq!(committed.text.as_deref(), Some(raw.as_str()));
        assert!(ime.candidates().is_empty());
        let again = ime.commit(CommitOptions { force: true });
        assert!(!again.ok && again.text.is_none());
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert!(ime.undo().is_none());
    }
    println!("{DONE}");
}
