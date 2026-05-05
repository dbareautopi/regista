# STORY-019: Añadir campo `model` a configuración y función `model_for_role()`

## Status
**Draft**

## Epic
EPIC-07

## Descripción
Ampliar la configuración de agentes en `config.rs` para que soporte un campo `model` opcional a nivel global (`AgentsConfig.model`) y por rol (`AgentRoleConfig.model`). Implementar `AgentsConfig::model_for_role(role, skill_path)` que resuelve el modelo con prioridad: rol > global > YAML frontmatter del skill > `"desconocido"`.

## Criterios de aceptación
- [ ] CA1: `AgentsConfig` tiene campo `pub model: Option<String>` con `#[serde(default)]` y aparece en `Default`
- [ ] CA2: `AgentRoleConfig` tiene campo `pub model: Option<String>` con `#[serde(default)]`
- [ ] CA3: `AgentsConfig::model_for_role(role: &str, skill_path: &Path) -> String` existe y compila
- [ ] CA4: `model_for_role` devuelve el modelo de `AgentRoleConfig.model` si está definido para ese rol
- [ ] CA5: `model_for_role` devuelve `AgentsConfig.model` (global) si no hay por rol
- [ ] CA6: `model_for_role` lee el campo `model` del YAML frontmatter del skill (usando `providers::read_yaml_field`) si no hay en config
- [ ] CA7: `model_for_role` devuelve `"desconocido"` cuando no hay modelo en ningún lado
- [ ] CA8: `model_for_role` no paniquea si `skill_path` no existe (trata error de lectura como fallback a `"desconocido"`)
- [ ] CA9: El archivo `.regista/config.toml` existente sigue parseando sin errores (el campo nuevo es opcional)
- [ ] CA10: Tests unitarios cubren los 4 casos de resolución (rol, global, YAML, desconocido)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 7: Resolución del modelo).
