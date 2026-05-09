# EPIC-V10-01: Cliente LLM Nativo

## Objetivo
Reemplazar la invocación de tools CLI externas (`pi`, `claude`, `codex`, `opencode`) por llamadas directas a APIs de LLM (OpenAI, Anthropic, Ollama) mediante un nuevo módulo `infra/llm/`. Esto elimina la dependencia de binarios externos y permite gestionar la conversación multi-turn con historial completo por task.

## Alcance
- Nuevo módulo `infra/llm/` con trait `LlmProvider`, tipos de mensaje, y factory
- Implementación de `OpenAiProvider` (compatible con Ollama vía mismo formato API)
- Implementación de `AnthropicProvider` (Messages API)
- Retry con backoff exponencial, timeout configurable, y rate limiting
- Configuración de modelos LLM en `.regista/config.toml`

## Historias
- STORY-V10-001: Definir trait LlmProvider y tipos base
- STORY-V10-002: Implementar OpenAiProvider
- STORY-V10-003: Implementar AnthropicProvider
- STORY-V10-004: Retry con backoff, timeout y rate limiting
- STORY-V10-005: Configuración de modelos LLM en TOML

## Dependencias
- Ninguna (es la base del rework)

## Rama
`rework`

## Estimación
2 semanas
