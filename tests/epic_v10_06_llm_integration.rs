//! Integration tests for EPIC-V10-06 — STORY-V10-021: LLM client with mock HTTP server.
//!
//! These tests define the expected behavior for HTTP-level integration tests
//! of OpenAiProvider and AnthropicProvider. They follow the Gherkin scenarios from:
//!   - roadmap/features/infra/llm-provider.feature (STORY-V10-001, -002, -003)
//!   - roadmap/features/infra/llm-retry.feature    (STORY-V10-004)
//!
//! TDD RED: All tests are #[ignore]d because they require a mock HTTP server
//! infrastructure that doesn't exist yet. The inline unit tests in
//! src/infra/llm/ already test internal methods. These integration tests
//! will validate the full HTTP request/response cycle once:
//!   1. A mock HTTP server utility is available (e.g., wiremock, or custom TCP mock)
//!   2. lib.rs exists to publicly export the LLM module
//!
//! For now, these tests serve as executable documentation of the expected
//! integration behavior.

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-002: OpenAiProvider HTTP integration
// ═══════════════════════════════════════════════════════════════════════

/// Gherkin: "OpenAiProvider construye la request HTTP correctamente"
///
/// Expected behavior:
/// - POST to /v1/chat/completions
/// - Authorization: Bearer <api_key>
/// - Body: { model: "...", messages: [{role, content}, ...] }
/// - Parses ChatResponse from JSON response
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure (wiremock or custom TCP mock)"]
fn gherkin_openai_correct_http_request() {
    // TODO: When mock server exists:
    // let server = spawn_mock().expect("POST", "/v1/chat/completions")
    //     .respond_with(200, r#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#);
    // let provider = OpenAiProvider::new("sk-test", server.url());
    // let result = provider.chat(vec![Message::system("..."), Message::user("...")], "gpt-4o", Duration::from_secs(30));
    // assert!(result.is_ok());
    // let req = server.captured();
    // assert_eq!(req.header("Authorization"), "Bearer sk-test");
    // assert_eq!(req.json_body()["model"], "gpt-4o");
}

/// Gherkin: "OpenAiProvider con base_url personalizada (Ollama)"
///
/// Expected: with empty api_key, "Bearer " header is sent (Ollama-compatible)
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn gherkin_openai_custom_url_ollama() {
    // TODO: provider with base_url=http://localhost:11434/v1 and api_key=""
    // should send Authorization: "Bearer "
}

/// Gherkin: "OpenAiProvider propaga errores HTTP correctamente"
///
/// Expected: HTTP 401 → Err with "401" and error message body
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn gherkin_openai_propagates_401_error() {
    // TODO: mock responds 401 {"error":{"message":"Invalid API key"}}
    // chat() returns Err containing "401" and "Invalid API key"
}

/// Gherkin (extra): HTTP 500 propagated with message
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn gherkin_openai_propagates_500_error() {
    // TODO: mock responds 500 {"error":{"message":"Internal Server Error"}}
    // chat() returns Err containing "500" and "Internal Server Error"
}

/// Edge: Very short timeout causes error on slow server
#[test]
#[ignore = "TDD RED: requires mock HTTP server with delay capability"]
fn openai_timeout_causes_error() {
    // TODO: mock server with 5s delay, timeout=100ms → Err (timeout)
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-003: AnthropicProvider HTTP integration
// ═══════════════════════════════════════════════════════════════════════

/// Gherkin: "AnthropicProvider adapta mensajes al formato de Anthropic"
///
/// Expected:
/// - POST to /v1/messages
/// - x-api-key header
/// - anthropic-version header present
/// - system is top-level field (not in messages array)
/// - messages array only contains user/assistant roles
/// - Parses multi-block content correctly
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn gherkin_anthropic_adapts_messages_format() {
    // TODO: mock server captures the request
    // Verify system is top-level, messages only user/assistant
    // Verify x-api-key and anthropic-version headers
}

/// Gherkin: "AnthropicProvider maneja rate limiting (HTTP 429)"
///
/// Expected: HTTP 429 with retry-after: 30 → Err with "Retry-After: 30"
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn gherkin_anthropic_handles_429_with_retry_after() {
    // TODO: mock responds 429 + retry-after: 30
    // chat() returns Err containing "429" and "Retry-After: 30"
}

/// Edge: No system message → no system field in body
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn anthropic_no_system_message_omits_field() {
    // TODO: messages without system role → body has no "system" field
}

/// Edge: Multi-block content joined with newlines
#[test]
#[ignore = "TDD RED: requires mock HTTP server infrastructure"]
fn anthropic_joins_multi_block_content() {
    // TODO: response with 2 content blocks → content is joined with "\n"
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-004: Retry integration (tests require mock server + retry infra)
// ═══════════════════════════════════════════════════════════════════════

/// Gherkin: "Backoff exponencial entre reintentos"
///
/// Provider fails 2x with 503, succeeds on 3rd.
/// retry_delay_base=2s, max_retries=5, timeout=30s.
/// Result: 3 calls, delays ≥2s and ≥4s, final response "ok".
#[tokio::test]
#[ignore = "TDD RED: requires lib.rs with invoke_with_retry exported"]
async fn gherkin_backoff_exponential_retries() {
    // TODO: MockLlmProvider that fails N times
    // invoke_with_retry(mock, msgs, "model", timeout, 5, Duration::from_secs(2), None)
    // Verify 3 calls total and elapsed ≥ 6s
}

/// Gherkin: "Timeout aborta la request y se reintenta"
///
/// Provider takes >timeout on first attempt → retried (timeout is retryable)
#[tokio::test]
#[ignore = "TDD RED: requires lib.rs"]
async fn gherkin_timeout_aborts_and_retries() {
    // TODO: timeout errors are retryable
}

/// Gherkin: "Rate limiting respeta HTTP 429 con header retry-after"
///
/// First call: HTTP 429 + retry-after: 15
/// Second call: success
/// Delay before retry ≥ 15s (respects retry-after, not backoff)
#[tokio::test]
#[ignore = "TDD RED: requires lib.rs with RateLimiter + invoke_with_retry exported"]
async fn gherkin_rate_limiting_respects_retry_after() {
    // TODO: Mock provider that returns 429 first, then 200
    // RateLimiter applied → retry-after respected
}

/// Edge: Non-retryable errors (401) are NOT retried
#[test]
#[ignore = "TDD RED: requires lib.rs"]
fn non_retryable_errors_not_retried() {
    // TODO: 401 → single call, immediate error return
}

// ═══════════════════════════════════════════════════════════════════════
// Factory tests
// ═══════════════════════════════════════════════════════════════════════

/// Gherkin: "La factory instancia el provider correcto desde config"
#[test]
#[ignore = "TDD RED: requires lib.rs with from_config() exported"]
fn factory_instantiates_correct_provider() {
    // TODO: from_config("gpt4o", &openai_config) → provider_name() = "openai"
    // TODO: from_config("claude", &anthropic_config) → provider_name() = "anthropic"
    // TODO: from_config("unknown", &mistral_config) → Err
}

/// Edge: from_config is case-insensitive
#[test]
#[ignore = "TDD RED: requires lib.rs"]
fn factory_case_insensitive() {
    // TODO: "OPENAI" → OpenAiProvider, "ANTHROPIC" → AnthropicProvider
}
