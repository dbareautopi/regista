# Decisión QA: STORY-019 — Revisión de tests

**Fecha**: 2026-05-05  
**Rol**: QA Engineer  
**Historia**: STORY-019 — Añadir campo `model` a configuración y función `model_for_role()`

## Contexto

STORY-019 pide:
1. Añadir campo `model: Option<String>` a `AgentsConfig` y `AgentRoleConfig`
2. Implementar `AgentsConfig::model_for_role(role, skill_path)` con prioridad: rol > global > YAML frontmatter > `"desconocido"`

## Hallazgos

Los tests para STORY-019 ya existen en `src/config.rs` (módulo `#[cfg(test)] mod tests`). Fueron escritos junto con el stub de `model_for_role()` y las definiciones de los campos `model`.

### Cobertura

| CA | Descripción | Tests |
|---|---|---|
| CA1 | `AgentsConfig.model: Option<String>` con `#[serde(default)]` y en `Default` | 2 tests |
| CA2 | `AgentRoleConfig.model: Option<String>` con `#[serde(default)]` | 2 tests |
| CA3 | `model_for_role()` existe y compila | 3 tests |
| CA4 | Prioridad: modelo de rol | 2 tests |
| CA5 | Prioridad: modelo global | 2 tests |
| CA6 | Prioridad: YAML frontmatter | 2 tests |
| CA7 | Fallback `"desconocido"` | 2 tests |
| CA8 | No paniquea con skill_path inexistente | 3 tests |
| CA9 | Backward compatibility (parseo sin campo `model`) | 4 tests |
| CA10 | Cobertura explícita de 4 casos de resolución | 1 test |

**Total**: 21 tests unitarios

### Tests que PASAN con el stub actual

- CA1, CA2: La estructura ya tiene los campos (el PO o arquitecto los añadió junto con el stub)
- CA3: El método existe como stub
- CA7: El stub devuelve `"desconocido"` → comportamiento correcto para el caso "sin modelo"
- CA8: El stub no paniquea
- CA9: Parseo backward-compatible funciona

### Tests que FALLAN con el stub actual

- CA4: Espera `"gpt-5"` (modelo de rol), stub devuelve `"desconocido"`
- CA5: Espera `"claude-sonnet-4"` (modelo global), stub devuelve `"desconocido"`
- CA6: Espera `"opencode/gpt-5-nano"` (YAML), stub devuelve `"desconocido"`
- CA10: Fallará en los casos 1, 2, 3 por lo mismo

**Esto es esperado**: el Developer debe implementar la lógica de resolución en `model_for_role()` para que estos tests pasen.

## Decisión

**No se requieren tests nuevos.** Los 21 tests existentes cubren exhaustivamente todos los criterios de aceptación (CA1-CA10). Son tests unitarios en el módulo correcto (`src/config.rs`), no crean módulos nuevos, no requieren fake providers, y siguen las convenciones del proyecto.

El Developer debe:
1. Implementar la lógica de `model_for_role()` con la prioridad: rol > global > YAML > `"desconocido"`
2. Usar `providers::read_yaml_field()` para leer el YAML
3. Verificar que los 21 tests pasan

## Historia pasada a Tests Ready

Sin dependencias bloqueantes. Lista para que el Developer implemente.
