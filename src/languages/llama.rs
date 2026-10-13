//! Compatibility exports for pre-0.5 model integrations.
//! New code should use `crate::model`; model names do not select a protocol.
pub use crate::model::{
    DEFAULT_IME_SYSTEM_PROMPT, DEFAULT_LLAMA_ENDPOINT, DEFAULT_LLAMA_MODEL, LlamaProvider,
    LlamaProviderConfig, OpenAiCompatibleLlamaProvider, default_llama_english_plugin,
    is_local_llm_endpoint, llama_english_plugin_with_config, runtime,
};
