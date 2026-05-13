# STORY-V10-002: Implementar OpenAiProvider

## Status
**Done**

## Epic
EPIC-V10-01

## Descripción
Implementar `OpenAiProvider` en `infra/llm/openai.rs` que llame a la API de chat/completions de OpenAI. Este provider debe soportar `base_url` configurable para ser compatible con proxies y con Ollama (que expone el mismo formato de API en un endpoint local).

La autenticación se hará mediante `Authorization: Bearer <api_key>` donde `api_key` se resuelve desde variables de entorno con el patrón `${ENV_VAR}`.

**Valor de negocio**: Primer provider LLM nativo funcional. Permite usar cualquier modelo compatible con el formato OpenAI (GPT-4o, GPT-4.1, Ollama local, etc.).

## Criterios de aceptación
- [ ] CA1: `OpenAiProvider` implementa `LlmProvider` realizando `POST {base_url}/chat/completions` con body JSON `{ model, messages }` y parseando la respuesta a `ChatResponse`
- [ ] CA2: El campo `base_url` es configurable (default `https://api.openai.com/v1`) y `api_key` se resuelve expandiendo `${ENV_VAR}` al valor de la variable de entorno, con error claro si la variable no existe
- [ ] CA3: Si la API responde con HTTP 4xx/5xx, se propaga un error con el status code y el body de la respuesta para diagnóstico

## Dependencias
- Bloqueado por: STORY-V10-001

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 1
- 2026-05-09 | Dev | STORY-V10-002 completada: OpenAiProvider con POST JSON, base_url configurable, ${ENV_VAR} expansion, HTTP error propagation, 17 tests
