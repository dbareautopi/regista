# STORY-020: Añadir `TokenCount` y `token_usage` a `SharedState`

## Status
**In Review**

## Epic
EPIC-07

## Descripción
Añadir el struct `TokenCount { input: u64, output: u64 }` y el campo `token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>` a `SharedState` en `domain/state.rs`. Cada clave del HashMap es un `story_id` (String) y el valor es un vector con los conteos de cada invocación de agente sobre esa historia (incluyendo reintentos). El campo debe ser inicializado correctamente en `SharedState::new()` y clonable (comparte el Arc).

## Criterios de aceptación
- [x] CA1: Existe `pub struct TokenCount { pub input: u64, pub output: u64 }` en `domain/state.rs`
- [x] CA2: `TokenCount` implementa `Debug`, `Clone`, y `Default`
- [x] CA3: `SharedState` tiene campo `pub token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>`
- [x] CA4: `SharedState::new()` inicializa `token_usage` como `Arc::new(RwLock::new(HashMap::new()))`
- [x] CA5: `SharedState` sigue implementando `Clone` (comparte el mismo `Arc` de `token_usage`)
- [x] CA6: `cargo check --lib` compila sin errores en el módulo `domain::state`
- [x] CA7: `cargo test --lib domain::state` pasa todos los tests existentes
- [x] CA8: Test unitario verifica que `token_usage` se puede leer y escribir concurrentemente (usando `read()` y `write()` del `RwLock`)
- [x] CA9: Test unitario verifica que clonar `SharedState` comparte el mismo `token_usage` (escribir en un clone → visible en el otro)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 6: Tracking de tokens — Acumulación).
- 2026-05-05 | PO | Refinamiento: validada contra DoR. Descripción clara, 9 CAs específicos y testeables, sin dependencias. Alineada con spec §6. Transición Draft → Ready. Ver .regista/decisions/STORY-020-po-refinement-2026-05-05.md.
- 2026-05-05 | QA | Tests verificados: 20 tests unitarios ya existentes en `domain::state.rs::tests::story020` cubren CA1-CA5 y CA8-CA9. CA6 (cargo check) y CA7 (regresión) son verificaciones de build, no tests unitarios. No se requieren tests adicionales. Transición Ready → Tests Ready. Ver .regista/decisions/STORY-020-qa-tests-2026-05-05.md.
- 2026-05-05 | Dev | Implementación completa: `TokenCount` con `Debug`, `Clone`, `Default` + campo `token_usage` en `SharedState`. Todos los 21 tests story020 pasan (421 total, 0 fallos). `cargo fmt`, `cargo check`, `cargo clippy -D warnings` limpios. Añadido `#[allow(dead_code)]` en `TokenCount` y `token_usage` (serán consumidos por futuras historias de tracking). Transición Tests Ready → In Review. Ver .regista/decisions/STORY-020-dev-implement-2026-05-05.md.
