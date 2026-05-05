# EPIC-07: Infraestructura Base para Logs Transparentes

## Descripción
Estructuras de datos y funciones fundacionales que todo el resto del refactor de logging necesita: resolución de modelo por rol desde la configuración, tipo `TokenCount` con acumulación en `SharedState`, y parseo de consumo de tokens desde la salida de agentes. Sin dependencias externas — código puro o con efectos mínimos.

## Historias
- STORY-019: Añadir campo `model` a `AgentsConfig` y `AgentRoleConfig` + `model_for_role()`
- STORY-020: Añadir `TokenCount` y `token_usage` a `SharedState`
- STORY-021: Implementar `parse_token_count()` con patrones multi-provider
