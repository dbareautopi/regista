# STORY-008: Migrar `pipeline.rs` a usar `&dyn Workflow`

## Status
**Ready**

## Epic
EPIC-03

## Descripción
Una vez definido el trait `Workflow` y `CanonicalWorkflow`, hay que adaptar `pipeline.rs` para que use `&dyn Workflow` en lugar de las funciones hardcodeadas `next_status()`, `map_status_to_role()`, y las transiciones automáticas hardcodeadas. El orchestrator recibirá un `Box<dyn Workflow>` (por ahora siempre `CanonicalWorkflow`) y lo pasará a las funciones internas.

## Criterios de aceptación
- [ ] CA1: `run_real()` acepta un parámetro `workflow: &dyn Workflow` (o lo construye internamente como `CanonicalWorkflow`)
- [ ] CA2: `process_story()` usa `workflow.map_status_to_role(status)` en lugar de la función hardcodeada
- [ ] CA3: `apply_automatic_transitions()` usa `workflow.next_status()` para determinar el estado de desbloqueo (en lugar de hardcodear `Status::Ready`)
- [ ] CA4: La transición `Blocked → Ready` (cuando dependencias se resuelven) se obtiene del workflow, no está hardcodeada
- [ ] CA5: Las funciones hardcodeadas `next_status()` y `map_status_to_role()` se eliminan de `pipeline.rs`
- [ ] CA6: `cargo test --lib orchestrator` pasa (tests de pipeline)
- [ ] CA7: `cargo build` compila sin warnings

## Dependencias
- Bloqueado por: STORY-007

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #1.2, #1.3, #1.4).