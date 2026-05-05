# STORY-020: Dev Implementation Decision

**Date**: 2026-05-05
**Actor**: Developer

---

## Summary

Implementación de `TokenCount` y `token_usage` en `SharedState` (`domain/state.rs`).

## Changes

### `src/domain/state.rs`

1. **`TokenCount` struct** (línea ~78):
   ```rust
   #[derive(Debug, Clone, Default)]
   #[allow(dead_code)]
   pub struct TokenCount {
       pub input: u64,
       pub output: u64,
   }
   ```

2. **Campo `token_usage` en `SharedState`** (línea ~100):
   ```rust
   #[allow(dead_code)]
   pub token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>,
   ```

3. **Inicialización en `SharedState::new()`** (línea ~112):
   ```rust
   token_usage: Arc::new(RwLock::new(HashMap::new())),
   ```

## Design Decisions

- **`#[allow(dead_code)]`**: `TokenCount` y `token_usage` no son consumidos aún por otras partes del código. Serán utilizados por futuras historias de tracking de tokens (spec §6 — Acumulación). El atributo evita warnings de clippy sin suprimir la verificación global `-D warnings`.
- **`derive(Default)` en `TokenCount`**: los campos `u64` se inicializan a 0 automáticamente, que es el valor deseado para conteos vacíos.
- **`derive(Clone)` en `SharedState`**: el `Arc` se comparte entre clones, manteniendo la misma semántica que los otros campos del struct (compartición de estado, no copia profunda).

## Verification

| Check | Result |
|-------|--------|
| `cargo fmt --check` | ✅ OK |
| `cargo check` | ✅ OK |
| `cargo clippy -- -D warnings` | ✅ OK |
| `cargo test` (421 unit) | ✅ 421 passed, 0 failed, 1 ignored |
| `cargo test --test architecture` (11) | ✅ 11 passed |
| `domain::state::tests::story020` (21 tests) | ✅ 21 passed |

## Backward Compatibility

Ningún cambio rompe APIs existentes. `SharedState::new()` mantiene su firma de 3 parámetros, y la inicialización de `token_usage` es interna. `SharedState::default()` funciona correctamente gracias a `derive(Default)`.
