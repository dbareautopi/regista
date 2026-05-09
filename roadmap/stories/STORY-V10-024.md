# STORY-V10-024: Tests de presets y actualización de arquitectura

## Status
**Draft**

## Epic
EPIC-V10-06

## Descripción
Crear tests que validen que los 3 presets de fábrica (`software-dev`, `research`, `single-agent`) son correctos y completos: tienen estados inicial y terminal, al menos una fase, roles con system prompts, y `task_format` con `id_pattern` y `section_markers`.

Además, actualizar `tests/architecture.rs` para reflejar la nueva estructura de capas:
- `domain/` solo contiene `task.rs`, `workflow.rs`, `prompts.rs`, `graph.rs`, `deadlock.rs`, `state.rs`
- `infra/` contiene `llm/` (nuevo), `checkpoint.rs`, `daemon.rs`, `git.rs`, `hooks.rs`
- Las reglas R1-R5 se actualizan para verificar que `infra/llm/` solo importa `config` y librerías externas

**Valor de negocio**: Garantiza que los presets son usables out-of-the-box y que la arquitectura en capas se mantiene tras el rework.

## Criterios de aceptación
- [ ] CA1: Tests de presets: para cada preset, verificar que `states.initial` y `states.terminal` no están vacíos, `phases` tiene al menos 1 entrada, cada `PhaseConfig` tiene `from`/`to` que existen en `states`, y `task_format.id_pattern` es un regex válido
- [ ] CA2: Test de migración: cargar una `Story` de v0.x (formato STORY-NNN con CA) usando el `task_format` del preset `software-dev`, verificar que se parsea correctamente a `Task` con `fields["epic"]`, `fields["criterios"]`, etc.
- [ ] CA3: `tests/architecture.rs` actualizado: nuevas reglas que verifican que `infra/llm/` no importa `domain`, `app`, ni `cli`; que `domain/task.rs` no importa `infra`, `app`, ni `cli`; que `app/pipeline.rs` importa `domain` e `infra` pero no `cli`. `cargo test --test architecture` pasa con todas las reglas

## Dependencias
- Bloqueado por: STORY-V10-013, STORY-V10-014, STORY-V10-020

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 6
