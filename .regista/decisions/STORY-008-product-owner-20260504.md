# Decisión de Product Owner — STORY-008

**Fecha:** 2026-05-04
**Rol:** Product Owner
**Historia:** STORY-008 — Migrar `pipeline.rs` a usar `&dyn Workflow`
**Transición:** Business Review → Done

---

## Validación de valor de negocio

### ¿Qué se pidió?
Migrar `pipeline.rs` para que use el trait `&dyn Workflow` en lugar de funciones hardcodeadas
(`next_status()`, `map_status_to_role()`, y transiciones automáticas hardcodeadas).

### ¿Qué se entregó?

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `run_real()` acepta `workflow: &dyn Workflow` | ✅ Verificado en `pipeline.rs:82` |
| CA2 | `process_story()` usa `workflow.map_status_to_role()` | ✅ Línea 717 |
| CA3 | `apply_automatic_transitions()` usa `workflow.next_status()` | ✅ Línea 596 |
| CA4 | Transición Blocked→Ready desde el workflow | ✅ `CanonicalWorkflow::next_status(Blocked) => Ready` |
| CA5 | Funciones hardcodeadas eliminadas | ✅ `next_status()` y `map_status_to_role()` no existen en `pipeline.rs` |
| CA6 | Tests del pipeline pasan | ✅ 227 tests, 0 fallos |
| CA7 | `cargo build` sin warnings | ✅ Compilación limpia |

### Verificación técnica adicional
- **Tests:** 227 pasando (216 unitarios + 11 arquitectura), 0 fallos
- **Build:** `cargo build` limpio, sin warnings
- **Formato:** `cargo fmt --check` sin diferencias
- **Linting:** `cargo clippy -- -D warnings` sin warnings

### Valor de negocio entregado
El pipeline ahora es **extensible**. Cualquier implementación del trait `Workflow` puede
reemplazar el comportamiento canónico sin modificar el código del orquestador. Esto
permite workflows custom por proyecto, habilitando la feature de workflow configurable (#04 del roadmap).

## Decisión

**APROBADO. Avanzar a Done.** La implementación satisface todos los criterios de aceptación,
el código está limpio, los tests pasan, y el valor de negocio (extensibilidad del pipeline)
está plenamente entregado.
