//! Optional homophones must not hide the visible original branch's authored sources.
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource},
    },
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{
        self,
        packs::{OfflinePack, PackStore},
    },
};

const OLD_WORDS: [&str; 5] = [
    "拿过来",
    "拿国来不及了",
    "拿国来得及吗",
    "拿国来得及赶过去吗",
    "拿过来不及了",
];
const OLD_SENTENCES: [&str; 6] = [
    "拿过来吧，我帮你看看。",
    "拿过来放在这里就好。",
    "拿国来不及了，我们改天再约吧。",
    "拿国来不及了，你们先去吧。",
    "拿国来得及吗？我们现在就准备出门。",
    "拿国来得及吗？不行就换个时间。",
];

struct PrivatePack(std::path::PathBuf);
impl Drop for PrivatePack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn isolated(name: &str, enabled: bool) -> Option<PrivatePack> {
    const FLAG: &str = "SUZAKU_COMPOUND_SOURCES_PRIVATE_CHILD";
    let mode = format!("{name}:{enabled}");
    if std::env::var(FLAG).ok().as_deref() != Some(&mode) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(FLAG, &mode)
            .env_remove("SUZAKU_LEXICON_DIR")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    let dir = std::env::temp_dir().join(format!("suzaku-compound-sources-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let guard = PrivatePack(dir);
    let before = lexicon::builtin("zh-Hans").unwrap();
    assert_eq!(before.readings().len(), 3330);
    let before = serde_json::to_value(before).unwrap();
    let pack = OfflinePack::from_json(&serde_json::json!({
        "format_version":1,
        "manifest":{"id":"org.suzaku.test.compound-sources","version":"1.0.0",
            "name":"Private compound source fixture","description":"Synthetic test data, never installed for the user.",
            "language":"zh-Hans","topics":["test"],"license":"MIT","authors":["Suzaku tests"]},
        "lexicon":{"format_version":1,"language":"zh-Hans",
            "readings":[{"reading":"na","text":"那","kind":"word"}]}
    }).to_string()).unwrap();
    let store = PackStore::new(guard.0.join("packs")).unwrap();
    store.install(pack, false).unwrap();
    if !enabled {
        store
            .set_enabled("org.suzaku.test.compound-sources", false)
            .unwrap();
    }
    let report = lexicon::packs::initialize(&store).unwrap();
    assert!(report.errors.is_empty(), "{report:?}");
    assert_eq!(report.loaded.len(), usize::from(enabled));
    assert_eq!(
        lexicon::active("zh-Hans").unwrap().readings().len(),
        3330 + usize::from(enabled)
    );
    assert_eq!(
        serde_json::to_value(lexicon::builtin("zh-Hans").unwrap()).unwrap(),
        before
    );
    Some(guard)
}

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

fn draft(ime: &XRTabletImeEngine, raw: &str) {
    assert_eq!(ime.snapshot().seed_text, raw);
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
    index(ime, raw, CandidateKind::Literal);
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

#[test]
fn disabled_pack_keeps_thirteen_real_complete_vectors() {
    let Some(_guard) = isolated("disabled_pack_keeps_thirteen_real_complete_vectors", false) else {
        return;
    };
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
fn enabled_compound_keeps_all_eleven_old_choices_and_the_literal_without_raising_limits() {
    let Some(_guard) = isolated(
        "enabled_compound_keeps_all_eleven_old_choices_and_the_literal_without_raising_limits",
        true,
    ) else {
        return;
    };
    for seed in [
        "na guo lai",
        "naguolai",
        "na'guo'lai",
        "NA GUO LAI",
        "na2 guo4 lai2",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert_eq!(index(&ime, "拿过来", CandidateKind::Word), 0);
        assert_eq!(ime.candidates()[0].score, 100.0);
        assert_eq!(ime.candidates().len(), 12);
        let positions = OLD_WORDS.map(|word| index(&ime, word, CandidateKind::Word));
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
        for word in OLD_WORDS {
            commit_once(&mut engine(seed), word, CandidateKind::Word);
        }
        for text in OLD_SENTENCES {
            commit_once(&mut engine(seed), text, CandidateKind::Sentence);
        }
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
}

#[test]
fn enabled_demonstrative_combinations_preserve_the_original_primary_in_four_spellings() {
    let Some(_guard) = isolated(
        "enabled_demonstrative_combinations_preserve_the_original_primary_in_four_spellings",
        true,
    ) else {
        return;
    };
    for (reading, before, new) in [
        ("na'ben'shu", "拿本书", "那本书"),
        ("wo'yao'na'ben'shu", "我要拿本书", "我要那本书"),
        ("wo'na", "我拿", "我那"),
    ] {
        for raw in [
            reading.to_owned(),
            reading.replace('\'', ""),
            reading.replace('\'', " "),
            reading.replace('\'', " ").to_ascii_uppercase(),
        ] {
            let ime = engine(&raw);
            draft(&ime, &raw);
            assert_eq!(index(&ime, before, CandidateKind::Sentence), 0);
            commit_once(&mut engine(&raw), before, CandidateKind::Sentence);
            commit_once(&mut engine(&raw), new, CandidateKind::Sentence);
        }
    }
}

#[test]
fn adopted_demonstrative_keeps_horizontal_gaps_and_remains_editable_until_commit() {
    let Some(_guard) = isolated(
        "adopted_demonstrative_keeps_horizontal_gaps_and_remains_editable_until_commit",
        true,
    ) else {
        return;
    };
    let mut adopted = engine("na");
    adopted.select_candidate(index(&adopted, "那", CandidateKind::Word));
    assert_eq!(adopted.selected_completion_text(true), Some("那"));
    assert!(adopted.snapshot().committed_text.is_empty());
    for gap in ["", " ", "  ", "\t", "\u{a0}", "\u{3000}", "\u{202f}"] {
        let raw = format!("那{gap}ben shu");
        let text = format!("那{gap}本书");
        adopted.seed(&raw);
        draft(&adopted, &raw);
        adopted.select_candidate(index(&adopted, &text, CandidateKind::Sentence));
        assert_eq!(adopted.selected_completion_text(true), Some(text.as_str()));
        assert!(adopted.snapshot().committed_text.is_empty());
        commit_once(&mut engine(&raw), &text, CandidateKind::Sentence);
        let raw = format!("{text}{gap}gei ni");
        let text = format!("{text}{gap}给你");
        commit_once(&mut engine(&raw), &text, CandidateKind::Sentence);
    }
    for prefix in ["已确认 API。".to_owned(), "前文 café 😀。  ".repeat(40)] {
        let raw = format!("{prefix}na ben shu");
        let ime = engine(&raw);
        draft(&ime, &raw);
        assert!(ime.candidates().iter().all(|c| c.text.starts_with(&prefix)));
        commit_once(
            &mut engine(&raw),
            &format!("{prefix}那本书"),
            CandidateKind::Sentence,
        );
    }
}

#[test]
fn quotes_numbers_wrong_splits_protected_tokens_and_lines_remain_literal_safe() {
    let Some(_guard) = isolated(
        "quotes_numbers_wrong_splits_protected_tokens_and_lines_remain_literal_safe",
        true,
    ) else {
        return;
    };
    for seed in [
        "na'",
        "na4.5",
        "na2:30",
        "na40",
        "n a",
        "https://example.invalid/na",
        "na@example.invalid",
        "file_na",
    ] {
        let ime = engine(seed);
        draft(&ime, seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| OLD_SENTENCES.contains(&c.text.as_str()))
        );
        commit_once(&mut engine(seed), seed, CandidateKind::Literal);
    }
    for line in [
        "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let seed = format!("na{line}ben shu");
        let ime = engine(&seed);
        draft(&ime, &seed);
        assert!(
            !ime.candidates()
                .iter()
                .any(|c| c.text == "那本书" || c.text == "拿本书")
        );
        commit_once(&mut engine(&seed), &seed, CandidateKind::Literal);
    }
}

#[test]
fn failed_timed_out_and_empty_models_keep_the_compound_source_pair_immediately_and_after_poll() {
    let Some(_guard) = isolated(
        "failed_timed_out_and_empty_models_keep_the_compound_source_pair_immediately_and_after_poll",
        true,
    ) else {
        return;
    };
    for error in [
        Some(LlmProviderError::Unavailable),
        Some(LlmProviderError::Timeout),
        None,
    ] {
        let mut ime = engine("");
        ime.configure_prediction(Some(Arc::new(FailedProvider(error.clone()))));
        ime.seed("na guo lai");
        draft(&ime, "na guo lai");
        for text in OLD_SENTENCES {
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
        commit_once(&mut ime, OLD_SENTENCES[4], CandidateKind::Sentence);
    }
}

struct FailedProvider(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedProvider {
    fn provider_id(&self) -> &str {
        "compound-source-synthetic-error"
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
