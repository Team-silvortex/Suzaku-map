//! Explicit one-request cloud QA, not a normal CI gate or a GUI/quality guarantee.
//! Configuration is in memory; no personal settings, input, history or keyring is read.
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use suzaku_map::{
    ime::{
        EngineConfig, PredictionStatus, XRTabletImeEngine,
        candidate_mix::{CandidateKind, CandidateSource},
    },
    model::{HttpModelProvider, ModelProtocol, ModelProviderConfig, ModelScope},
    prediction::{
        PredictionCancellation, PredictionCandidate, PredictionError, PredictionProvider,
        PredictionRequest, PredictionResponse,
    },
};

const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const MODELS: [&str; 2] = ["gpt-6.1-sol", "gpt-6-astra"];
const INPUT_NAMES: [&str; 6] = [
    "SUZAKU_MODEL_CLOUD_QA",
    "SUZAKU_MODEL_CLOUD_QA_ONCE",
    "SUZAKU_MODEL_CLOUD_QA_CONSENT",
    "SUZAKU_MODEL_CLOUD_QA_MODEL",
    "SUZAKU_MODEL_CLOUD_QA_LANGUAGE",
    "SUZAKU_MODEL_CLOUD_QA_KEY_ENV",
];
// Observation includes the real 120 ms debounce and bounded scheduler overhead.
// It does not extend HttpModelProvider's unchanged 5000 ms generation deadline.
const OBSERVATION_DEADLINE: Duration = Duration::from_millis(5500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Case {
    language: &'static str,
    seed: &'static str,
}

const CASES: [Case; 3] = [
    Case {
        language: "en",
        seed: "Could you sh",
    },
    Case {
        language: "zh-Hans",
        seed: "nihao",
    },
    Case {
        language: "ja",
        seed: "nihongo",
    },
];

struct Options {
    model: String,
    case: Case,
    credential_env: String,
}

impl Options {
    fn parse(mut read: impl FnMut(&str) -> Option<OsString>) -> Result<Self, &'static str> {
        let mut text = |name: &str| {
            read(name)
                .and_then(|value| value.into_string().ok())
                .ok_or("explicit QA settings are missing or non-Unicode")
        };
        for (name, required) in INPUT_NAMES[..3].iter().zip(["1", "1", "true"]) {
            if text(name)? != required {
                return Err("explicit one-request synthetic cloud consent is required");
            }
        }
        let model = text(INPUT_NAMES[3])?;
        if !MODELS.contains(&model.as_str()) {
            return Err("the requested model is outside this two-model QA authorization");
        }
        let language = text(INPUT_NAMES[4])?;
        let case = CASES
            .iter()
            .find(|case| case.language == language)
            .copied()
            .ok_or("select exactly one fixed English, Chinese or Japanese QA case")?;
        let options = Self {
            model,
            case,
            credential_env: text(INPUT_NAMES[5])?,
        };
        // Name validation only. This does not look up or format its value.
        options
            .config()
            .validate()
            .map_err(|_| "invalid explicit credential environment reference")?;
        Ok(options)
    }

    fn config(&self) -> ModelProviderConfig {
        ModelProviderConfig {
            scope: ModelScope::Cloud,
            protocol: ModelProtocol::ChatCompletionsReasoning,
            cloud_consent: true,
            endpoint: ENDPOINT.into(),
            model: self.model.clone(),
            api_key_env: Some(self.credential_env.clone()),
            timeout_ms: 5000,
            // This bounds visible output AND reasoning; it is not measured usage.
            max_tokens: 512,
            ..Default::default()
        }
    }
}

#[derive(Clone)]
struct Observation {
    elapsed_ms: u128,
    result: Result<PredictionResponse, PredictionError>,
}

/// The wrapper observes the production adapter, never substitutes a live result.
/// The atomic slot is claimed before delegation, including on error/timeout.
struct Once<P> {
    inner: P,
    case: Case,
    calls: AtomicUsize,
    observation: Mutex<Option<Observation>>,
}

impl<P> Once<P> {
    fn new(inner: P, case: Case) -> Self {
        Self {
            inner,
            case,
            calls: AtomicUsize::new(0),
            observation: Mutex::new(None),
        }
    }
}

impl<P: PredictionProvider> PredictionProvider for Once<P> {
    fn predict(
        &self,
        request: &PredictionRequest,
        cancellation: &PredictionCancellation,
    ) -> Result<PredictionResponse, PredictionError> {
        cancellation.check()?;
        if request.input.language_id != self.case.language
            || request.input.seed_text != self.case.seed
            || !request.input.context_before_cursor.is_empty()
        {
            return Err(PredictionError::InvalidRequest);
        }
        if self
            .calls
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(PredictionError::Unavailable);
        }
        let started = Instant::now();
        let result = self.inner.predict(request, cancellation);
        *self.observation.lock().unwrap() = Some(Observation {
            elapsed_ms: started.elapsed().as_millis(),
            result: result.clone(),
        });
        result
    }
}

fn rows(engine: &XRTabletImeEngine) -> Vec<Value> {
    engine
        .candidates()
        .iter()
        .map(|candidate| {
            json!({
                "text": candidate.text, "kind": candidate.kind.id(),
                "source": candidate.source.id(), "weight": candidate.score,
            })
        })
        .collect()
}

fn measure<P: PredictionProvider + 'static>(
    options: &Options,
    provider: Arc<Once<P>>,
) -> (bool, Value) {
    let mut engine = XRTabletImeEngine::new(EngineConfig {
        default_language: options.case.language.into(),
        ..Default::default()
    });
    engine.set_preferences_enabled(false);
    engine.enable_ibus_candidate_mix();
    engine.configure_prediction_provider(Some(provider.clone()));
    let started = Instant::now();
    engine.seed(options.case.seed);
    let before = rows(&engine);
    let deadline = started + OBSERVATION_DEADLINE;
    while engine.prediction_pending() && Instant::now() < deadline {
        engine.poll_prediction();
        if engine.prediction_pending() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let elapsed_ms = started.elapsed().as_millis();
    let status = engine.prediction_status();
    let snapshot = engine.snapshot();
    let after = rows(&engine);
    let model_rows = engine
        .candidates()
        .iter()
        .filter(|candidate| {
            candidate.source == CandidateSource::Model
                && matches!(
                    candidate.kind,
                    CandidateKind::Word | CandidateKind::Sentence
                )
        })
        .count();
    let observation = provider.observation.lock().unwrap().clone();
    let adapter_success = observation
        .as_ref()
        .is_some_and(|observed| observed.result.is_ok());
    let preserved = snapshot.seed_text == options.case.seed && snapshot.committed_text.is_empty();
    let passed = status == PredictionStatus::Ready
        && adapter_success
        && preserved
        && model_rows > 0
        && provider.calls.load(Ordering::SeqCst) == 1;
    let adapter = match observation {
        Some(Observation {
            elapsed_ms,
            result: Ok(response),
        }) => json!({
            "outcome": "ok", "elapsed_ms": elapsed_ms,
            "candidates": response.candidates.iter().map(|candidate| json!({
                "text": candidate.text, "kind": candidate.kind,
                "score_bias": candidate.score_bias,
            })).collect::<Vec<_>>()
        }),
        Some(Observation {
            elapsed_ms,
            result: Err(error),
        }) => json!({
            "outcome": "error", "elapsed_ms": elapsed_ms, "error": format!("{error:?}"),
        }),
        None => json!({"outcome": "not_completed", "elapsed_ms": null}),
    };
    let summary = json!({
        "format": "suzaku-cloud-quality-v1", "passed": passed,
        "model": options.model, "endpoint": ENDPOINT,
        "protocol": options.config().protocol.id(), "reasoning_effort": "low",
        "max_completion_tokens": 512, "provider_timeout_ms": 5000,
        "observation_deadline_ms": OBSERVATION_DEADLINE.as_millis(),
        "language": options.case.language, "seed": options.case.seed,
        "elapsed_ms": elapsed_ms, "prediction_status": format!("{status:?}"),
        "prediction_error": engine.prediction_error().map(|error| format!("{error:?}")),
        "adapter_calls": provider.calls.load(Ordering::SeqCst), "adapter": adapter,
        "draft_preserved_without_commit": preserved,
        "displayed_model_word_or_sentence_count": model_rows,
        "before": before, "after": after,
        "usage": null, "usage_status": "unavailable_adapter_does_not_expose_api_usage",
        "quality_scope": "one synthetic case; structural acceptance is not general language quality or GUI acceptance",
    });
    engine.cancel_prediction();
    (passed, summary)
}

#[test]
#[ignore = "explicit one-request cloud QA consent; fixed synthetic input; may incur API charges"]
fn authorized_cloud_model_single_synthetic_case() {
    let options = match Options::parse(|name| std::env::var_os(name)) {
        Ok(options) => options,
        Err(reason) => {
            println!(
                "SUZAKU_CLOUD_QUALITY_RESULT {}",
                json!({
                    "format": "suzaku-cloud-quality-v1", "passed": false,
                    "adapter_calls": 0, "stage": "authorization", "reason": reason,
                    "usage": null, "usage_status": "unavailable_no_request_performed",
                })
            );
            panic!("cloud QA authorization failed; no request was made");
        }
    };
    let provider = Arc::new(Once::new(
        HttpModelProvider::new(options.config()),
        options.case,
    ));
    let (passed, summary) = measure(&options, provider);
    println!("SUZAKU_CLOUD_QUALITY_RESULT {summary}");
    assert!(
        passed,
        "single live candidate case failed; no retry is authorized"
    );
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use std::collections::BTreeMap;
    use suzaku_map::prediction::{PredictionInput, PredictionKind};

    fn settings() -> BTreeMap<&'static str, OsString> {
        INPUT_NAMES
            .into_iter()
            .zip(["1", "1", "true", MODELS[0], "en", "SUZAKU_SYNTHETIC_QA_KEY"])
            .map(|(key, value)| (key, value.into()))
            .collect()
    }

    fn parsed(settings: &BTreeMap<&str, OsString>) -> Result<Options, &'static str> {
        Options::parse(|name| {
            assert!(
                INPUT_NAMES.contains(&name),
                "preflight tried reading a credential or unrelated setting"
            );
            settings.get(name).cloned()
        })
    }

    fn options() -> Options {
        parsed(&settings()).unwrap()
    }

    fn request(case: Case) -> PredictionRequest {
        PredictionRequest::new(
            1,
            PredictionInput {
                language_id: case.language.into(),
                seed_text: case.seed.into(),
                normalized_phrase: case.seed.into(),
                context_before_cursor: String::new(),
                confidence: 1.0,
                degraded: false,
            },
        )
    }

    struct Fixed {
        calls: Arc<AtomicUsize>,
        result: Result<&'static str, PredictionError>,
    }

    impl PredictionProvider for Fixed {
        fn predict(
            &self,
            request: &PredictionRequest,
            _: &PredictionCancellation,
        ) -> Result<PredictionResponse, PredictionError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result.clone().map(|text| {
                PredictionResponse::new(
                    request,
                    vec![PredictionCandidate {
                        text: text.into(),
                        score_bias: 0.6,
                        kind: Some(PredictionKind::Sentence),
                    }],
                )
            })
        }
    }

    #[test]
    fn parsing_is_explicit_and_never_reads_the_referenced_synthetic_credential() {
        let mut fields = settings();
        fields.insert(
            "SUZAKU_SYNTHETIC_QA_KEY",
            "SYNTHETIC_VALUE_NEVER_READ".into(),
        );
        let options = parsed(&fields).unwrap();
        let config = options.config();
        assert_eq!(config.endpoint, ENDPOINT);
        assert_eq!(config.protocol, ModelProtocol::ChatCompletionsReasoning);
        assert_eq!(config.scope, ModelScope::Cloud);
        assert!(config.cloud_consent);
        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(
            config.api_key_env.as_deref(),
            Some("SUZAKU_SYNTHETIC_QA_KEY")
        );
        for name in INPUT_NAMES {
            let mut absent = settings();
            absent.remove(name);
            assert!(parsed(&absent).is_err());
        }
    }

    #[test]
    fn invalid_opt_ins_models_languages_and_credential_references_are_not_echoed() {
        for (name, invalid) in [
            (INPUT_NAMES[0], "0"),
            (INPUT_NAMES[1], "all"),
            (INPUT_NAMES[2], "yes"),
            (INPUT_NAMES[3], "SYNTHETIC_UNAUTHORIZED_MODEL"),
            (INPUT_NAMES[4], "all"),
            (INPUT_NAMES[5], "SYNTHETIC_LITERAL_KEY=NOT_A_KEY"),
            (INPUT_NAMES[5], ""),
        ] {
            let mut fields = settings();
            fields.insert(name, invalid.into());
            let error = parsed(&fields).err().expect("invalid QA options accepted");
            assert!(invalid.is_empty() || !error.contains(invalid));
        }
        assert_eq!(
            CASES.iter().map(|case| case.language).collect::<Vec<_>>(),
            ["en", "zh-Hans", "ja"]
        );
        assert_eq!(
            CASES.iter().map(|case| case.seed).collect::<Vec<_>>(),
            ["Could you sh", "nihao", "nihongo"]
        );
        for case in CASES {
            request(case).validate().unwrap();
        }
    }

    #[test]
    fn a_claimed_provider_never_delegates_twice_even_after_failure() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Once::new(
            Fixed {
                calls: calls.clone(),
                result: Err(PredictionError::Timeout),
            },
            CASES[0],
        );
        let request = request(CASES[0]);
        let cancellation = PredictionCancellation::default();
        assert_eq!(
            provider.predict(&request, &cancellation),
            Err(PredictionError::Timeout)
        );
        assert_eq!(
            provider.predict(&request, &cancellation),
            Err(PredictionError::Unavailable)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_worker_preservation_and_mixed_candidate_chain_rejects_fallback_only_results() {
        for result in [
            Err(PredictionError::Unavailable),
            Ok("lost exact raw draft prefix"),
        ] {
            let options = options();
            let calls = Arc::new(AtomicUsize::new(0));
            let provider = Arc::new(Once::new(
                Fixed {
                    calls: calls.clone(),
                    result,
                },
                options.case,
            ));
            let (passed, summary) = measure(&options, provider);
            assert!(!passed);
            assert_eq!(summary["prediction_status"], "Unavailable");
            assert_eq!(summary["displayed_model_word_or_sentence_count"], 0);
            assert_eq!(summary["draft_preserved_without_commit"], true);
            assert!(!summary["after"].as_array().unwrap().is_empty());
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(summary["usage"].is_null());
        }
    }

    #[test]
    fn a_synthetic_provider_can_pass_the_same_real_worker_and_mixed_chain_without_network() {
        let options = options();
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(Once::new(
            Fixed {
                calls: calls.clone(),
                result: Ok("Could you share a synthetic constellation?"),
            },
            options.case,
        ));
        let (passed, summary) = measure(&options, provider);
        assert!(passed, "{summary}");
        assert_eq!(summary["adapter"]["outcome"], "ok");
        assert_eq!(summary["prediction_status"], "Ready");
        assert_eq!(summary["draft_preserved_without_commit"], true);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(summary["usage"].is_null());
    }
}
