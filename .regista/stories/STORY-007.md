# STORY-007: Definir trait `Workflow` + implementar `CanonicalWorkflow`

## Status
**Draft**

## Epic
EPIC-03

## Descripción
El workflow actual está hardcodeado en múltiples funciones: `next_status()`, `map_status_to_role()`, y el orden canónico de columnas en `board.rs`. Para preparar el terreno para #04 (workflow configurable), hay que extraer un trait `Workflow` que encapsule estas decisiones, y proveer una implementación por defecto `CanonicalWorkflow` que replique exactamente el comportamiento actual. Esto permite que el resto del código se migre a `&dyn Workflow` sin cambiar el comportamiento.

## Criterios de aceptación
- [ ] CA1: Existe un trait `Workflow` en `src/domain/state.rs` (o nuevo `src/domain/workflow.rs`) con al menos estos métodos:
  - `fn next_status(&self, current: Status) -> Status`
  - `fn map_status_to_role(&self, status: Status) -> &'static str`
  - `fn canonical_column_order(&self) -> &[&'static str]`
- [ ] CA2: Existe un struct `CanonicalWorkflow` que implementa `Workflow` con el comportamiento actual (el de las 14 transiciones canónicas)
- [ ] CA3: `CanonicalWorkflow::next_status()` produce la misma salida que la función `next_status()` actual de `pipeline.rs`
- [ ] CA4: `CanonicalWorkflow::map_status_to_role()` produce la misma salida que `map_status_to_role()` actual
- [ ] CA5: `CanonicalWorkflow::canonical_column_order()` devuelve `["Draft", "Ready", "Tests Ready", "In Progress", "In Review", "Business Review", "Done", "Blocked", "Failed"]`
- [ ] CA6: `cargo test --lib state` pasa (nuevos tests para CanonicalWorkflow)
- [ ] CA7: El trait usa `&self` (no `&mut self`) — los workflows son inmutables durante la ejecución

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #1, recomendación #4).
