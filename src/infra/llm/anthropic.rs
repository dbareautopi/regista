//! Provider LLM para Anthropic (Messages API).
//!
//! Soporta modelos Claude (Sonnet, Opus). A diferencia de OpenAI, Anthropic:
//! - Usa `x-api-key` en lugar de `Authorization: Bearer`
//! - Requiere el header `anthropic-version`
//! - Envía el system prompt como campo top-level, no como mensaje

use std::time::Duration;

use super::types::{ChatResponse, Message, TokenUsage};
use super::LlmProvider;

/// Provider que llama a la Messages API de Anthropic.
///
/// Maneja la conversión de formato: los mensajes con rol `system` se extraen
/// y se envían como campo `system` de nivel superior. Los mensajes con roles
/// `user` y `assistant` se envían en el array `messages`.
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    /// Crea un nuevo provider de Anthropic.
    ///
    /// `base_url` es la URL base de la API (default: `https://api.anthropic.com`).
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }

    /// URL base (para tests).
    #[cfg(test)]
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// API key (para tests).
    #[cfg(test)]
    pub(crate) fn api_key_str(&self) -> &str {
        &self.api_key
    }

    /// Construye el endpoint de la Messages API.
    fn messages_endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    /// Extrae el system prompt de los mensajes (primer mensaje con rol `system`).
    fn extract_system_prompt(messages: &[Message]) -> Option<String> {
        messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.clone())
            .next()
    }

    /// Filtra los mensajes no-system (user/assistant) para el array `messages`.
    fn filter_conversation(messages: &[Message]) -> Vec<&Message> {
        messages.iter().filter(|m| m.role != "system").collect()
    }

    /// Construye el body JSON para Anthropic.
    fn build_body(&self, messages: &[Message], model: &str) -> serde_json::Value {
        let system = Self::extract_system_prompt(messages);
        let conversation = Self::filter_conversation(messages);

        let msgs: Vec<serde_json::Value> = conversation
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "messages": msgs,
        });

        if let Some(sys) = system {
            body["system"] = serde_json::json!(sys);
        }

        body
    }

    /// Parsea la respuesta de Anthropic a ChatResponse.
    fn parse_response(body: &serde_json::Value) -> anyhow::Result<ChatResponse> {
        let content_blocks = body["content"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("respuesta Anthropic sin campo 'content'"))?;

        let content = content_blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let finish_reason = body["stop_reason"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let token_usage = body["usage"].as_object().map(|usage| TokenUsage {
            input: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
            output: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
        });

        Ok(ChatResponse {
            content,
            finish_reason,
            token_usage,
        })
    }

    /// Maneja un error HTTP de Anthropic, extrayendo `retry-after` si es 429.
    fn handle_http_error(
        status: u16,
        body_str: &str,
        retry_after: Option<String>,
    ) -> anyhow::Error {
        let mut msg = format!(
            "API Anthropic devolvió HTTP {}: {}",
            status,
            if body_str.len() > 500 {
                &body_str[..500]
            } else {
                body_str
            }
        );

        if let Some(ref ra) = retry_after {
            msg.push_str(&format!(". Retry-After: {ra}"));
        }

        anyhow::anyhow!(msg)
    }
}

impl LlmProvider for AnthropicProvider {
    fn chat(
        &self,
        messages: Vec<Message>,
        model: &str,
        timeout: Duration,
    ) -> anyhow::Result<ChatResponse> {
        let url = self.messages_endpoint();
        let body = self.build_body(&messages, model);

        let response = ureq::post(&url)
            .timeout(timeout)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("error HTTP al llamar a Anthropic ({}): {e}", url))?;

        let status = response.status();
        let retry_after = response.header("retry-after").map(|s| s.to_string());

        let body_str = response
            .into_string()
            .map_err(|e| anyhow::anyhow!("error leyendo respuesta Anthropic: {e}"))?;

        if status >= 400 {
            return Err(Self::handle_http_error(status, &body_str, retry_after));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_str)
            .map_err(|e| anyhow::anyhow!("error parseando JSON de Anthropic: {e}"))?;

        Self::parse_response(&parsed)
    }

    fn provider_name(&self) -> &str {
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::Message;
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // EPIC-V10-01 | STORY-V10-003: Implementar AnthropicProvider
    // ═══════════════════════════════════════════════════════════════

    // ── CA1: Implementa LlmProvider ──────────────────────────────────

    /// CA1: AnthropicProvider implementa el trait LlmProvider.
    #[test]
    fn story_v10003_ca1_implements_llm_provider() {
        let provider = AnthropicProvider::new(
            "sk-ant-test".to_string(),
            "https://api.anthropic.com".to_string(),
        );
        assert_eq!(provider.provider_name(), "anthropic");
    }

    /// CA1: El trait es Send + Sync + Debug.
    #[test]
    fn story_v10003_ca1_provider_is_send_sync_debug() {
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_send_sync::<AnthropicProvider>();
        assert_debug::<AnthropicProvider>();
    }

    // ── extract_system_prompt ────────────────────────────────────────

    #[test]
    fn extract_system_prompt_returns_first_system_message() {
        let messages = vec![
            Message::system("eres un asistente útil"),
            Message::user("hola"),
        ];
        let result = AnthropicProvider::extract_system_prompt(&messages);
        assert_eq!(result, Some("eres un asistente útil".to_string()));
    }

    #[test]
    fn extract_system_prompt_no_system_message() {
        let messages = vec![Message::user("pregunta"), Message::assistant("respuesta")];
        let result = AnthropicProvider::extract_system_prompt(&messages);
        assert!(result.is_none());
    }

    #[test]
    fn extract_system_prompt_multiple_systems_returns_first() {
        let messages = vec![
            Message::system("instrucción A"),
            Message::system("instrucción B"),
        ];
        let result = AnthropicProvider::extract_system_prompt(&messages);
        assert_eq!(result, Some("instrucción A".to_string()));
    }

    // ── filter_conversation ──────────────────────────────────────────

    #[test]
    fn filter_conversation_excludes_system_messages() {
        let messages = vec![
            Message::system("system prompt"),
            Message::user("user msg"),
            Message::assistant("assistant msg"),
        ];
        let filtered = AnthropicProvider::filter_conversation(&messages);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].role, "user");
        assert_eq!(filtered[1].role, "assistant");
    }

    #[test]
    fn filter_conversation_all_user_assistant() {
        let messages = vec![
            Message::user("u1"),
            Message::assistant("a1"),
            Message::user("u2"),
        ];
        let filtered = AnthropicProvider::filter_conversation(&messages);
        assert_eq!(filtered.len(), 3);
    }

    // ── build_body ───────────────────────────────────────────────────

    /// CA1: El system prompt va como campo top-level.
    #[test]
    fn story_v10003_ca1_build_body_puts_system_as_top_level() {
        let provider = AnthropicProvider::new(
            "sk-test".to_string(),
            "https://api.anthropic.com".to_string(),
        );
        let messages = vec![
            Message::system("instrucción del sistema"),
            Message::user("pregunta del usuario"),
        ];
        let body = provider.build_body(&messages, "claude-sonnet-4");

        // system es un campo top-level, no está en messages
        assert_eq!(body["system"], "instrucción del sistema");
        assert_eq!(body["model"], "claude-sonnet-4");

        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "pregunta del usuario");
    }

    /// CA1: Los mensajes user/assistant van en el array messages.
    #[test]
    fn story_v10003_ca1_build_body_messages_have_user_assistant_only() {
        let provider = AnthropicProvider::new(
            "sk-test".to_string(),
            "https://api.anthropic.com".to_string(),
        );
        let messages = vec![
            Message::user("primera pregunta"),
            Message::assistant("primera respuesta"),
            Message::user("segunda pregunta"),
        ];
        let body = provider.build_body(&messages, "claude-opus");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[2]["role"], "user");

        // No debe haber campo system
        assert!(body.get("system").is_none() || body["system"].is_null());
    }

    #[test]
    fn build_body_no_system_message_no_system_field() {
        let provider = AnthropicProvider::new(
            "sk-test".to_string(),
            "https://api.anthropic.com".to_string(),
        );
        let messages = vec![Message::user("hola")];
        let body = provider.build_body(&messages, "claude");
        // system no debería estar presente
        assert!(body.get("system").is_none());
    }

    // ── messages_endpoint ────────────────────────────────────────────

    #[test]
    fn messages_endpoint_builds_correct_url() {
        let provider = AnthropicProvider::new(
            "sk-test".to_string(),
            "https://api.anthropic.com".to_string(),
        );
        assert_eq!(
            provider.messages_endpoint(),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn messages_endpoint_with_trailing_slash() {
        let provider = AnthropicProvider::new(
            "sk-test".to_string(),
            "https://api.anthropic.com/".to_string(),
        );
        assert_eq!(
            provider.messages_endpoint(),
            "https://api.anthropic.com/v1/messages"
        );
    }

    // ── parse_response ───────────────────────────────────────────────

    #[test]
    fn parse_response_extracts_content_from_blocks() {
        let json = serde_json::json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hola, ¿cómo estás?"},
                {"type": "text", "text": " Soy Claude."}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 25,
                "output_tokens": 15
            }
        });
        let resp = AnthropicProvider::parse_response(&json).unwrap();
        assert_eq!(resp.content, "Hola, ¿cómo estás?\n Soy Claude.");
        assert_eq!(resp.finish_reason, "end_turn");
        assert_eq!(resp.token_usage.unwrap().input, 25);
        assert_eq!(resp.token_usage.unwrap().output, 15);
    }

    #[test]
    fn parse_response_single_content_block() {
        let json = serde_json::json!({
            "content": [{"type": "text", "text": "respuesta simple"}],
            "stop_reason": "end_turn"
        });
        let resp = AnthropicProvider::parse_response(&json).unwrap();
        assert_eq!(resp.content, "respuesta simple");
    }

    #[test]
    fn parse_response_missing_content_field() {
        let json = serde_json::json!({});
        let result = AnthropicProvider::parse_response(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_response_missing_stop_reason_defaults_unknown() {
        let json = serde_json::json!({
            "content": [{"type": "text", "text": "ok"}]
        });
        let resp = AnthropicProvider::parse_response(&json).unwrap();
        assert_eq!(resp.finish_reason, "unknown");
    }

    #[test]
    fn parse_response_without_usage() {
        let json = serde_json::json!({
            "content": [{"type": "text", "text": "sin conteo"}],
            "stop_reason": "max_tokens"
        });
        let resp = AnthropicProvider::parse_response(&json).unwrap();
        assert!(resp.token_usage.is_none());
    }

    // ── CA2: HTTP 429 manejo de retry-after ──────────────────────────

    /// CA2: handle_http_error incluye retry-after si está presente.
    #[test]
    fn story_v10003_ca2_handle_http_error_with_retry_after() {
        let err = AnthropicProvider::handle_http_error(
            429,
            r#"{"error":{"type":"rate_limit_error"}}"#,
            Some("30".to_string()),
        );
        let msg = err.to_string();
        assert!(msg.contains("429"), "error: {msg}");
        assert!(
            msg.contains("Retry-After"),
            "debe incluir Retry-After: {msg}"
        );
        assert!(msg.contains("30"), "debe incluir el valor: {msg}");
    }

    /// CA2: handle_http_error sin retry-after no menciona Retry-After.
    #[test]
    fn story_v10003_ca2_handle_http_error_without_retry_after() {
        let err = AnthropicProvider::handle_http_error(500, "Internal Server Error", None);
        let msg = err.to_string();
        assert!(msg.contains("500"), "error: {msg}");
        assert!(
            !msg.contains("Retry-After"),
            "no debe mencionar Retry-After: {msg}"
        );
    }

    // ── CA3: base_url configurable, api_key ──────────────────────────

    /// CA3: El constructor almacena api_key y base_url.
    #[test]
    fn story_v10003_ca3_constructor_stores_config() {
        let provider = AnthropicProvider::new(
            "sk-ant-custom".to_string(),
            "https://custom.anthropic.com".to_string(),
        );
        assert_eq!(provider.api_key_str(), "sk-ant-custom");
        assert_eq!(provider.base_url(), "https://custom.anthropic.com");
    }

    /// CA3: Desde la factory, el default es https://api.anthropic.com.
    #[test]
    fn story_v10003_ca3_default_base_url_in_factory() {
        let config = crate::config::ModelConfig {
            provider: "anthropic".to_string(),
            model_id: "claude-sonnet-4".to_string(),
            api_key: "sk-ant-test".to_string(),
            base_url: None,
        };
        let provider = super::super::from_config("test", &config).unwrap();
        assert_eq!(provider.provider_name(), "anthropic");
    }
}
