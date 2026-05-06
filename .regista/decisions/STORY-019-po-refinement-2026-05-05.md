# Decisión de refinamiento — STORY-019

**Fecha**: 2026-05-05  
**Rol**: Product Owner  
**Transición**: Draft → Ready

## Decisión

STORY-019 cumple el Definition of Ready. Se aprueba el paso a Ready.

## Verificación DoR

1. **Descripción clara**: Define sin ambigüedad los campos `model` en `AgentsConfig` y `AgentRoleConfig`, y la función `model_for_role(role, skill_path)` con cadena de prioridad explícita: rol > global > YAML frontmatter > `"desconocido"`.

2. **Criterios testeables**: CA1-CA10 cubren exhaustivamente los 4 casos de resolución (CA4-CA7), el caso de error (CA8), retrocompatibilidad (CA9), y cobertura de tests (CA10). Todos son verificables con tests unitarios de Rust.

3. **Sin dependencias**: La historia no depende de ninguna otra — solo necesita `config.rs` y `providers::read_yaml_field`.

## Notas técnicas para el desarrollador

- `read_yaml_field` en `src/infra/providers.rs` debe hacerse `pub` para que `model_for_role` pueda invocarla (CA6).
- Los `Default` impl de `AgentsConfig` y `AgentRoleConfig` deben incluir `model: None`.
- El `config.toml` existente no define `model` → el parseo con `#[serde(default)]` no falla (CA9).
- `skill_path` puede no existir → `read_yaml_field` debe tratarse con `ok()`/`unwrap_or` (CA8).
