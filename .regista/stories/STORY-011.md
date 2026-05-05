# STORY-011: Estado compartido con `Arc<RwLock<>>` para contadores del orchestrator

## Status
**Business Review**

## Epic
EPIC-04

## Descripción
Actualmente los contadores del orchestrator (`reject_cycles: HashMap<String, u32>`, `story_iterations: HashMap<String, u32>`, `story_errors: HashMap<String, String>`) son variables locales mutables que se pasan como `&mut` a través de la pila de llamadas. Para soportar paralelismo (#01), estos necesitan ser compartidos entre múltiples tareas de tokio. Hay que wrappearlos en `Arc<RwLock<HashMap<...>>>` y adaptar todas las funciones que los leen/escriben.

## Criterios de aceptación
- [x] CA1: Los contadores se agrupan en un struct `SharedState` con campos `Arc<RwLock<HashMap<String, u32>>>` para `reject_cycles` y `story_iterations`, y `Arc<RwLock<HashMap<String, String>>>` para `story_errors`
- [x] CA2: `process_story()` recibe `&SharedState` en lugar de `&mut HashMap<...>`
- [x] CA3: Las lecturas usan `.read().unwrap()` y las escrituras usan `.write().unwrap()` (los locks son de corta duración)
- [x] CA4: `apply_automatic_transitions()` accede a `reject_cycles` a través de `SharedState`
- [x] CA5: `save_checkpoint()` clona el contenido bajo `read()` lock para serializar
- [x] CA6: `cargo test --lib orchestrator` pasa (tests de pipeline adaptados)
- [x] CA7: `cargo build` compila sin warnings

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #10.3, recomendación #7).
- 2026-05-05 | PO | Refinamiento: DoR verificado. Descripción alineada con hallazgo #10.3, 7 CAs específicos y testeables, sin dependencias. Nota: CA6 referencia `orchestrator` legado, adaptar a `app::pipeline`. Decisión documentada en decisions/STORY-011-po-refinement.md.
- 2026-05-05 | QA | Verificación de cobertura: tests existentes cubren CA1-CA5 (20 tests en 3 módulos). CA1: `SharedState` struct (3 tests en `domain/state.rs`). CA2: `process_story` con `&SharedState` (3 tests en `app/pipeline.rs`). CA3: locks `.read()/.write()` (6 tests entre `state.rs` y `pipeline.rs`). CA4: `apply_automatic_transitions` accede a `reject_cycles` via SharedState (4 tests en `pipeline.rs`). CA5: `save_checkpoint` clona bajo `read()` lock (4 tests entre `pipeline.rs` y `checkpoint.rs`). CA6/CA7: verificación manual del Developer. Los tests son TDD: no compilan aún porque las firmas de las funciones no han sido actualizadas — el Developer debe cambiar `&mut HashMap<...>` → `&SharedState`. Decisión documentada en decisions/STORY-011-qa-engineer-20260505T010000.md.
- 2026-05-05 | Dev | Implementación completada. Cambios: (1) Añadido constructor `SharedState::new()` en `domain/state.rs` para facilitar creación desde checkpoint. (2) `apply_automatic_transitions()`: firma cambiada de `&mut HashMap<String, u32>` a `&SharedState`, lectura de `reject_cycles` vía `state.reject_cycles.read().unwrap()`. (3) `process_story()`: firma cambiada de `&mut HashMap<String, u32>` a `&SharedState`, escritura de `reject_cycles` vía `state.reject_cycles.write().unwrap()` con lock de corta duración (guard capturado en scope y droppeado explícitamente). (4) `save_checkpoint()`: firma simplificada a `(project_root, iteration, &SharedState)`, clona los 3 mapas bajo `.read().unwrap()` locks. (5) `run_real()`: variables locales reemplazadas por `SharedState`, propagado `&state` a todas las funciones. (6) Tests `story008` actualizados para usar `&SharedState` en lugar de `&mut HashMap`. 243 tests pasan, `cargo build` limpio, `cargo clippy` sin warnings, `cargo fmt` aplicado. Decisión documentada en decisions/STORY-011-dev-20260505T000000.md.
- 2026-05-05 | Reviewer | Revisión técnica aprobada → Business Review. Verificaciones: (1) `cargo build` compila sin errores. (2) `cargo test`: 243 unit tests + 11 architecture tests pasan, 0 fallos. (3) `cargo clippy -- -D warnings` sin warnings. (4) `cargo fmt --check` sin diferencias. Los 7 CAs están satisfechos: `SharedState` con 3 `Arc<RwLock<>>` en `domain/state.rs:78-84`, `process_story` y `apply_automatic_transitions` usan `&SharedState` con locks de corta duración, `save_checkpoint` clona bajo `read()` lock. Sin regresiones. Decisión documentada en decisions/STORY-011-reviewer-20260505T000000.md.
