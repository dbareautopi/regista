# STORY-007: Definir trait `Workflow` + implementar `CanonicalWorkflow`

## Status
**In Review**

## Epic
EPIC-03

## Descripción
El workflow actual está hardcodeado en múltiples funciones: `next_status()`, `map_status_to_role()`, y el orden canónico de columnas en `board.rs`. Para preparar el terreno para #04 (workflow configurable), hay que extraer un trait `Workflow` que encapsule estas decisiones, y proveer una implementación por defecto `CanonicalWorkflow` que replique exactamente el comportamiento actual. Esto permite que el resto del código se migre a `&dyn Workflow` sin cambiar el comportamiento.

## Criterios de aceptación
- [x] CA1: Existe un trait `Workflow` en `src/domain/state.rs` (o nuevo `src/domain/workflow.rs`) con al menos estos métodos:
  - `fn next_status(&self, current: Status) -> Status`
  - `fn map_status_to_role(&self, status: Status) -> &'static str`
  - `fn canonical_column_order(&self) -> &[&'static str]`
- [x] CA2: Existe un struct `CanonicalWorkflow` que implementa `Workflow` con el comportamiento actual (el de las 14 transiciones canónicas)
- [x] CA3: `CanonicalWorkflow::next_status()` produce la misma salida que la función `next_status()` actual de `pipeline.rs`
- [x] CA4: `CanonicalWorkflow::map_status_to_role()` produce la misma salida que `map_status_to_role()` actual
- [x] CA5: `CanonicalWorkflow::canonical_column_order()` devuelve `["Draft", "Ready", "Tests Ready", "In Progress", "In Review", "Business Review", "Done", "Blocked", "Failed"]`
- [x] CA6: `cargo test --lib state` pasa (nuevos tests para CanonicalWorkflow)
- [x] CA7: El trait usa `&self` (no `&mut self`) — los workflows son inmutables durante la ejecución

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #1, recomendación #4).
- 2026-05-04 | PO | Refinamiento: historia validada contra DoR. Descripción clara, 7 CAs testeables, sin dependencias. Decisión documentada en .regista/decisions/story-007-refinement.md. Avanza a Ready.
- 2026-05-04 | QA | Verificación de tests existentes en src/domain/workflow.rs: 20 tests unitarios cubren CA1-CA7 exhaustivamente. No se requieren tests adicionales. Decisión documentada en .regista/decisions/STORY-007-qa-engineer-20260504T230000.md. Avanza a Tests Ready.
- 2026-05-04 | Dev | Implementación de trait Workflow + CanonicalWorkflow en src/domain/workflow.rs. Replican next_status(), map_status_to_role() y canonical_column_order() del pipeline actual. 20 tests pasan. Añadido #[allow(dead_code)] por clippy (items públicos sin uso externo aún). cargo build, cargo test (208 tests), cargo clippy y cargo fmt pasan. Decisión documentada en .regista/decisions/STORY-007-dev-20260504T233000.md. Avanza a In Review.
