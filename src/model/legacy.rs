//! Source-compatible completion and English plugin entry points.
use super::{HttpModelProvider, LlamaProviderConfig, OpenAiCompatibleLlamaProvider};
use crate::languages::english::english_sentence_variants;
use crate::languages::llm::{
    LlmCancellation, LlmCompletion, LlmCompletionProvider, LlmCompletionRequest, LlmLanguagePlugin,
    LlmProviderError,
};
use std::sync::Arc;

impl LlmCompletionProvider for HttpModelProvider {
    fn provider_id(&self) -> &str {
        if self.uses_ollama_api() {
            "ollama"
        } else {
            "openai-compatible"
        }
    }
    fn generate(&self, request: &LlmCompletionRequest) -> Vec<LlmCompletion> {
        self.generate_checked(request).unwrap_or_default()
    }

    fn generate_checked(
        &self,
        request: &LlmCompletionRequest,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.generate_completion(request, &LlmCancellation::default())
            .map(|candidates| candidates.into_iter().map(Into::into).collect())
    }

    fn generate_cancellable(
        &self,
        request: &LlmCompletionRequest,
        cancellation: &LlmCancellation,
    ) -> Result<Vec<LlmCompletion>, LlmProviderError> {
        self.generate_completion(request, cancellation)
            .map(|candidates| candidates.into_iter().map(Into::into).collect())
    }
}

pub fn llama_english_plugin_with_config(config: LlamaProviderConfig) -> LlmLanguagePlugin {
    LlmLanguagePlugin::new(
        "llama-en",
        "Llama English",
        Arc::new(OpenAiCompatibleLlamaProvider::new(config)),
        english_sentence_variants,
    )
}
pub fn default_llama_english_plugin() -> LlmLanguagePlugin {
    llama_english_plugin_with_config(LlamaProviderConfig::default())
}
