# STORY-020: Validación de negocio (PO)

**Fecha**: 2026-05-05  
**Actor**: PO (Product Owner)  
**Transición**: Business Review → Done

---

## Verificación de Criterios de Aceptación

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `pub struct TokenCount { pub input: u64, pub output: u64 }` en `domain/state.rs` | ✅ |
| CA2 | `TokenCount` implementa `Debug`, `Clone`, `Default` | ✅ |
| CA3 | `SharedState` tiene campo `pub token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>` | ✅ |
| CA4 | `SharedState::new()` inicializa `token_usage` como `Arc::new(RwLock::new(HashMap::new()))` | ✅ |
| CA5 | `SharedState` sigue implementando `Clone` (comparte el mismo `Arc` de `token_usage`) | ✅ |
| CA6 | `cargo check --lib` compila sin errores en el módulo `domain::state` | ✅ |
| CA7 | `cargo test --lib domain::state` pasa todos los tests existentes | ✅ |
| CA8 | Test unitario verifica lectura/escritura concurrente de `token_usage` | ✅ |
| CA9 | Test unitario verifica que clonar `SharedState` comparte el mismo `token_usage` | ✅ |

## Evidencia de ejecución

- `cargo test domain::state`: **52 passed, 0 failed, 0 ignored** (incluye 21 tests story020)
- `cargo test --test architecture`: **11 passed, 0 failed**
- `cargo check`: limpio
- 0 regresiones en módulos existentes

## Evaluación de valor de negocio

La historia entrega exactamente lo solicitado en la spec §6 (Tracking de tokens — Acumulación):

1. **`TokenCount`**: struct con `input: u64` y `output: u64` para modelar el conteo de tokens de cada invocación de agente.
2. **`token_usage` en `SharedState`**: `HashMap<String, Vec<TokenCount>>` indexado por `story_id`, permitiendo acumular múltiples invocaciones (incluyendo reintentos) sobre la misma historia.
3. **Concurrencia**: `Arc<RwLock<>>` garantiza acceso seguro desde múltiples tareas (preparado para paralelismo #01).
4. **Clonabilidad**: los clones comparten el mismo `Arc`, igual que el resto de campos de `SharedState`.

La anotación `#[allow(dead_code)]` en `TokenCount` y `token_usage` es correcta: estos tipos serán consumidos por historias futuras de tracking de tokens. No hay warnings superfluos.

## Decisión

**Done**. El valor de negocio se cumple completamente. No se requieren correcciones.
