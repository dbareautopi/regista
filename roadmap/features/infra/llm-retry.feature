# language: es
@infra @llm @retry @STORY-V10-004

Feature: Retry con backoff, timeout y rate limiting en cliente LLM
  Como orquestador que invoca APIs externas,
  quiero reintentos automáticos con backoff exponencial, timeout configurable, y rate limiting,
  para que fallos transitorios de red no aborten el pipeline completo.

  Background:
    Given un Box<dyn LlmProvider> mock que falla las primeras 2 llamadas con HTTP 503
    And responde exitosamente en la tercera llamada con ChatResponse {content: "ok"}
    And límites configurados: retry_delay_base_seconds=2, max_retries=5, timeout_seconds=30

  Scenario: Backoff exponencial entre reintentos
    When se invoca invoke_with_retry()
    Then se realizan exactamente 3 llamadas al provider
    And el delay entre la 1ª y 2ª llamada es ≥ 2s (delay base)
    And el delay entre la 2ª y 3ª llamada es ≥ 4s (delay × 2)
    And la respuesta final es ChatResponse {content: "ok"}

  Scenario: Timeout aborta la request y se reintenta
    Given el provider tarda 35s en responder (supera timeout de 30s)
    And es el primer intento
    When se invoca invoke_with_retry()
    Then el primer intento se aborta por timeout
    And se reintenta (el timeout no cuenta como fallo terminal)

  Scenario: Rate limiting respeta HTTP 429 con header retry-after
    Given el provider responde HTTP 429 con header retry-after: 15 en el primer intento
    And responde exitosamente en el segundo intento
    When se invoca invoke_with_retry()
    Then el delay antes del segundo intento es ≥ 15s (respeta retry-after, no usa backoff)
    And la respuesta final es exitosa
