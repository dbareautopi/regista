# STORY-008: Migrar `pipeline.rs` a usar `&dyn Workflow`

## Status
**Business Review**

## Epic
EPIC-03

## Descripción
Una vez definido el trait `Workflow` y `CanonicalWorkflow`, hay que adaptar `pipeline.rs` para que use `&dyn Workflow` en lugar de las funciones hardcodeadas `next_status()`, `map_status_to_role()`, y las transiciones automáticas hardcodeadas. El orchestrator recibirá un `Box<dyn Workflow>` (por ahora siempre `CanonicalWorkflow`) y lo pasará a las funciones internas.

## Criterios de aceptación
- [x] CA1: `run_real()` acepta un parámetro `workflow: &dyn Workflow` (o lo construye internamente como `CanonicalWorkflow`)
- [x] CA2: `process_story()` usa `workflow.map_status_to_role(status)` en lugar de la función hardcodeada
- [x] CA3: `apply_automatic_transitions()` usa `workflow.next_status()` para determinar el estado de desbloqueo (en lugar de hardcodear `Status::Ready`)
- [x] CA4: La transición `Blocked → Ready` (cuando dependencias se resuelven) se obtiene del workflow, no está hardcodeada
- [x] CA5: Las funciones hardcodeadas `next_status()` y `map_status_to_role()` se eliminan de `pipeline.rs`
- [x] CA6: `cargo test --lib orchestrator` pasa (tests de pipeline)
- [x] CA7: `cargo build` compila sin warnings

## Dependencias
- Bloqueado por: STORY-007

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #1.2, #1.3, #1.4).
- 2026-05-04 | QA | Tests unitarios escritos para CA1-CA7 en src/app/pipeline.rs (mod story008). 21 tests total: 14 pasan, 7 fallan por CanonicalWorkflow.next_status(Blocked) → Blocked (debe ser → Ready). Tests compilan correctamente. Decisión documentada en .regista/decisions/STORY-008-qa-engineer-20260504.md. Avanza a Tests Ready.
- 2026-05-04 | Dev | Implementación completada en 2 archivos: (1) src/domain/workflow.rs → añadido Status::Blocked => Status::Ready a CanonicalWorkflow::next_status() para que el desbloqueo dirija a Ready. (2) src/app/pipeline.rs → eliminadas funciones hardcodeadas next_status() y map_status_to_role(); run_real(), run_dry(), process_story() y apply_automatic_transitions() ahora reciben y usan &dyn Workflow. 216 tests pasan, 0 fallos; cargo build y cargo clippy limpios. Decisión documentada en .regista/decisions/STORY-008-dev-20260504.md. Avanza a In Review.
- 2026-05-04 | Reviewer | DoD técnico verificado: cargo build limpio, cargo test 227 pasando (216 unitarios + 11 arquitectura), 0 fallos, cargo fmt --check sin diferencias, cargo clippy sin warnings. CAs confirmados en código: run_real() construye &CanonicalWorkflow, process_story() usa workflow.map_status_to_role(), apply_automatic_transitions() usa workflow.next_status(Blocked) para target desbloqueo, funciones hardcodeadas eliminadas. Sin regresiones. Decisión documentada en .regista/decisions/STORY-008-reviewer-20260504.md. Avanza a Business Review.