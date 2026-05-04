# Decision: Refinamiento de STORY-007

**Fecha**: 2026-05-04
**Actor**: PO
**Story**: STORY-007 — Definir trait `Workflow` + implementar `CanonicalWorkflow`

---

## Evaluación del Definition of Ready

### 1. Descripción clara y no ambigua ✅
La historia explica con precisión qué abstraer (next_status, map_status_to_role, column_order), por qué (preparar #04), y dónde colocarlo (src/domain/state.rs o src/domain/workflow.rs).

### 2. Criterios de aceptación específicos y testeables ✅
Los 7 CAs son verificables:

| CA | Verificable mediante |
|----|---------------------|
| CA1 | Compilación (trait Workflow con 3 métodos) |
| CA2 | Compilación (impl Workflow for CanonicalWorkflow) |
| CA3 | Test comparando salida de next_status() actual vs CanonicalWorkflow::next_status() |
| CA4 | Test comparando salida de map_status_to_role() actual vs CanonicalWorkflow::map_status_to_role() |
| CA5 | Test de igualdad de arrays |
| CA6 | `cargo test state` (28 tests pasan actualmente; se añadirán los de Workflow) |
| CA7 | El compilador fuerza que los métodos usen `&self` |

### 3. Dependencias identificadas ✅
"Ninguna" — la historia es autónoma. No depende de otras historias ni tiene bloqueadores.

### 4. Activity Log presente ✅
Registra la generación desde la auditoría de escalabilidad.

---

## Notas de refinamiento

### Comando de tests (CA6)
La historia menciona `cargo test --lib state`, pero el proyecto no tiene target `--lib` (es un binario). El comando real es `cargo test state`, que actualmente ejecuta 28 tests del módulo `domain::state`. Esto es una discrepancia menor en la documentación — el espíritu del CA es que los tests del módulo state/workflow pasen, lo cual es verificable.

### Ubicación del trait
La historia da flexibilidad: `src/domain/state.rs` o `src/domain/workflow.rs`. Se recomienda crear `src/domain/workflow.rs` para mantener separación de concerns (state.rs = datos/enum, workflow.rs = trait/comportamiento). Esto se alinea con la nota de board.rs que dice "Diseñado para ser resistente a #04".

### Validación cruzada con CA5
El orden canónico en CA5 coincide exactamente con el array `canonical_order` en `src/app/board.rs:print_human()`. Esto confirma que la historia captura correctamente el comportamiento actual.

### Comportamiento de next_status() actual
Verificado en `src/app/pipeline.rs:808-816`:
- Draft → Ready
- Ready → TestsReady
- TestsReady → InReview
- InProgress → InReview
- InReview → BusinessReview
- BusinessReview → Done
- Resto → current (no-op)

### Comportamiento de map_status_to_role() actual
Verificado en `src/app/pipeline.rs:821-828`:
- Draft, BusinessReview → "product_owner"
- Ready → "qa_engineer"
- TestsReady, InProgress → "developer"
- InReview → "reviewer"
- Resto → "product_owner" (fallback)

---

## Veredicto

**La historia cumple el Definition of Ready.** Se avanza de Draft → Ready.

La historia es pequeña, atómica, no tiene dependencias, y todos los CAs son verificables con el compilador y tests unitarios.
