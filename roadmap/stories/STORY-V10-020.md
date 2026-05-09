# STORY-V10-020: Eliminar dominio hardcodeado y dependencia spartito

## Status
**Draft**

## Epic
EPIC-V10-05

## Descripción
Eliminar `domain/story.rs` (el `Story` con formato STORY-NNN fijo), `domain/workflow.rs` (el `CanonicalWorkflow` de 14 transiciones), y los tipos fijos `Status`, `Actor`, `Transition` de `domain/state.rs`. Todo esto ha sido reemplazado por `domain/task.rs` (Task genérico), el nuevo `domain/workflow.rs` (ConfigurableWorkflow), y los estados/roles definidos en TOML.

También se elimina la dependencia del crate `spartito`, que era el contrato compartido con `mezzala`. Al eliminar las tools CLI externas, spartito deja de ser necesario.

**Valor de negocio**: Elimina ~1,500 líneas de código hardcodeado. El dominio ahora es 100% configurable por el usuario. Sin dependencia externa de spartito.

## Criterios de aceptación
- [ ] CA1: Los archivos `domain/story.rs` y `domain/workflow.rs` son eliminados. De `domain/state.rs` solo se conserva `SharedState` (con `TokenCount` y `token_usage`). Los imports de `Story`, `Status`, `Actor`, `Transition`, `CanonicalWorkflow` se eliminan de todos los módulos
- [ ] CA2: `Cargo.toml` elimina la dependencia `spartito`. Los re-exports en `domain/mod.rs` se actualizan para reflejar solo los módulos que quedan (`task`, `workflow`, `prompts`, `graph`, `deadlock`, `state`)
- [ ] CA3: `cargo build`, `cargo test` (unitarios), y `cargo clippy -- -D warnings` pasan sin errores. Los tests que dependían de `Story` o `CanonicalWorkflow` se eliminan o migran a usar `Task` y `ConfigurableWorkflow`

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-007, STORY-V10-019

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 5
