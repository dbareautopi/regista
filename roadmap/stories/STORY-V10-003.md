# STORY-V10-003: Implementar AnthropicProvider

## Status
**Draft**

## Epic
EPIC-V10-01

## Descripción
Implementar `AnthropicProvider` en `infra/llm/anthropic.rs` que llame a la Messages API de Anthropic (`POST /v1/messages`). Este provider maneja los headers específicos de Anthropic (`x-api-key`, `anthropic-version`) y adapta el formato de mensajes de regista al formato esperado por Anthropic (system prompt separado, roles `user`/`assistant`).

A diferencia de OpenAI, Anthropic requiere que el system prompt se envíe como campo de nivel superior (no como un mensaje con rol `system`). El provider debe manejar esta conversión internamente.

**Valor de negocio**: Soporte para modelos Claude (Sonnet, Opus), ampliando las opciones de LLM disponibles para los usuarios.

## Criterios de aceptación
- [ ] CA1: `AnthropicProvider` implementa `LlmProvider` realizando `POST {base_url}/v1/messages` con headers `x-api-key` y `anthropic-version`, adaptando `Vec<Message>` al formato Anthropic (system como campo top-level, mensajes con roles `user`/`assistant`)
- [ ] CA2: Si la API responde con HTTP 429 (rate limit), se extrae el header `retry-after` y se propaga en el error para que la capa de retry lo use
- [ ] CA3: El campo `base_url` es configurable (default `https://api.anthropic.com`) y `api_key` se resuelve con expansión `${ENV_VAR}` igual que en OpenAiProvider

## Dependencias
- Bloqueado por: STORY-V10-001

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 1
