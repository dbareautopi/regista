# PO Decision: STORY-002 — Refinamiento Draft → Ready

**Fecha**: 2026-05-05
**Rol**: Product Owner
**Decisión**: Mover STORY-002 de Draft a Ready

## Verificación del Definition of Ready

| Criterio | Estado | Evidencia |
|----------|--------|-----------|
| Descripción clara y no ambigua | ✅ | Explica con precisión el acoplamiento incorrecto y qué debe moverse |
| Criterios de aceptación específicos y testeables | ✅ | 7 CAs concretos: añadir 2 métodos + verificar 1 existente + eliminar 2 funciones + actualizar callers + build + tests |
| Dependencias identificadas | ✅ | Explícitamente "Ninguna" |
| Activity Log presente | ✅ | Entrada inicial del 2026-05-04 |
| Epic asignado | ✅ | EPIC-01 |
| Formato de historia correcto | ✅ | Cumple contrato (Status, Epic, Descripción, CAs, Dependencias, Activity Log) |
| Tamaño adecuado (no épica) | ✅ | Refactorización acotada a ~5 archivos |
| Sin ambigüedades en los CAs | ✅ | CA1-CA7 son binarios (compila/no compila, test pasa/no pasa) |

## Análisis de impacto

- **Archivos modificados**: `src/config.rs` (+métodos), `src/infra/providers.rs` (-funciones), callers en `src/app/pipeline.rs`, `src/app/plan.rs`, `src/app/validate.rs`
- **Riesgo**: Bajo. Cambio puramente mecánico de `providers::provider_for_role(&cfg.agents, role)` → `cfg.agents.provider_for_role(role)`.
- **Rollback**: Trivial vía git revert.

## Notas adicionales

- `AgentsConfig::all_roles()` ya existe como método estático (CA3 verificado: solo requiere confirmación).
- Los callers identificados vía grep: `pipeline.rs` (7 calls), `plan.rs` (2 calls), `validate.rs` (2 calls), `config.rs` tests (12 calls que también deben migrarse).
- Las funciones en `providers.rs` tienen tests propios que deben moverse a `config.rs`.

## Conclusión

Historia lista para desarrollo. Sin bloqueos, alcance claro, CAs verificables mecánicamente.
