# STORY-009: Adaptar `board.rs` para columnas dinámicas según workflow

## Status
**Ready**

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