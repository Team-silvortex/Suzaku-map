//! Public module compatibility without discovery, model requests, or user configuration.
use std::any::TypeId;

use suzaku_map::{
    languages::{llama, llm, model as legacy_model, translation::TranslationProvider},
    model,
    prediction::{PredictionCancellation, PredictionError, PredictionInput, PredictionProvider},
};

fn assert_same_type<T: 'static, U: 'static>() {
    assert_eq!(TypeId::of::<T>(), TypeId::of::<U>());
}

#[test]
fn canonical_and_legacy_configuration_types_are_interchangeable() {
    assert_same_type::<model::ModelScope, legacy_model::ModelScope>();
    assert_same_type::<model::ModelProtocol, legacy_model::ModelProtocol>();
    assert_same_type::<model::ModelProviderConfig, legacy_model::ModelProviderConfig>();
    assert_same_type::<model::ModelProviderConfig, model::LlamaProviderConfig>();
    assert_same_type::<model::ModelProviderConfig, legacy_model::LlamaProviderConfig>();
    assert_same_type::<model::ModelProviderConfig, llama::LlamaProviderConfig>();

    let mut config = model::ModelProviderConfig {
        scope: model::ModelScope::Local,
        protocol: model::ModelProtocol::OpenAiCompatible,
        api_key_env: None,
        cloud_consent: false,
        endpoint: "http://127.0.0.1:1/v1/chat/completions".into(),
        model: "compatibility-fixture".into(),
        system_prompt: "Complete the supplied text.".into(),
        max_tokens: 64,
        temperature_tenths: 2,
        timeout_ms: 250,
        handwriting_hint: Some("fixture hint".into()),
    };
    let expected = config.clone();
    let legacy: &mut legacy_model::ModelProviderConfig = &mut config;
    let old_llama: &mut llama::LlamaProviderConfig = legacy;
    old_llama.max_tokens = 96;
    assert_eq!(config.max_tokens, 96);
    config.max_tokens = expected.max_tokens;

    let legacy_owned: legacy_model::LlamaProviderConfig = config;
    let llama_owned: llama::LlamaProviderConfig = legacy_owned;
    let canonical: model::ModelProviderConfig = llama_owned;
    assert_eq!(canonical, expected);
    assert_eq!(
        model::ModelProviderConfig::default(),
        legacy_model::ModelProviderConfig::default()
    );
    assert_eq!(
        model::ModelProviderConfig::default(),
        llama::LlamaProviderConfig::default()
    );
}

#[test]
fn provider_aliases_share_configuration_and_all_three_traits() {
    assert_same_type::<model::HttpModelProvider, legacy_model::HttpModelProvider>();
    assert_same_type::<model::HttpModelProvider, model::LlamaProvider>();
    assert_same_type::<model::HttpModelProvider, model::OpenAiCompatibleLlamaProvider>();
    assert_same_type::<model::HttpModelProvider, legacy_model::LlamaProvider>();
    assert_same_type::<model::HttpModelProvider, legacy_model::OpenAiCompatibleLlamaProvider>();
    assert_same_type::<model::HttpModelProvider, llama::LlamaProvider>();
    assert_same_type::<model::HttpModelProvider, llama::OpenAiCompatibleLlamaProvider>();

    // Construction stores configuration only; do not invoke any provider operation.
    let config = llama::LlamaProviderConfig::default();
    let provider = model::HttpModelProvider::new(config.clone());
    let legacy: &legacy_model::HttpModelProvider = &provider;
    let old_llama: &llama::LlamaProvider = legacy;
    let old_openai: &llama::OpenAiCompatibleLlamaProvider = old_llama;
    let canonical: &model::HttpModelProvider = old_openai;
    assert_eq!(canonical.config(), &config);
    assert!(std::ptr::eq(provider.config(), old_llama.config()));
    assert!(std::ptr::eq(provider.config(), legacy.config()));

    let _canonical_traits: (
        &dyn PredictionProvider,
        &dyn llm::LlmCompletionProvider,
        &dyn TranslationProvider,
    ) = (canonical, canonical, canonical);
    let _legacy_traits: (
        &dyn PredictionProvider,
        &dyn llm::LlmCompletionProvider,
        &dyn TranslationProvider,
    ) = (legacy, legacy, legacy);
    let _llama_traits: (
        &dyn PredictionProvider,
        &dyn llm::LlmCompletionProvider,
        &dyn TranslationProvider,
    ) = (old_llama, old_llama, old_llama);
}

#[test]
fn runtime_types_and_function_signatures_remain_compatible() {
    assert_same_type::<model::runtime::LocalModel, legacy_model::runtime::LocalModel>();
    assert_same_type::<model::runtime::LocalModel, llama::runtime::LocalModel>();
    assert_same_type::<model::runtime::RuntimeStatus, legacy_model::runtime::RuntimeStatus>();
    assert_same_type::<model::runtime::RuntimeStatus, llama::runtime::RuntimeStatus>();

    let local_model: llama::runtime::LocalModel = model::runtime::LocalModel {
        name: "compatibility-fixture".into(),
        family: "fixture".into(),
        size_bytes: 42,
        remote: false,
    };
    let status: legacy_model::runtime::RuntimeStatus = model::runtime::RuntimeStatus {
        endpoint: model::DEFAULT_LOCAL_ENDPOINT.into(),
        model: local_model.name.clone(),
        installed: true,
        loaded: Some(false),
        models: vec![local_model],
    };
    let old_status: &llama::runtime::RuntimeStatus = &status;
    let canonical: &model::runtime::RuntimeStatus = old_status;
    assert!(std::ptr::eq(canonical, &status));
    assert_eq!(canonical.models[0].name, "compatibility-fixture");

    type Discover =
        fn(&model::ModelProviderConfig) -> Result<model::ModelProviderConfig, PredictionError>;
    type Inspect =
        fn(&model::ModelProviderConfig) -> Result<model::runtime::RuntimeStatus, PredictionError>;
    type WarmUp = fn(&model::ModelProviderConfig) -> Result<(), PredictionError>;
    // Signature checks only: calling these functions would contact a model service.
    let _discover: [Discover; 3] = [
        model::runtime::discover,
        legacy_model::runtime::discover,
        llama::runtime::discover,
    ];
    let _inspect: [Inspect; 3] = [
        model::runtime::inspect,
        legacy_model::runtime::inspect,
        llama::runtime::inspect,
    ];
    let _warm_up: [WarmUp; 3] = [
        model::runtime::warm_up,
        legacy_model::runtime::warm_up,
        llama::runtime::warm_up,
    ];
}

#[test]
fn legacy_plugin_entry_points_and_constants_remain_available() {
    type ConfiguredPlugin = fn(model::ModelProviderConfig) -> llm::LlmLanguagePlugin;
    let _configured_plugins: [ConfiguredPlugin; 3] = [
        model::llama_english_plugin_with_config,
        legacy_model::llama_english_plugin_with_config,
        llama::llama_english_plugin_with_config,
    ];
    let _default_plugins: [fn() -> llm::LlmLanguagePlugin; 3] = [
        model::default_llama_english_plugin,
        legacy_model::default_llama_english_plugin,
        llama::default_llama_english_plugin,
    ];
    let _local_endpoint_checks: [fn(&str) -> bool; 3] = [
        model::is_local_llm_endpoint,
        legacy_model::is_local_llm_endpoint,
        llama::is_local_llm_endpoint,
    ];

    for default_model in [
        model::DEFAULT_LLAMA_MODEL,
        legacy_model::DEFAULT_MODEL,
        legacy_model::DEFAULT_LLAMA_MODEL,
        llama::DEFAULT_LLAMA_MODEL,
    ] {
        assert_eq!(default_model, model::DEFAULT_MODEL);
    }
    for endpoint in [
        model::DEFAULT_LLAMA_ENDPOINT,
        legacy_model::DEFAULT_LOCAL_ENDPOINT,
        legacy_model::DEFAULT_LLAMA_ENDPOINT,
        llama::DEFAULT_LLAMA_ENDPOINT,
    ] {
        assert_eq!(endpoint, model::DEFAULT_LOCAL_ENDPOINT);
    }
    assert_eq!(
        model::DEFAULT_IME_SYSTEM_PROMPT,
        legacy_model::DEFAULT_IME_SYSTEM_PROMPT
    );
    assert_eq!(
        model::DEFAULT_IME_SYSTEM_PROMPT,
        llama::DEFAULT_IME_SYSTEM_PROMPT
    );
    assert_eq!(
        model::runtime::LOCAL_ENDPOINTS,
        legacy_model::runtime::LOCAL_ENDPOINTS
    );
    assert_eq!(
        model::runtime::LOCAL_ENDPOINTS,
        llama::runtime::LOCAL_ENDPOINTS
    );
}

#[test]
fn legacy_llm_protocol_aliases_preserve_identity_and_shared_cancellation() {
    assert_same_type::<PredictionInput, llm::LlmCompletionRequest>();
    assert_same_type::<PredictionError, llm::LlmProviderError>();
    assert_same_type::<PredictionCancellation, llm::LlmCancellation>();

    let input = PredictionInput {
        language_id: "en".into(),
        seed_text: "hel".into(),
        normalized_phrase: "hel".into(),
        context_before_cursor: String::new(),
        confidence: 1.0,
        degraded: false,
    };
    let legacy: &llm::LlmCompletionRequest = &input;
    let canonical: &PredictionInput = legacy;
    assert!(std::ptr::eq(canonical, &input));

    let cancellation = PredictionCancellation::default();
    let legacy_cancellation: llm::LlmCancellation = cancellation.clone();
    let initial: Result<(), llm::LlmProviderError> = legacy_cancellation.check();
    assert_eq!(initial, Ok(()));
    legacy_cancellation.cancel();
    assert!(cancellation.is_cancelled());
    assert_eq!(cancellation.check(), Err(PredictionError::Cancelled));
}
