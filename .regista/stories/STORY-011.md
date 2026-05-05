# STORY-011: Estado compartido con `Arc<RwLock<>>` para contadores del orchestrator

## Status
**Tests Ready**

## Epic
EPIC-04

## Descripción
Actualmente los contadores del orchestrator (`reject_cycles: HashMap<String, u32>`, `story_iterations: HashMap<String, u32>`, `story_errors: HashMap<String, String>`) son variables locales mutables que se pasan como `&mut` a través de la pila de llamadas. Para soportar paralelismo (#01), estos necesitan ser compartidos entre múltiples tareas de tokio. Hay que wrappearlos en `Arc<RwLock<HashMap<...>>>` y adaptar todas las funciones que los leen/escriben.

## Criterios de aceptación
- [ ] CA1: Los contadores se agrupan en un struct `SharedState` con campos `Arc<RwLock<HashMap<String, u32>>>` para `reject_cycles` y `story_iterations`, y `Arc<RwLock<HashMap<String, String>>>` para `story_errors`
- [ ] CA2: `process_story()` recibe `&SharedState` en lugar de `&mut HashMap<...>`
- [ ] CA3: Las lecturas usan `.read().unwrap()` y las escrituras usan `.write().unwrap()` (los locks son de corta duración)
- [ ] CA4: `apply_automatic_transitions()` accede a `reject_cycles` a través de `SharedState`
- [ ] CA5: `save_checkpoint()` clona el contenido bajo `read()` lock para serializar
- [ ] CA6: `cargo test --lib orchestrator` pasa (tests de pipeline adaptados)
- [ ] CA7: `cargo build` compila sin warnings

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #10.3, recomendación #7).
- 2026-05-05 | PO | Refinamiento: DoR verificado. Descripción alineada con hallazgo #10.3, 7 CAs específicos y testeables, sin dependencias. Nota: CA6 referencia `orchestrator` legado, adaptar a `app::pipeline`. Decisión documentada en decisions/STORY-011-po-refinement.md.
- 2026-05-05 | QA | Verificación de cobertura: tests existentes cubren CA1-CA5 (20 tests en 3 módulos). CA1: `SharedState` struct (3 tests en `domain/state.rs`). CA2: `process_story` con `&SharedState` (3 tests en `app/pipeline.rs`). CA3: locks `.read()/.write()` (6 tests entre `state.rs` y `pipeline.rs`). CA4: `apply_automatic_transitions` accede a `reject_cycles` via SharedState (4 tests en `pipeline.rs`). CA5: `save_checkpoint` clona bajo `read()` lock (4 tests entre `pipeline.rs` y `checkpoint.rs`). CA6/CA7: verificación manual del Developer. Los tests son TDD: no compilan aún porque las firmas de las funciones no han sido actualizadas — el Developer debe cambiar `&mut HashMap<...>` → `&SharedState`. Decisión documentada en decisions/STORY-011-qa-engineer-20260505T010000.md.
