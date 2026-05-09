# EPIC-V10-06: Tests y Cobertura

## Objetivo
Garantizar la calidad del rework con tests unitarios y de integración para todos los módulos nuevos y adaptados: cliente LLM con mock server HTTP, ConfigurableWorkflow cargando TOML de prueba, Task genérico con distintos formatos, pipeline con mock LLM provider, presets de fábrica, y tests de arquitectura actualizados para las nuevas capas.

## Alcance
- Tests del cliente LLM (`OpenAiProvider`, `AnthropicProvider`) con mock server (`hyper`/`axum` o `wiremock`)
- Tests de `ConfigurableWorkflow` deserializando TOML con fases, roles y guards
- Tests de `Task` con parseo configurable (distintos `id_pattern` y `section_markers`)
- Tests de `pipeline.rs` con mock `LlmProvider` que simula respuestas
- Tests de presets (`software-dev`, `research`, `single-agent`)
- Actualizar `tests/architecture.rs` (reglas R1-R5 para nuevas capas)

## Historias
- STORY-V10-021: Tests del cliente LLM con mock server
- STORY-V10-022: Tests de ConfigurableWorkflow y Task genérico
- STORY-V10-023: Tests de pipeline con mock LLM provider
- STORY-V10-024: Tests de presets y actualización de arquitectura

## Dependencias
- EPIC-V10-01 a EPIC-V10-05 (los tests validan el código ya implementado)

## Rama
`rework`

## Estimación
1-2 semanas
