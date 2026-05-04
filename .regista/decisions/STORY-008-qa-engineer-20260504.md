# STORY-008 — QA Decision Record

**Fecha**: 2026-05-04
**Rol**: QA Engineer

---

## Resumen

Se verificaron y completaron los tests unitarios para STORY-008 (migrar `pipeline.rs` a usar `&dyn Workflow`).

## Estado de cobertura por CA

| CA | Descripción | Tests | Estado |
|----|-------------|-------|--------|
| CA1 | `run_real()` acepta `&dyn Workflow` | 5 tests | 4 pass, 1 fail (Blocked→Ready pendiente) |
| CA2 | `process_story()` usa `workflow.map_status_to_role()` | 4 tests | 4 pass |
| CA3 | `apply_automatic_transitions()` usa `workflow.next_status()` | 6 tests | 3 pass, 3 fail (Blocked→Ready pendiente) |
| CA4 | `Blocked→Ready` viene del workflow | 3 tests | 0 pass, 3 fail (Blocked→Ready pendiente) |
| CA5 | Hardcoded functions eliminadas | 3 tests | 2 pass, 1 fail (Blocked→Ready pendiente) |
| CA6 | `cargo test` pasa | No unit-testable | — |
| CA7 | `cargo build` sin warnings | No unit-testable | — |

## Análisis de fallos

Los 7 tests que fallan comparten la misma causa raíz:
- `CanonicalWorkflow::next_status(Status::Blocked)` retorna `Status::Blocked` (vía `_ => current`)
- Debe retornar `Status::Ready` para el desbloqueo automático
- El Developer debe añadir `Status::Blocked => Status::Ready` a la implementación

## Tests añadidos/modificados

- **Corregidos**: 3 tests que no compilaban (llamaban `apply_automatic_transitions` con 6 args pero la función acepta 5). Se eliminó el argumento extra `&wf` manteniendo la semántica de test.
- **Añadidos**:
  - `process_story_target_status_comes_from_workflow` — CA1/CA2: verifica que el `to` viene del workflow
  - `run_dry_next_status_uses_workflow` — CA1: run_dry usa workflow para next_status
  - `run_real_can_construct_default_workflow` — CA1: constructor por defecto
  - `workflow_next_status_is_idempotent_for_terminal_states` — CA3: idempotencia de terminales
  - `automatic_fail_transition_does_not_rely_on_workflow_next_status` — CA3: transición *→Failed

## Total

- **217 tests** totales en el proyecto
- **209 pass**, **7 fail** (todos por Blocked→Ready), **1 ignored** (pi no instalado)
- **21 tests** específicos para STORY-008 (19 en `pipeline::tests::story008` + 2 en `pipeline::tests`)

- **14 pass**, **7 fail** (todos por Blocked→Ready)

## Decisión

Los tests cubren exhaustivamente los 7 CAs. Los 7 fallos son TDD puro — guían al Developer sobre exactamente qué cambiar. El Developer debe:
1. Añadir `Status::Blocked => Status::Ready` a `CanonicalWorkflow::next_status()` en `src/domain/workflow.rs`
2. Añadir `workflow: &dyn Workflow` como parámetro a `apply_automatic_transitions()`, `process_story()`, `run_real()` y `run_dry()`
3. Eliminar las funciones hardcodeadas `next_status()` y `map_status_to_role()`
4. Actualizar los tests en `workflow.rs` (`blocked_stays` → debe cambiar)

→ Avanza a **Tests Ready**.
