//! Cliente LLM nativo de regista (rework v1.0).
//!
//! Este módulo reemplaza la invocación de tools CLI externas (`pi`, `claude`,
//! `codex`, `opencode`) por llamadas directas a APIs de LLM. Define el trait
//! `LlmProvider` que abstrae la comunicación con distintos backends (OpenAI,
//! Anthropic, Ollama) y la factory `from_config()` que instancia el provider
//! correcto según la configuración TOML.

#![allow(dead_code)]

pub mod anthropic;
pub mod openai;
pub mod rate_limiter;
pub mod retry;
pub mod types;

use std::time::Duration;

use crate::config::ModelConfig;
use types::{ChatResponse, Message};

/// Contrato para proveedores LLM nativos.
///
/// Cada implementación encapsula la autenticación, el formato de mensajes,
/// y el endpoint HTTP específico de una API de LLM.
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    /// Envía una solicitud de chat al modelo y devuelve la respuesta.
    ///
    /// `messages` es la historia completa de la conversación (system, user, assistant).
    /// `model` es el identificador del modelo en la API (ej: "gpt-4o").
    /// `timeout` es el tiempo máximo de espera para la request HTTP.
    fn chat(
        &self,
        messages: Vec<Message>,
        model: &str,
        timeout: Duration,
    ) -> anyhow::Result<ChatResponse>;

    /// Nombre del provider para logs y diagnóstico.
    fn provider_name(&self) -> &str;
}

/// Construye un provider LLM a partir de la configuración de modelo.
///
/// `model_name` es el nombre lógico del modelo (para mensajes de error).
/// `model_config` contiene provider, model_id, api_key, y base_url opcional.
///
/// Devuelve error si el provider es desconocido.
pub fn from_config(
    model_name: &str,
    model_config: &ModelConfig,
) -> anyhow::Result<Box<dyn LlmProvider>> {
    match model_config.provider.to_lowercase().as_str() {
        "openai" => {
            let base_url = model_config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            Ok(Box::new(openai::OpenAiProvider::new(
                model_config.api_key.clone(),
                base_url,
            )))
        }
        "anthropic" => {
            let base_url = model_config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            Ok(Box::new(anthropic::AnthropicProvider::new(
                model_config.api_key.clone(),
                base_url,
            )))
        }
        other => anyhow::bail!(
            "provider LLM desconocido: '{other}' para el modelo '{model_name}'. \
             Providers válidos: openai, anthropic"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // EPIC-V10-01 | STORY-V10-001: Definir trait LlmProvider y tipos base
    // ═══════════════════════════════════════════════════════════════

    // ── CA1: LlmProvider trait ───────────────────────────────────────

    /// CA1: El trait LlmProvider existe y tiene los métodos requeridos.
    /// Verificamos que compila creando un tipo dummy que lo implementa.
    #[test]
    fn story_v10001_ca1_trait_exists_and_can_be_implemented() {
        #[derive(Debug)]
        struct DummyProvider;

        impl LlmProvider for DummyProvider {
            fn chat(
                &self,
                _messages: Vec<Message>,
                _model: &str,
                _timeout: Duration,
            ) -> anyhow::Result<ChatResponse> {
                Ok(ChatResponse {
                    content: "dummy".to_string(),
                    finish_reason: "stop".to_string(),
                    token_usage: None,
                })
            }

            fn provider_name(&self) -> &str {
                "dummy"
            }
        }

        let provider = DummyProvider;
        let result = provider.chat(vec![], "test", Duration::from_secs(10));
        assert!(result.is_ok());
        assert_eq!(provider.provider_name(), "dummy");
    }

    /// CA1: El trait es Send + Sync + Debug (bounds verificados por compilación).
    #[test]
    fn story_v10001_ca1_trait_is_send_sync_debug() {
        // Verifica que Box<dyn LlmProvider> implementa Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn LlmProvider>>();

        // Verifica que &dyn LlmProvider implementa Debug
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<&dyn LlmProvider>();
    }

    /// CA1: La firma de chat acepta Vec<Message>, &str (model), Duration (timeout).
    #[test]
    fn story_v10001_ca1_chat_signature_matches_spec() {
        // Este test verifica que la firma coincide con la especificación.
        // La implementación real se prueba en los tests de cada provider.
        fn call_chat(provider: &dyn LlmProvider) -> anyhow::Result<ChatResponse> {
            provider.chat(
                vec![Message::system("test")],
                "gpt-4o",
                Duration::from_secs(30),
            )
        }

        // Verifica que compila
        let _ = call_chat;
    }

    // ── CA3: Factory from_config ──────────────────────────────────────

    /// CA3: from_config crea OpenAiProvider con provider="openai".
    #[test]
    fn story_v10001_ca3_from_config_creates_openai() {
        let config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            api_key: "sk-test".to_string(),
            base_url: None,
        };
        let provider = from_config("gpt4o", &config).unwrap();
        assert_eq!(provider.provider_name(), "openai");
    }

    /// CA3: from_config crea AnthropicProvider con provider="anthropic".
    #[test]
    fn story_v10001_ca3_from_config_creates_anthropic() {
        let config = ModelConfig {
            provider: "anthropic".to_string(),
            model_id: "claude-sonnet-4".to_string(),
            api_key: "sk-ant-test".to_string(),
            base_url: None,
        };
        let provider = from_config("claude", &config).unwrap();
        assert_eq!(provider.provider_name(), "anthropic");
    }

    /// CA3: from_config usa base_url personalizado si se especifica.
    #[test]
    fn story_v10001_ca3_from_config_uses_custom_base_url() {
        // Para OpenAI
        let config = ModelConfig {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
        };
        let provider = from_config("local", &config).unwrap();
        assert_eq!(provider.provider_name(), "openai");
        // La URL se almacena internamente (verificable en tests de OpenAiProvider)

        // Para Anthropic
        let config2 = ModelConfig {
            provider: "anthropic".to_string(),
            model_id: "claude".to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some("https://proxy.example.com".to_string()),
        };
        let provider2 = from_config("claude_local", &config2).unwrap();
        assert_eq!(provider2.provider_name(), "anthropic");
    }

    /// CA3: from_config devuelve error descriptivo para provider desconocido.
    #[test]
    fn story_v10001_ca3_from_config_error_on_unknown_provider() {
        let config = ModelConfig {
            provider: "mistral".to_string(),
            model_id: "mistral-large".to_string(),
            api_key: "test".to_string(),
            base_url: None,
        };
        let result = from_config("my-model", &config);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("mistral"),
            "error debe mencionar el provider: {err}"
        );
        assert!(
            err.contains("my-model"),
            "error debe mencionar el nombre del modelo: {err}"
        );
        assert!(err.contains("openai"), "error debe sugerir openai: {err}");
        assert!(
            err.contains("anthropic"),
            "error debe sugerir anthropic: {err}"
        );
    }

    /// CA3: from_config con provider vacío retorna error.
    #[test]
    fn story_v10001_ca3_from_config_error_on_empty_provider() {
        let config = ModelConfig {
            provider: String::new(),
            model_id: "model".to_string(),
            api_key: "key".to_string(),
            base_url: None,
        };
        let result = from_config("empty", &config);
        assert!(result.is_err());
    }

    /// CA3: from_config es case-insensitive para el provider.
    #[test]
    fn story_v10001_ca3_from_config_provider_case_insensitive() {
        for name in &["OPENAI", "OpenAI", "openai", "Anthropic", "ANTHROPIC"] {
            let provider_name = if name.to_lowercase().contains("openai") {
                "openai"
            } else {
                "anthropic"
            };

            let config = ModelConfig {
                provider: name.to_string(),
                model_id: "model".to_string(),
                api_key: "key".to_string(),
                base_url: None,
            };
            let result = from_config("test", &config);
            assert!(
                result.is_ok(),
                "from_config con provider='{name}' debería ser Ok"
            );
            assert_eq!(
                result.unwrap().provider_name(),
                provider_name,
                "provider='{name}' debería resolver a '{provider_name}'"
            );
        }
    }
}
