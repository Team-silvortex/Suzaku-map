//! Shared guards for new daily-dialogue suites; no test harness or production hooks.
use ring::digest::{SHA256, digest};
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
    lexicon::packs::{self, PackStore},
};

pub type EnglishCase = (
    &'static str,
    [&'static str; 2],
    [&'static str; 2],
    [&'static str; 2],
);
pub type ReadingCase = (&'static str, &'static str, &'static str, [&'static str; 2]);

pub struct Suite {
    pub marker: &'static str,
}

impl Suite {
    pub fn engine(&self, language: &str, seed: &str) -> XRTabletImeEngine {
        let mut ime = XRTabletImeEngine::new(EngineConfig {
            default_language: language.into(),
            ..Default::default()
        });
        ime.enable_ibus_candidate_mix();
        ime.seed(seed);
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        assert!(ime.candidates().len() <= PAGE_SIZE * 2);
        assert!(ime.candidates().iter().all(|c| {
            c.source == CandidateSource::Local && c.label == c.text && c.score.is_finite()
        }));
        assert_eq!(
            ime.candidates().len(),
            ime.candidates()
                .iter()
                .map(|c| &c.text)
                .collect::<HashSet<_>>()
                .len()
        );
        if !seed.is_empty() {
            assert!(ime.candidates().iter().any(|c| c.text == seed));
        }
        ime
    }

    pub fn first_page(&self, ime: &XRTabletImeEngine, text: &str, kind: CandidateKind) -> usize {
        ime.candidates()
            .iter()
            .take(PAGE_SIZE)
            .position(|c| c.text == text && c.kind == kind)
            .unwrap_or_else(|| {
                panic!(
                    "missing first-page {kind:?} {text:?}: {:?}",
                    ime.candidates()
                )
            })
    }

    pub fn choices(&self, language: &str, seed: &str, word: &str, authors: &[&str]) {
        let ime = self.engine(language, seed);
        if seed != word {
            self.first_page(&ime, word, CandidateKind::Word);
        }
        for author in authors {
            self.first_page(&ime, author, CandidateKind::Sentence);
        }
    }

    pub fn commit(&self, language: &str, seed: &str, text: &str) {
        let mut ime = self.engine(language, seed);
        let pos = ime
            .candidates()
            .iter()
            .position(|c| c.text == text)
            .unwrap();
        ime.select_candidate(pos);
        assert_eq!(ime.snapshot().seed_text, seed);
        assert_eq!(ime.selected_completion_text(true), Some(text));
        assert!(ime.snapshot().committed_text.is_empty());
        let result = ime.commit(CommitOptions { force: true });
        assert!(result.ok);
        assert_eq!(result.text.as_deref(), Some(text));
        assert_eq!(ime.snapshot().committed_text, text);
        assert!(ime.snapshot().seed_text.is_empty() && ime.candidates().is_empty());
        let again = ime.commit(CommitOptions { force: true });
        assert!(!again.ok && again.text.is_none());
        assert!(ime.undo().unwrap().committed_text.is_empty());
        assert!(ime.undo().is_none());
    }

    pub fn adopt(&self, language: &str, seed: &str, word: &str, authors: &[&str]) {
        let mut ime = self.engine(language, seed);
        ime.select_candidate(self.first_page(&ime, word, CandidateKind::Word));
        assert_eq!(ime.selected_completion_text(true), Some(word));
        assert_eq!(ime.snapshot().seed_text, seed);
        assert!(ime.snapshot().committed_text.is_empty());
        ime.seed(word); // Explicit UI adoption/reseed, not an implicit commit.
        for author in authors {
            self.first_page(&ime, author, CandidateKind::Sentence);
        }
        assert_eq!(ime.snapshot().seed_text, word);
        assert!(ime.snapshot().committed_text.is_empty());
    }

    pub fn preserve_resource(&self, raw: &str, first: &str, after: &str, expected_sha: &str) {
        let mut previous = raw.to_owned();
        if previous.contains(first) {
            assert_eq!(previous.matches(first).count(), 1);
            assert_eq!(previous.matches(after).count(), 1);
            let (start, end) = (previous.find(first).unwrap(), previous.find(after).unwrap());
            assert!(start < end);
            previous.replace_range(start..end, "");
        }
        let hash = digest(&SHA256, previous.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected_sha);
    }

    pub fn missing_models(&self, language: &str, seed: &str, word: &str, authors: &[&str]) {
        self.choices(language, seed, word, authors);
        let baseline = self.engine(language, seed).candidates().to_vec();
        for error in [
            Some(LlmProviderError::NoLocalModel),
            Some(LlmProviderError::Unavailable),
            Some(LlmProviderError::Timeout),
            Some(LlmProviderError::HttpStatus(503)),
            Some(LlmProviderError::InvalidResponse),
            None,
        ] {
            let mut ime = self.engine(language, "");
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
            ime.configure_prediction(Some(Arc::new(FailedModel(error.clone()))));
            ime.seed(seed);
            assert_eq!(ime.prediction_status(), PredictionStatus::Pending);
            assert_eq!(ime.candidates(), baseline);
            let deadline = Instant::now() + Duration::from_secs(2);
            while ime.prediction_pending() && Instant::now() < deadline {
                ime.poll_prediction();
                assert_eq!(ime.candidates(), baseline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(!ime.prediction_pending());
            assert_eq!(ime.prediction_status(), PredictionStatus::Unavailable);
            assert_eq!(
                ime.prediction_error(),
                Some(&error.clone().unwrap_or(LlmProviderError::NoCandidates))
            );
            assert_eq!(ime.candidates(), baseline);
            ime.configure_prediction(None);
            assert_eq!(ime.prediction_status(), PredictionStatus::Disabled);
        }
    }

    pub fn private_packs(
        &self,
        test_name: &str,
        child_env: &str,
        old_controls: impl Fn(&str),
        new_choices: impl Fn(&str),
    ) {
        if let Ok(language) = std::env::var(child_env) {
            assert!(matches!(language.as_str(), "en" | "zh-Hans" | "ja"));
            let directory = std::env::temp_dir().join(format!(
                "suzaku-{}-packs-{}-{}",
                self.marker,
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
            let prefix = if language == "zh-Hans" {
                "zh-hans"
            } else {
                &language
            };
            let mut ids = [
                "outdoors", "study", "cooking", "travel", "work", "family", "shopping", "internet",
            ]
            .map(|topic| format!("org.suzaku.{prefix}.{topic}"))
            .into_iter()
            .collect::<HashSet<_>>();
            if language == "ja" {
                ids.insert("org.suzaku.ja.rail".into());
            }
            let selected: Vec<_> = packs::recommended()
                .into_iter()
                .filter(|p| p.manifest.language == language)
                .collect();
            assert_eq!(
                selected
                    .iter()
                    .map(|p| p.manifest.id.clone())
                    .collect::<HashSet<_>>(),
                ids
            );
            for pack in selected {
                store.install(pack, false).unwrap();
            }
            let installed = store.list().unwrap();
            let expected = if language == "ja" { 9 } else { 8 };
            assert_eq!(installed.len(), expected);
            assert!(installed.iter().all(|p| p.enabled));
            assert_eq!(
                installed
                    .iter()
                    .map(|p| p.package.manifest.id.clone())
                    .collect::<HashSet<_>>(),
                ids
            );
            let registry = fs::read(store.path()).unwrap();
            let report = packs::initialize(&store).unwrap();
            assert_eq!(report.loaded.len(), expected);
            assert_eq!(report.loaded.iter().cloned().collect::<HashSet<_>>(), ids);
            assert!(report.errors.is_empty() && report.disabled.is_empty());
            old_controls(&language);
            println!("{}_OLD_CONTROLS_OK:{language}:{expected}", self.marker);
            new_choices(&language);
            assert_eq!(fs::read(store.path()).unwrap(), registry);
            assert!(store.list().unwrap().iter().all(|p| p.enabled));
            println!("{}_NEW_CHOICES_OK:{language}:{expected}", self.marker);
            return;
        }
        for language in ["en", "zh-Hans", "ja"] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
                .env(child_env, language)
                .env_remove("SUZAKU_LEXICON_DIR")
                .env_remove("SUZAKU_IME_CONFIG")
                .env_remove("SUZAKU_LINUX_IME_SOCKET")
                .env_remove("DISPLAY")
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("DBUS_SESSION_BUS_ADDRESS")
                .env_remove("IBUS_ADDRESS")
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(text.contains("running 1 test"), "{text}");
            assert!(
                text.contains(&format!("{}_OLD_CONTROLS_OK:{language}:", self.marker)),
                "{text}"
            );
            assert!(
                output.status.success(),
                "{text}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                text.contains(&format!("{}_NEW_CHOICES_OK:{language}:", self.marker)),
                "{text}"
            );
            assert!(
                text.contains("test result: ok. 1 passed; 0 failed; 0 ignored"),
                "{text}"
            );
        }
    }
}

struct FailedModel(Option<LlmProviderError>);
impl LlmCompletionProvider for FailedModel {
    fn provider_id(&self) -> &str {
        "daily-dialogue-synthetic-fallback"
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

struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
