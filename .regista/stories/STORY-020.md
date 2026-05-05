# STORY-020: Añadir `TokenCount` y `token_usage` a `SharedState`

## Status
**Ready**

## Epic
EPIC-07

## Descripción
Añadir el struct `TokenCount { input: u64, output: u64 }` y el campo `token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>` a `SharedState` en `domain/state.rs`. Cada clave del HashMap es un `story_id` (String) y el valor es un vector con los conteos de cada invocación de agente sobre esa historia (incluyendo reintentos). El campo debe ser inicializado correctamente en `SharedState::new()` y clonable (comparte el Arc).

## Criterios de aceptación
- [ ] CA1: Existe `pub struct TokenCount { pub input: u64, pub output: u64 }` en `domain/state.rs`
- [ ] CA2: `TokenCount` implementa `Debug`, `Clone`, y `Default`
- [ ] CA3: `SharedState` tiene campo `pub token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>`
- [ ] CA4: `SharedState::new()` inicializa `token_usage` como `Arc::new(RwLock::new(HashMap::new()))`
- [ ] CA5: `SharedState` sigue implementando `Clone` (comparte el mismo `Arc` de `token_usage`)
- [ ] CA6: `cargo check --lib` compila sin errores en el módulo `domain::state`
- [ ] CA7: `cargo test --lib domain::state` pasa todos los tests existentes
- [ ] CA8: Test unitario verifica que `token_usage` se puede leer y escribir concurrentemente (usando `read()` y `write()` del `RwLock`)
- [ ] CA9: Test unitario verifica que clonar `SharedState` comparte el mismo `token_usage` (escribir en un clone → visible en el otro)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 6: Tracking de tokens — Acumulación).
- 2026-05-05 | PO | Refinamiento: validada contra DoR. Descripción clara, 9 CAs específicos y testeables, sin dependencias. Alineada con spec §6. Transición Draft → Ready. Ver .regista/decisions/STORY-020-po-refinement-2026-05-05.md.
