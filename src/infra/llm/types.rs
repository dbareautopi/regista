//! Tipos de datos para la comunicación con APIs LLM.
//!
//! Define el contrato de datos entre regista y los providers LLM nativos:
//! mensajes (prompt multi-turn), respuestas del modelo, y conteo de tokens.

use serde::{Deserialize, Serialize};

/// Un mensaje en una conversación con un LLM.
///
/// Los roles canónicos son `"system"`, `"user"`, y `"assistant"`.
/// Algunos providers (Anthropic) pueden requerir transformación de roles.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    /// Rol del emisor: "system", "user", o "assistant".
    pub role: String,
    /// Contenido del mensaje.
    pub content: String,
}

impl Message {
    /// Crea un mensaje con rol `"system"`.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    /// Crea un mensaje con rol `"user"`.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// Crea un mensaje con rol `"assistant"`.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// Respuesta de un modelo LLM a una solicitud de chat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatResponse {
    /// Texto generado por el modelo.
    pub content: String,
    /// Razón de finalización: `"stop"`, `"length"`, `"tool_calls"`, etc.
    pub finish_reason: String,
    /// Conteo de tokens de la respuesta (opcional, no todos los providers lo reportan).
    pub token_usage: Option<TokenUsage>,
}

/// Conteo de tokens de entrada y salida de una solicitud LLM.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    /// Tokens de entrada (prompt).
    pub input: u32,
    /// Tokens de salida (respuesta generada).
    pub output: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── CA2: Message, ChatResponse, TokenUsage con Serialize/Deserialize ──

    /// CA2: Message se serializa a JSON correctamente.
    #[test]
    fn story_v10001_ca2_message_serializes_to_json() {
        let msg = Message::user("hola mundo");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("hola mundo"));
    }

    /// CA2: Message se deserializa de JSON correctamente.
    #[test]
    fn story_v10001_ca2_message_deserializes_from_json() {
        let json = r#"{"role":"assistant","content":"respuesta del modelo"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "respuesta del modelo");
    }

    /// CA2: ChatResponse con token_usage se serializa.
    #[test]
    fn story_v10001_ca2_chat_response_with_tokens_serializes() {
        let resp = ChatResponse {
            content: "resultado".to_string(),
            finish_reason: "stop".to_string(),
            token_usage: Some(TokenUsage {
                input: 100,
                output: 50,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("resultado"));
        assert!(json.contains("stop"));
        assert!(json.contains("token_usage"));
        assert!(json.contains("100"));
        assert!(json.contains("50"));
    }

    /// CA2: ChatResponse sin token_usage se serializa con null.
    #[test]
    fn story_v10001_ca2_chat_response_without_tokens_serializes_null() {
        let resp = ChatResponse {
            content: "sin tokens".to_string(),
            finish_reason: "length".to_string(),
            token_usage: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("sin tokens"));
        assert!(json.contains("length"));
        assert!(json.contains("null"));
    }

    /// CA2: ChatResponse se deserializa con token_usage.
    #[test]
    fn story_v10001_ca2_chat_response_deserializes_with_tokens() {
        let json =
            r#"{"content":"hello","finish_reason":"stop","token_usage":{"input":10,"output":5}}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content, "hello");
        assert_eq!(resp.finish_reason, "stop");
        assert_eq!(resp.token_usage.unwrap().input, 10);
        assert_eq!(resp.token_usage.unwrap().output, 5);
    }

    /// CA2: ChatResponse se deserializa sin token_usage.
    #[test]
    fn story_v10001_ca2_chat_response_deserializes_without_tokens() {
        let json = r#"{"content":"hello","finish_reason":"stop","token_usage":null}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content, "hello");
        assert!(resp.token_usage.is_none());
    }

    /// CA2: TokenUsage con Serialize/Deserialize.
    #[test]
    fn story_v10001_ca2_token_usage_roundtrip() {
        let usage = TokenUsage {
            input: 42,
            output: 7,
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: TokenUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input, 42);
        assert_eq!(parsed.output, 7);
    }

    // ── Constructores ────────────────────────────────────────────────

    #[test]
    fn message_system_constructor() {
        let msg = Message::system("instrucción");
        assert_eq!(msg.role, "system");
        assert_eq!(msg.content, "instrucción");
    }

    #[test]
    fn message_user_constructor() {
        let msg = Message::user("pregunta");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "pregunta");
    }

    #[test]
    fn message_assistant_constructor() {
        let msg = Message::assistant("respuesta");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "respuesta");
    }

    #[test]
    fn message_equality() {
        let a = Message::user("hola");
        let b = Message::user("hola");
        let c = Message::assistant("hola");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn chat_response_equality() {
        let a = ChatResponse {
            content: "ok".to_string(),
            finish_reason: "stop".to_string(),
            token_usage: None,
        };
        let b = ChatResponse {
            content: "ok".to_string(),
            finish_reason: "stop".to_string(),
            token_usage: None,
        };
        assert_eq!(a, b);
    }
}
