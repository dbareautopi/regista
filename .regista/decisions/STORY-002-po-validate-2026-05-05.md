# Decisión PO — STORY-002: Validación de negocio (Business Review → Done)

**Fecha**: 2026-05-05
**Rol**: Product Owner
**Transición**: Business Review → Done

## Verificación de valor de negocio

### Objetivo de la historia
Eliminar el acoplamiento incorrecto de la capa `infra` hacia `config`, reubicando las funciones `provider_for_role()` y `skill_for_role()` como métodos de `AgentsConfig` en `src/config.rs`.

### Criterios de aceptación — verificación final

| CA | Descripción | Verificación |
|----|-------------|-------------|
| CA1 | `AgentsConfig::provider_for_role(&self, role: &str) -> String` | ✅ Existe en `src/config.rs:125` |
| CA2 | `AgentsConfig::skill_for_role(&self, role: &str) -> String` | ✅ Existe en `src/config.rs:144` |
| CA3 | `AgentsConfig::all_roles()` | ✅ Existe en `src/config.rs:116` |
| CA4 | Funciones libres eliminadas de `infra/providers.rs` | ✅ No existen `fn provider_for_role` ni `fn skill_for_role` como free functions |
| CA5 | Callers actualizados a `cfg.agents.provider_for_role(...)` | ✅ `plan.rs:80-82`, `pipeline.rs:720-722,1636-1722`, `validate.rs:196,407` |
| CA6 | `cargo build` sin warnings | ✅ Compilación limpia |
| CA7 | `cargo test` pasa todos los tests | ✅ 357 tests (346 unitarios + 11 arquitectura), 0 fallos, 1 ignorado |

### Evidencia de ejecución
- `cargo build`: Finished dev profile, 0.28s, sin warnings
- `cargo test`: 357 passed, 0 failed, 1 ignored (requiere pi instalado)
- `grep -rn "providers::provider_for_role\|providers::skill_for_role" src/`: sin resultados → no quedan callers antiguos

### Valor de negocio
El acoplamiento `infra → config` ha sido eliminado. La arquitectura de capas (`cli → app → domain/infra → config`) se respeta ahora íntegramente. Las funciones de resolución de provider/skill son métodos de `AgentsConfig`, que es su ubicación natural según el principio de responsabilidad única. No hay regresiones funcionales.

### Conclusión
**APROBADO**. La historia cumple todos los criterios de aceptación y entrega el valor de negocio esperado. Transición: **Business Review → Done**.
