//! A unique nearly complete optional parent retains its own authored first page.
//! The actual mixed list, not a mocked helper or a general nasal interpretation,
//! is exercised in private zero/one/two/nine-pack processes.
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        CommitOptions, EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource, PAGE_SIZE},
    },
    languages::llm::{
        LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmProviderError,
    },
    lexicon::{
        self, EntryKind,
        packs::{self, PackStore},
    },
};

const CHILD: &str = "SUZAKU_JA_SHORT_NASAL_PARENT_CHILD";
const FAMILY: &str = "org.suzaku.ja.family";
const RAIL: &str = "org.suzaku.ja.rail";
const PARENT: [&str; 2] = ["玄関で待っています。", "玄関の靴をそろえてください。"];
const COMPOUND: [&str; 2] = [
    "玄関マットは靴箱の前にあります。",
    "玄関マットの横に置いた荷物を取りました。",
];

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalogs(name: &str, modes: &[&str], body: impl FnOnce(&str)) {
    let Ok(mode) = std::env::var(CHILD) else {
        let results = modes
            .iter()
            .map(|mode| {
                let output = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", name, "--nocapture", "--test-threads=1"])
                    .env(CHILD, mode)
                    .env_remove("SUZAKU_LEXICON_DIR")
                    .env_remove("SUZAKU_IME_CONFIG")
                    .env_remove("SUZAKU_LINUX_IME_SOCKET")
                    .env_remove("DISPLAY")
                    .env_remove("WAYLAND_DISPLAY")
                    .env_remove("DBUS_SESSION_BUS_ADDRESS")
                    .output()
                    .unwrap();
                (*mode, output)
            })
            .collect::<Vec<_>>();
        let details = results
            .iter()
            .map(|(mode, output)| {
                format!(
                    "{mode}: {}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            results.iter().all(|(_, output)| output.status.success()),
            "{details}"
        );
        for (mode, output) in results {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
                "{stdout}"
            );
            assert!(
                stdout.contains(&format!("completed nasal parent {name} {mode}")),
                "{stdout}"
            );
        }
        return;
    };
    assert!(modes.contains(&mode.as_str()));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let owned = Owned(std::env::temp_dir().join(format!(
        "suzaku-ja-short-nasal-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&owned.0).unwrap();
    let store = PackStore::new(owned.0.join("packs")).unwrap();
    assert!(store.list().unwrap().is_empty());
    let selected = packs::recommended()
        .into_iter()
        .filter(|pack| {
            pack.manifest.language == "ja"
                && match mode.as_str() {
                    "builtin" => false,
                    "family" => pack.manifest.id == FAMILY,
                    "family-rail" => matches!(pack.manifest.id.as_str(), FAMILY | RAIL),
                    "all" => true,
                    _ => unreachable!(),
                }
        })
        .collect::<Vec<_>>();
    let expected = match mode.as_str() {
        "builtin" => 0,
        "family" => 1,
        "family-rail" => 2,
        "all" => 9,
        _ => unreachable!(),
    };
    let ids = selected
        .iter()
        .map(|pack| pack.manifest.id.clone())
        .collect::<HashSet<_>>();
    assert_eq!((selected.len(), ids.len()), (expected, expected));
    for pack in selected {
        store.install(pack, false).unwrap();
    }
    let report = packs::initialize(&store).unwrap();
    assert!(
        report.errors.is_empty() && report.disabled.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
    let active = lexicon::active("ja").unwrap();
    assert!(
        active
            .readings()
            .iter()
            .any(|entry| entry.reading == "げんかんまっと"
                && entry.text == "玄関マット"
                && entry.kind == EntryKind::Word)
    );
    assert_eq!(
        active
            .readings()
            .iter()
            .any(|entry| entry.reading == "げんかん"
                && entry.text == "玄関"
                && entry.kind == EntryKind::Word),
        mode != "builtin"
    );
    println!("NASAL_PARENT_SOURCE_CONTROLS_OK {mode}");
    body(&mode);
    println!("completed nasal parent {name} {mode}");
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
        .position(|candidate| candidate.text == text && candidate.kind == kind)
        .unwrap_or_else(|| panic!("missing {text:?} {kind:?}: {:?}", ime.candidates()))
}
fn draft(ime: &XRTabletImeEngine, seed: &str) {
    assert_eq!(ime.snapshot().seed_text, seed);
    assert!(ime.snapshot().committed_text.is_empty());
    assert!(ime.candidates().len() <= PAGE_SIZE * 2);
    assert!(
        ime.candidates()
            .iter()
            .all(|candidate| candidate.source == CandidateSource::Local
                && candidate.text == candidate.label
                && candidate.score.is_finite()
                && (0.0..=100.0).contains(&candidate.score))
    );
    assert_eq!(
        ime.candidates().len(),
        ime.candidates()
            .iter()
            .map(|candidate| &candidate.text)
            .collect::<HashSet<_>>()
            .len()
    );
    assert!(
        ime.candidates()
            .iter()
            .any(|candidate| candidate.text == seed)
    );
}
fn first_pair(ime: &XRTabletImeEngine, pair: [&str; 2]) {
    for text in pair {
        assert!(
            index(ime, text, CandidateKind::Sentence) < PAGE_SIZE,
            "{text}: {:?}",
            ime.candidates()
        );
    }
}
fn commit_once(seed: &str, text: &str, kind: CandidateKind) {
    let mut ime = engine(seed);
    ime.select_candidate(index(&ime, text, kind));
    assert_eq!(ime.selected_completion_text(true), Some(text));
    assert!(ime.snapshot().committed_text.is_empty());
    let result = ime.commit(CommitOptions { force: true });
    assert!(result.ok);
    assert_eq!(result.text.as_deref(), Some(text));
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.candidates().is_empty());
    let second = ime.commit(CommitOptions { force: true });
    assert!(!second.ok && second.text.is_none());
    assert_eq!(ime.snapshot().committed_text, text);
    assert!(ime.undo().unwrap().committed_text.is_empty());
    assert!(ime.undo().is_none());
}

#[test]
fn short_nasal_parent_retains_actual_typing_anchors_word_order_scores_and_own_page() {
    const NAME: &str =
        "short_nasal_parent_retains_actual_typing_anchors_word_order_scores_and_own_page";
    catalogs(NAME, &["family-rail"], |_| {
        let ime = engine("genka");
        draft(&ime, "genka");
        for (at, text, score) in [(0, "げんか", 100.0_f32), (1, "ゲンカ", 92.0_f32)] {
            let row = &ime.candidates()[at];
            assert_eq!(
                (&*row.text, row.kind, row.source, row.score.to_bits()),
                (
                    text,
                    CandidateKind::Unspecified,
                    CandidateSource::Local,
                    score.to_bits()
                )
            );
        }
        let words = ime
            .candidates()
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Word)
            .map(|candidate| (candidate.text.as_str(), candidate.score.to_bits()))
            .collect::<Vec<_>>();
        assert_eq!(
            words,
            [
                ("玄関マット", 91.0_f32.to_bits()),
                ("玄関", 90.5_f32.to_bits())
            ]
        );
        for (text, score) in [
            (PARENT[0], 77.0_f32),
            (PARENT[1], 76.5_f32),
            (COMPOUND[0], 78.0_f32),
            (COMPOUND[1], 77.5_f32),
        ] {
            assert_eq!(
                ime.candidates()[index(&ime, text, CandidateKind::Sentence)]
                    .score
                    .to_bits(),
                score.to_bits()
            );
        }
        assert_eq!(
            ime.candidates()[index(&ime, "genka", CandidateKind::Literal)]
                .score
                .to_bits(),
            55.0_f32.to_bits()
        );
        first_pair(&ime, PARENT);
        for text in PARENT {
            commit_once("genka", text, CandidateKind::Sentence);
        }
        commit_once("genka", "玄関", CandidateKind::Word);
        commit_once("genka", "玄関マット", CandidateKind::Word);
        commit_once("genka", "genka", CandidateKind::Literal);
    });
}

#[test]
fn short_nasal_parent_private_one_and_nine_packs_preserve_variants_and_compound_progress() {
    const NAME: &str =
        "short_nasal_parent_private_one_and_nine_packs_preserve_variants_and_compound_progress";
    catalogs(NAME, &["family", "all"], |_| {
        for seed in [
            "genka",
            "GENKA",
            "げんか",
            "gen ka",
            "gen\tka",
            "gen\u{3000}ka",
        ] {
            let ime = engine(seed);
            draft(&ime, seed);
            first_pair(&ime, PARENT);
            assert!(
                index(&ime, "玄関マット", CandidateKind::Word)
                    < index(&ime, "玄関", CandidateKind::Word)
            );
        }
        for seed in ["genkanmatt", "genkanmatto", "げんかんまっと"] {
            let ime = engine(seed);
            draft(&ime, seed);
            first_pair(&ime, COMPOUND);
            for text in COMPOUND {
                commit_once(seed, text, CandidateKind::Sentence);
            }
        }
    });
}

#[test]
fn short_nasal_parent_requires_real_parent_source_and_does_not_join_lines_or_identifiers() {
    const NAME: &str =
        "short_nasal_parent_requires_real_parent_source_and_does_not_join_lines_or_identifiers";
    catalogs(NAME, &["builtin", "family", "all"], |mode| {
        if mode == "builtin" {
            let ime = engine("genka");
            draft(&ime, "genka");
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| candidate.text == "玄関")
            );
            first_pair(&ime, COMPOUND);
        }
        let mut seeds = [
            "ゲンカ",
            "genkaxyz",
            "xgenka",
            "https://example.invalid/genka",
            "user+genka@example.invalid",
            "genka\u{200b}",
            "ｇｅｎｋａ",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        for line in [
            "\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
        ] {
            seeds.push(format!("gen{line}ka"));
        }
        let long = format!("{}genka", "前文。".repeat(90));
        assert!(long.chars().count() > 256);
        seeds.push(long);
        for seed in seeds {
            let ime = engine(&seed);
            draft(&ime, &seed);
            assert!(
                !ime.candidates()
                    .iter()
                    .any(|candidate| PARENT.contains(&candidate.text.as_str())),
                "{seed:?}: {:?}",
                ime.candidates()
            );
        }
    });
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "ja-short-nasal-parent-synthetic-failure"
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
#[test]
fn short_nasal_parent_pending_disabled_typed_failures_and_empty_models_keep_local_pool() {
    const NAME: &str =
        "short_nasal_parent_pending_disabled_typed_failures_and_empty_models_keep_local_pool";
    catalogs(NAME, &["family", "all"], |_| {
        let baseline = engine("genka").candidates().to_vec();
        let mut disabled = engine("genka");
        disabled.configure_prediction(None);
        assert_eq!(disabled.prediction_status(), PredictionStatus::Disabled);
        assert_eq!(disabled.candidates(), baseline);
        first_pair(&disabled, PARENT);
        for error in [
            Some(LlmProviderError::NoLocalModel),
            Some(LlmProviderError::Unavailable),
            Some(LlmProviderError::Timeout),
            Some(LlmProviderError::HttpStatus(503)),
            Some(LlmProviderError::InvalidResponse),
            None,
        ] {
            let mut ime = engine("");
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed("genka");
            assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
            assert!(ime.prediction_pending() && ime.prediction_error().is_none());
            assert_eq!(ime.candidates(), baseline);
            first_pair(&ime, PARENT);
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!ime.prediction_pending());
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), baseline);
            first_pair(&ime, PARENT);
        }
    });
}
