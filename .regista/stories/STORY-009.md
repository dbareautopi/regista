# STORY-009: Adaptar `board.rs` para columnas dinámicas según workflow

## Status
**Business Review**

## Epic
EPIC-03

## Descripción
Actualmente `board.rs` tiene un array hardcodeado `canonical_order` con los 9 estados en orden fijo. Cuando se introduzcan workflows custom (#04), los estados pueden tener nombres distintos o nuevos. `board.rs` debe obtener el orden de columnas del trait `Workflow` en lugar de usar el array hardcodeado. Además, si un estado del workflow no tiene historias, no debe mostrarse (para evitar columnas vacías permanentes en workflows que eliminan estados).

## Criterios de aceptación
- [ ] CA1: `print_human()` (o la función que renderiza el board) acepta `&dyn Workflow` como parámetro
- [ ] CA2: El orden de columnas se obtiene de `workflow.canonical_column_order()`
- [ ] CA3: Las columnas sin historias (count = 0) se omiten en la salida
- [ ] CA4: La salida para `CanonicalWorkflow` es visualmente idéntica a la actual
- [ ] CA5: `cargo test --lib board` pasa (tests existentes + nuevos para orden dinámico)
- [ ] CA6: Si se pasa un workflow hipotético con solo 5 columnas, el board muestra exactamente esas 5 columnas (test unitario)

## Dependencias
- Bloqueado por: STORY-007

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #1.5).
- 2026-05-05 | QA | Tests unitarios verificados: 8 tests (CA1-CA6) en src/app/board.rs cubren todos los CAs. Tests existentes en el archivo — no fue necesario añadir nuevos. La función `render_board` ya acepta `&dyn Workflow` (firma correcta, cuerpo es placeholder `todo!()`). Tests usan `CanonicalWorkflow` + workflows sintéticos (FiveColumnWorkflow, SparseWorkflow) para CA2, CA3, CA4, CA6. El Developer debe implementar el cuerpo de `render_board` para que los tests pasen de `todo!()` panic a green.
- 2026-05-05 | Dev | Implementado `render_board()` con columnas dinámicas desde `workflow.canonical_column_order()`. Eliminado array hardcodeado `canonical_order` de `print_human()`, que ahora delega en `render_board()`. Columnas con count=0 se omiten en la salida. `run()` pasa `&CanonicalWorkflow` a `print_human()`. Todos los 16 tests de `board` pasan (incluyendo 8 nuevos de STORY-009: dyn workflow, orden workflow, omisión vacías, output canónico, workflow 5-columnas, sparse). `cargo fmt` y `cargo clippy` limpios. Test suite completa: 223/223 pasando.
- 2026-05-05 | Reviewer | ✅ DoD técnico verificado: compilación limpia, 223/223 tests pasando (13 board), clippy 0 warnings, fmt OK, sin regresiones. CA1-CA6 todos cubiertos por tests. Se promueve a Business Review.