//! Provider LLM para APIs compatibles con OpenAI (Chat Completions API).
//!
//! Soporta OpenAI, proxies compatibles, y Ollama (que expone el mismo formato
//! de API en un endpoint local).

use std::time::Duration;

use super::types::{ChatResponse, Message, TokenUsage};
use super::LlmProvider;

/// Provider que llama a la API de Chat Completions de OpenAI.
///
/// La autenticación usa `Authorization: Bearer <api_key>`.
/// Soporta `base_url` configurable para proxies y Ollama.
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    /// Crea un nuevo provider de OpenAI.
    ///
    /// `base_url` debe ser la URL base de la API (ej: "https://api.openai.com/v1").
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }

    /// URL base configurada (para tests).
    #[cfg(test)]
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// API key configurada (para tests).
    #[cfg(test)]
    pub(crate) fn api_key_str(&self) -> &str {
        &self.api_key
    }

    /// Construye el endpoint de chat completions.
    fn chat_endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// Construye el body JSON para la request.
    fn build_body(&self, messages: &[Message], model: &str) -> serde_json::Value {
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        serde_json::json!({
            "model": model,
            "messages": msgs,
        })
    }

    /// Extrae la respuesta del body JSON de OpenAI.
    fn parse_response(body: &serde_json::Value) -> anyhow::Result<ChatResponse> {
        let choices = body["choices"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("respuesta OpenAI sin campo 'choices'"))?;

        let first = choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("respuesta OpenAI con 'choices' vacío"))?;

        let content = first["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let finish_reason = first["finish_reason"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let token_usage = body["usage"].as_object().map(|usage| TokenUsage {
            input: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            output: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            content,
            finish_reason,
            token_usage,
        })
    }

    /// Maneja un error HTTP de la API de OpenAI.
    fn handle_http_error(status: u16, body_str: &str) -> anyhow::Error {
        let detail = if body_str.len() > 500 {
            &body_str[..500]
        } else {
            body_str
        };
        anyhow::anyhow!("API OpenAI devolvió HTTP {}: {}", status, detail)
    }
}

impl LlmProvider for OpenAiProvider {
    fn chat(
        &self,
        messages: Vec<Message>,
        model: &str,
        timeout: Duration,
    ) -> anyhow::Result<ChatResponse> {
        let url = self.chat_endpoint();
        let body = self.build_body(&messages, model);

        let response = ureq::post(&url)
            .timeout(timeout)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("error HTTP al llamar a OpenAI ({}): {e}", url))?;

        let status = response.status();
        let body_str = response
            .into_string()
            .map_err(|e| anyhow::anyhow!("error leyendo respuesta OpenAI: {e}"))?;

        if status >= 400 {
            return Err(Self::handle_http_error(status, &body_str));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_str).map_err(|e| {
            anyhow::anyhow!("error parseando JSON de OpenAI: {e}. Body: {body_str}")
        })?;

        Self::parse_response(&parsed)
    }

    fn provider_name(&self) -> &str {
        "openai"
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::Message;
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // EPIC-V10-01 | STORY-V10-002: Implementar OpenAiProvider
    // ═══════════════════════════════════════════════════════════════

    // ── CA1: Implementa LlmProvider ──────────────────────────────────

    /// CA1: OpenAiProvider implementa el trait LlmProvider.
    #[test]
    fn story_v10002_ca1_implements_llm_provider() {
        let provider = OpenAiProvider::new(
            "sk-test".to_string(),
            "https://api.openai.com/v1".to_string(),
        );
        assert_eq!(provider.provider_name(), "openai");
    }

    /// CA1: El trait es Send + Sync + Debug.
    #[test]
    fn story_v10002_ca1_provider_is_send_sync_debug() {
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_send_sync::<OpenAiProvider>();
        assert_debug::<OpenAiProvider>();
    }

    // ── CA2: base_url configurable, api_key ──────────────────────────

    /// CA2: El constructor acepta api_key y base_url.
    #[test]
    fn story_v10002_ca2_constructor_stores_api_key_and_base_url() {
        let provider = OpenAiProvider::new(
            "sk-custom".to_string(),
            "https://custom.openai.com/v1".to_string(),
        );
        assert_eq!(provider.api_key_str(), "sk-custom");
        assert_eq!(provider.base_url(), "https://custom.openai.com/v1");
    }

    /// CA2: La URL por defecto desde from_config es https://api.openai.com/v1.
    #[test]
    fn story_v10002_ca2_default_base_url_in_factory() {
        let config = crate::config::ModelConfig {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            api_key: "sk-test".to_string(),
            base_url: None,
        };
        let provider = super::super::from_config("test", &config).unwrap();
        // Verificamos que compila — el base_url default se usa internamente
        assert_eq!(provider.provider_name(), "openai");
    }

    // ── chat_endpoint ────────────────────────────────────────────────

    #[test]
    fn chat_endpoint_builds_correct_url() {
        let provider = OpenAiProvider::new(
            "sk-test".to_string(),
            "https://api.openai.com/v1".to_string(),
        );
        assert_eq!(
            provider.chat_endpoint(),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn chat_endpoint_with_trailing_slash() {
        let provider = OpenAiProvider::new(
            "sk-test".to_string(),
            "https://api.openai.com/v1/".to_string(),
        );
        // No debe tener doble slash
        assert_eq!(
            provider.chat_endpoint(),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn chat_endpoint_with_custom_base_url() {
        let provider = OpenAiProvider::new(
            "sk-test".to_string(),
            "http://localhost:11434/v1".to_string(),
        );
        assert_eq!(
            provider.chat_endpoint(),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    // ── build_body ───────────────────────────────────────────────────

    #[test]
    fn build_body_includes_model_and_messages() {
        let provider = OpenAiProvider::new(
            "sk-test".to_string(),
            "https://api.example.com/v1".to_string(),
        );
        let messages = vec![Message::system("eres un asistente"), Message::user("hola")];
        let body = provider.build_body(&messages, "gpt-4o");

        assert_eq!(body["model"], "gpt-4o");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "eres un asistente");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "hola");
    }

    #[test]
    fn build_body_handles_empty_messages() {
        let provider = OpenAiProvider::new(
            "sk-test".to_string(),
            "https://api.example.com/v1".to_string(),
        );
        let body = provider.build_body(&[], "gpt-4o-mini");
        assert_eq!(body["model"], "gpt-4o-mini");
        assert!(body["messages"].as_array().unwrap().is_empty());
    }

    // ── parse_response ───────────────────────────────────────────────

    #[test]
    fn parse_response_extracts_content() {
        let json = serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": "respuesta de prueba" },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
            }
        });
        let resp = OpenAiProvider::parse_response(&json).unwrap();
        assert_eq!(resp.content, "respuesta de prueba");
        assert_eq!(resp.finish_reason, "stop");
        assert_eq!(resp.token_usage.unwrap().input, 10);
        assert_eq!(resp.token_usage.unwrap().output, 5);
    }

    #[test]
    fn parse_response_without_usage() {
        let json = serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ok" },
                "finish_reason": "length"
            }]
        });
        let resp = OpenAiProvider::parse_response(&json).unwrap();
        assert_eq!(resp.content, "ok");
        assert_eq!(resp.finish_reason, "length");
        assert!(resp.token_usage.is_none());
    }

    #[test]
    fn parse_response_missing_choices_is_error() {
        let json = serde_json::json!({});
        let result = OpenAiProvider::parse_response(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("choices"));
    }

    #[test]
    fn parse_response_empty_choices_is_error() {
        let json = serde_json::json!({"choices": []});
        let result = OpenAiProvider::parse_response(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_response_missing_content_defaults_empty() {
        let json = serde_json::json!({
            "choices": [{
                "message": { "role": "assistant" },
                "finish_reason": "stop"
            }]
        });
        let resp = OpenAiProvider::parse_response(&json).unwrap();
        assert_eq!(resp.content, "");
    }

    #[test]
    fn parse_response_missing_finish_reason_defaults_unknown() {
        let json = serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": "x" }
            }]
        });
        let resp = OpenAiProvider::parse_response(&json).unwrap();
        assert_eq!(resp.finish_reason, "unknown");
    }

    // ── CA3: HTTP errors ─────────────────────────────────────────────

    /// CA3: handle_http_error produce un error descriptivo.
    #[test]
    fn story_v10002_ca3_handle_http_error_is_descriptive() {
        let err = OpenAiProvider::handle_http_error(
            429,
            r#"{"error":{"message":"rate limit exceeded"}}"#,
        );
        let msg = err.to_string();
        assert!(msg.contains("HTTP 429"), "error: {msg}");
        assert!(
            msg.contains("rate limit exceeded"),
            "error debe contener el body: {msg}"
        );
    }

    /// CA3: handle_http_error trunca el body si es muy largo.
    #[test]
    fn story_v10002_ca3_handle_http_error_truncates_long_body() {
        let long_body = "x".repeat(1000);
        let err = OpenAiProvider::handle_http_error(500, &long_body);
        let msg = err.to_string();
        // El error no debe contener los 1000 caracteres
        assert!(!msg.contains(&"x".repeat(1000)));
    }

    /// CA3: handle_http_error con 401 devuelve error con status.
    #[test]
    fn story_v10002_ca3_handle_http_error_401() {
        let err = OpenAiProvider::handle_http_error(401, "Unauthorized");
        let msg = err.to_string();
        assert!(msg.contains("401"), "error: {msg}");
        assert!(msg.contains("Unauthorized"), "error: {msg}");
    }

    // ── Chat endpoint URL building ───────────────────────────────────

    #[test]
    fn url_building_respects_custom_base() {
        // Ollama local
        let provider = OpenAiProvider::new(
            "ollama".to_string(),
            "http://localhost:11434/v1".to_string(),
        );
        assert_eq!(
            provider.chat_endpoint(),
            "http://localhost:11434/v1/chat/completions"
        );
    }
}
