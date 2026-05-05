# STORY-016: Centralizar `extract_numeric` + eliminar duplicación en 4 módulos

## Status
**Draft**

## Epic
EPIC-06

## Descripción
La función `extract_numeric(id: &str) -> u32` está duplicada idénticamente en `src/app/pipeline.rs`, `src/domain/deadlock.rs`, y `src/app/board.rs`. Debe existir una única copia en `src/domain/story.rs` (como `pub(crate) fn extract_numeric`) o como método asociado `Story::numeric_id(&self) -> u32`, y todos los callers deben usar esa copia. Esto elimina una fuente de bugs por divergencia y reduce el acoplamiento.

## Criterios de aceptación
- [ ] CA1: Existe una única definición de `extract_numeric` en `src/domain/story.rs` como función pública del crate
- [ ] CA2: Las copias duplicadas en `pipeline.rs`, `deadlock.rs`, y `board.rs` se eliminan
- [ ] CA3: Todos los usos de `extract_numeric` en el código base importan desde `crate::domain::story::extract_numeric`
- [ ] CA4: `cargo build` compila sin warnings
- [ ] CA5: `cargo test` pasa todos los tests existentes
- [ ] CA6: `cargo clippy -- -D warnings` no reporta nuevos issues

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #11.1, recomendación #8).
