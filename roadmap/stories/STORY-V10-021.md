# STORY-V10-021: Tests del cliente LLM con mock server

## Status
**Draft**

## Epic
EPIC-V10-06

## Descripción
Crear tests unitarios y de integración para el módulo `infra/llm/` usando un mock server HTTP que simule las APIs de OpenAI y Anthropic. Los tests deben cubrir:

- Respuestas exitosas con parseo correcto de `ChatResponse`
- Errores HTTP (4xx, 5xx) con propagación de mensajes de error
- Timeout de request
- Rate limiting (HTTP 429 con header `retry-after`)
- Backoff exponencial en reintentos
- Expansión de `${ENV_VAR}` en api_key

Se recomienda usar `wiremock` o un approach de mock server simple con `tokio::net::TcpListener`.

**Valor de negocio**: Garantiza que el cliente LLM funciona correctamente sin depender de APIs reales (coste, latencia, rate limits).

## Criterios de aceptación
- [ ] CA1: Tests para `OpenAiProvider`: mock server que recibe `POST /v1/chat/completions`, verifica body JSON, responde con `ChatResponse` sintético. Incluye test de error 401 (API key inválida) y test de error 500 (server error)
- [ ] CA2: Tests para `AnthropicProvider`: mock server que recibe `POST /v1/messages`, verifica headers (`x-api-key`, `anthropic-version`), y formato de mensajes (system top-level, roles user/assistant). Incluye test de HTTP 429 con header `retry-after`
- [ ] CA3: Tests de `invoke_with_retry()`: mock server que falla las primeras 2 llamadas (HTTP 503), responde en la tercera. Verifica que el backoff crece (`delay *= 2`) y que la respuesta exitosa se devuelve correctamente. Test de timeout: mock server que duerme más que el timeout configurado

## Dependencias
- Bloqueado por: STORY-V10-002, STORY-V10-003, STORY-V10-004

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 6
