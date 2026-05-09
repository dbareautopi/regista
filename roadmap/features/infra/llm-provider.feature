# language: es
@infra @llm @STORY-V10-001 @STORY-V10-002 @STORY-V10-003

Feature: Cliente LLM multi-provider nativo
  Como orquestador,
  quiero invocar APIs de LLM directamente (OpenAI, Anthropic, Ollama) mediante un trait común,
  para eliminar la dependencia de binarios CLI externos y gestionar conversaciones multi-turn.

  # ── STORY-V10-001: Trait + tipos ──────────────────────

  Scenario: El trait LlmProvider define el contrato común
    Given el módulo infra/llm compila
    When se examina el trait LlmProvider en mod.rs
    Then expone el método fn chat(&self, messages: Vec<Message>, model: &str, timeout: Duration) -> Result<ChatResponse>
    And el trait requiere Send + Sync + Debug

  Scenario: Los tipos de mensaje son serializables
    Given los structs Message, ChatResponse y TokenUsage en types.rs
    When se intenta serializar un ChatResponse a JSON con serde
    Then la serialización produce campos "content", "finish_reason" y "token_usage"
    And TokenUsage contiene campos "input" y "output" (u64)

  Scenario: La factory instancia el provider correcto desde config
    Given un ModelConfig con provider="openai", model_id="gpt-4o", api_key="sk-test"
    When se invoca LlmProvider::from_config("gpt4o", model_config)
    Then devuelve Ok(Box<dyn LlmProvider>) cuyo provider_name() es "openai"
    Given un ModelConfig con provider="anthropic"
    When se invoca LlmProvider::from_config("claude", model_config)
    Then devuelve Ok(Box<dyn LlmProvider>) cuyo provider_name() es "anthropic"
    Given un ModelConfig con provider="desconocido"
    When se invoca LlmProvider::from_config()
    Then devuelve Err con mensaje que contiene "provider desconocido"

  # ── STORY-V10-002: OpenAiProvider ──────────────────────

  Scenario: OpenAiProvider construye la request HTTP correctamente
    Given un OpenAiProvider con base_url="https://api.openai.com/v1" y api_key="sk-test"
    And mensajes [{role: "system", content: "Eres útil"}, {role: "user", content: "Hola"}]
    When se invoca chat() con model="gpt-4o" y timeout=30s
    Then se realiza POST a https://api.openai.com/v1/chat/completions
    And el header Authorization es "Bearer sk-test"
    And el body JSON contiene "model": "gpt-4o"
    And el body JSON contiene el array "messages" con 2 elementos

  Scenario: OpenAiProvider con base_url personalizada (Ollama)
    Given un OpenAiProvider con base_url="http://localhost:11434/v1" y api_key=""
    When se invoca chat()
    Then se realiza POST a http://localhost:11434/v1/chat/completions
    And el header Authorization es "Bearer " (api_key vacía, compatible con Ollama)

  Scenario: OpenAiProvider propaga errores HTTP correctamente
    Given el servidor OpenAI responde con HTTP 401 {"error": {"message": "Invalid API key"}}
    When se invoca chat()
    Then devuelve Err con mensaje que contiene "401" y "Invalid API key"

  # ── STORY-V10-003: AnthropicProvider ────────────────────

  Scenario: AnthropicProvider adapta mensajes al formato de Anthropic
    Given un AnthropicProvider con api_key="sk-ant-test"
    And mensajes [{role: "system", content: "Eres útil"}, {role: "user", content: "Hola"}]
    When se invoca chat() con model="claude-sonnet-4-20250514"
    Then se realiza POST a https://api.anthropic.com/v1/messages
    And el header x-api-key es "sk-ant-test"
    And el header anthropic-version está presente
    And el body JSON tiene "system" como campo top-level (no dentro de messages)
    And el body JSON tiene "messages" con solo roles "user"/"assistant"

  Scenario: AnthropicProvider maneja rate limiting (HTTP 429)
    Given el servidor Anthropic responde con HTTP 429 y header retry-after: 30
    When se invoca chat()
    Then devuelve Err
    And el error contiene "retry-after: 30" para que la capa de retry lo use
