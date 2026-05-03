pub mod anthropic;
pub mod ollama;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;

use crate::provider::AiProvider;
use dca_config::AiConfig;

/// Construye el proveedor correcto a partir de la configuración.
pub fn build_provider(cfg: &AiConfig) -> Box<dyn AiProvider> {
    match cfg.provider.as_str() {
        "anthropic" => Box::new(AnthropicProvider::new(&cfg.base_url, &cfg.api_key)),
        "openai" | "groq" | "openrouter" | "custom" => {
            Box::new(OpenAiProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model))
        }
        _ => Box::new(OllamaProvider::new(&cfg.base_url, &cfg.model)),
    }
}
