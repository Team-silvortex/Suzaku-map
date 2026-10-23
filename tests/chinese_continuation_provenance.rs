//! Recover only visible displaced prefix sources; never expand decoder paths.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    ime::{CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine},
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{self, EntryKind},
};

fn engine(seed: &str) -> XRTabletImeEngine {
    let mut ime = XRTabletImeEngine::new(EngineConfig {
        default_language: "zh-Hans".into(),
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
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}

fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= 12);
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
    index(ime, seed, CandidateKind::Literal);
}

fn primary(ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) {
    assert_eq!(index(ime, text, kind), 0);
    assert_eq!(ime.candidates()[0].score, 100.0);
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
    let again = ime.commit(CommitOptions { force: true });
    assert!(!again.ok && again.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

fn signature(ime: &XRTabletImeEngine) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for c in ime.candidates() {
        for byte in c
            .text
            .bytes()
            .chain([0])
            .chain(c.label.bytes())
            .chain([0])
            .chain(c.kind.id().bytes())
            .chain([0])
            .chain(c.source.id().bytes())
            .chain([0])
            .chain(c.score.to_bits().to_le_bytes())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

type SourceCase = (
    &'static str,
    &'static str,
    &'static str,
    [&'static str; 7],
    [&'static str; 2],
);
const CASES: &[SourceCase] = &[
    (
        "ta",
        "ta1",
        "太好了",
        ["他", "她", "他们", "太棒了", "讨论", "堂食", "太好了"],
        ["太好了，终于等到你了。", "太好了，我们一起去吧。"],
    ),
    (
        "hao",
        "hao3",
        "好久不见",
        ["好", "号", "好吃", "好厉害", "好喝", "好的", "好久不见"],
        ["好久不见，最近怎么样？", "好久不见，有空一起吃饭吧。"],
    ),
    (
        "you",
        "you3",
        "优先级",
        ["有", "又", "游泳", "优惠", "邮件", "优先", "优先级"],
        ["优先级请再确认一下。", "优先级较高的事项先处理。"],
    ),
];

#[test]
fn missing_fourth_prefix_pairs_are_reachable_without_losing_old_words_or_scores() {
    for &(seed, _, _, words, pair) in CASES {
        let ime = engine(seed);
        draft(&ime, seed);
        primary(&ime, words[0], CandidateKind::Word);
        let positions = words.map(|word| index(&ime, word, CandidateKind::Word));
        assert!(positions.windows(2).all(|v| v[0] < v[1]));
        for (word, score) in words
            .into_iter()
            .zip([100.0_f32, 92.0, 90.0, 89.0, 88.0, 86.0, 84.0])
        {
            let c = &ime.candidates()[index(&ime, word, CandidateKind::Word)];
            assert_eq!(c.label, word);
            assert_eq!(c.score.to_bits(), score.to_bits());
        }
        assert_eq!(ime.candidates().len(), 10);
        assert_eq!(
            ime.candidates()[index(&ime, seed, CandidateKind::Literal)].score,
            55.0
        );
        for text in pair {
            assert!(index(&ime, text, CandidateKind::Sentence) < PAGE_SIZE);
            commit_once(&mut engine(seed), text, CandidateKind::Sentence);
        }
    }
}

#[test]
fn thirteen_unaffected_frames_keep_real_before_signatures_and_sensitive_pages() {
    for (seed, expected) in [
        ("ni", 0x1752da7310d124d0),
        ("shi", 0x8955abf026a49752),
        ("zai", 0x87e356ae18e46609),
        ("na", 0xe9be9c02bae62726),
        ("guo", 0x3072f706fe34dc50),
        ("shijian", 0xe647d89415f63c18),
        ("shi jian", 0x5041fca88ae63128),
        ("ta'", 0xce9da5cb1b0540b8),
        ("他ta", 0x102f85f608a6c225),
        ("unknown", 0x43c0e0ff10fa097b),
        ("huan ge", 0x999081dd912ed081),
        ("haimeishu", 0x08b8a281d7806afd),
        ("na guo lai", 0xc37e4a2218d764e5),
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(signature(&ime), expected, "{seed}: {:?}", ime.candidates());
    }
}

#[test]
fn source_recovery_handles_case_legal_tones_and_trailing_horizontal_spacing() {
    for &(seed, toned, _, words, pair) in CASES {
        for spelling in [
            seed.to_owned(),
            seed.to_ascii_uppercase(),
            toned.to_owned(),
            toned.to_ascii_uppercase(),
        ] {
            for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
                let raw = format!("{spelling}{gap}");
                draft(&engine(&raw), &raw);
                primary(&engine(&raw), words[0], CandidateKind::Word);
                for text in pair {
                    commit_once(&mut engine(&raw), text, CandidateKind::Sentence);
                }
                commit_once(&mut engine(&raw), &raw, CandidateKind::Literal);
            }
        }
    }
}

#[test]
fn selected_word_remains_editable_and_uses_its_own_existing_authored_context() {
    for &(seed, _, word, _, pair) in CASES {
        let mut ime = engine(seed);
        ime.select_candidate(index(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert!(ime.snapshot().committed_text.is_empty());
        ime.seed(word);
        draft(&ime, word);
        for text in pair {
            commit_once(&mut engine(word), text, CandidateKind::Sentence);
        }
    }
}

#[test]
fn excluded_quotes_numbers_lines_tokens_and_literal_prefixes_do_not_borrow_bare_contexts() {
    for &(reading, _, _, _, pair) in CASES {
        for ending in [
            "'", "4.5", "2:30", "40", "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}",
            "\u{2029}",
        ] {
            let raw = format!("{reading}{ending}");
            let ime = engine(&raw);
            draft(&ime, &raw);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|c| pair.contains(&c.text.as_str()))
            );
            commit_once(&mut engine(&raw), &raw, CandidateKind::Literal);
        }
    }
    for seed in [
        "t a",
        "ta@example.invalid",
        "https://example.invalid/hao",
        "file_you",
        "他ta",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| CASES.iter().any(|case| case.4.contains(&c.text.as_str())))
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn failed_timed_out_and_empty_model_paths_preserve_the_complete_new_local_pool() {
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        for &(seed, _, _, _, pair) in CASES {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedProvider(error.clone()))));
            ime.seed(seed);
            draft(&ime, seed);
            for text in pair {
                index(&ime, text, CandidateKind::Sentence);
            }
            let local = ime.candidates().to_vec();
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), local);
            commit_once(&mut ime, pair[0], CandidateKind::Sentence);
        }
    }
}

#[test]
fn private_optional_homophone_pack_keeps_original_fourth_prefix_context_without_builtin_changes() {
    const FLAG: &str = "SUZAKU_CONTINUATION_SOURCE_PRIVATE_CHILD";
    if let Ok(mode) = std::env::var(FLAG) {
        assert!(["enabled", "disabled"].contains(&mode.as_str()));
        let path =
            std::env::temp_dir().join(format!("suzaku-source-test-{}-{mode}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        struct Owned(std::path::PathBuf);
        impl Drop for Owned {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let fixture = Owned(path);
        let builtin = lexicon::builtin("zh-Hans").unwrap();
        assert_eq!(builtin.readings().len(), 3390);
        let before = serde_json::to_value(builtin).unwrap();
        let pack = suzaku_map::lexicon::packs::OfflinePack::from_json(&serde_json::json!({
            "format_version":1,
            "manifest":{"id":"org.suzaku.test.homophone-source","version":"1.0.0",
                "name":"Private homophone source fixture","description":"Synthetic data only, not installed for the user.",
                "language":"zh-Hans","topics":["test"],"license":"MIT","authors":["Suzaku tests"]},
            "lexicon":{"format_version":1,"language":"zh-Hans",
                "readings":[{"reading":"na","text":"那","kind":"word"}]}
        }).to_string()).unwrap();
        let store = suzaku_map::lexicon::packs::PackStore::new(fixture.0.join("packs")).unwrap();
        store.install(pack, false).unwrap();
        if mode == "disabled" {
            store
                .set_enabled("org.suzaku.test.homophone-source", false)
                .unwrap();
        }
        let report = suzaku_map::lexicon::packs::initialize(&store).unwrap();
        assert!(report.errors.is_empty(), "{report:?}");
        assert_eq!(report.loaded.len(), usize::from(mode == "enabled"));
        let ime = engine("na");
        draft(&ime, "na");
        primary(&ime, "拿", CandidateKind::Word);
        let active = lexicon::active("zh-Hans").unwrap();
        let exact: Vec<_> = active
            .readings()
            .iter()
            .filter(|r| r.reading == "na" && r.kind == EntryKind::Word)
            .map(|r| r.text.as_str())
            .collect();
        if mode == "enabled" {
            assert_eq!(exact, ["拿", "那"]);
            assert!(index(&ime, "那", CandidateKind::Word) < PAGE_SIZE);
        } else {
            assert_eq!(exact, ["拿"]);
            assert_eq!(signature(&ime), 0xe9be9c02bae62726);
        }
        for word in ["奶茶", "耐心", "难怪", "闹钟", "耐用"] {
            index(&ime, word, CandidateKind::Word);
        }
        for text in ["闹钟已经设好了。", "闹钟需要提前十分钟。"] {
            commit_once(&mut engine("na"), text, CandidateKind::Sentence);
        }
        assert_eq!(serde_json::to_value(builtin).unwrap(), before);
        return;
    }
    for mode in ["enabled", "disabled"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "private_optional_homophone_pack_keeps_original_fourth_prefix_context_without_builtin_changes", "--nocapture", "--test-threads=1"])
            .env(FLAG, mode).env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY").env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output().unwrap();
        assert!(
            output.status.success(),
            "{mode}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "continuation-source-synthetic-error"
    }
    fn generate(&self, _: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        Vec::new()
    }
    fn generate_checked(
        &self,
        _: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.0.clone().map_or_else(|| Ok(Vec::new()), Err)
    }
}
