# STORY-021 — Reviewer: RECHAZO TÉCNICO

**Fecha**: 2026-05-05
**Actor**: Reviewer
**Transición**: In Review → In Progress (reject)

---

## Verificaciones realizadas

### ✅ Build
```
cargo build → Finished `dev` profile
```
Sin errores de compilación.

### ✅ Tests unitarios
```
cargo test → 463 passed, 0 failed, 1 ignored
```
El ignorado es `invoke_with_retry_fails_when_agent_not_installed` (requiere `pi` en PATH). Sin regresiones.

### ✅ Tests específicos STORY-021 (`story021` module)
42 tests, todos passing. Cubren:
- CA1: `function_exists_and_is_callable`, `accepts_string_slice`, `accepts_string_reference`
- CA2: `pi_standard_pattern`, `pi_standard_small_numbers`, `pi_standard_large_numbers`, `pi_standard_zero_output`
- CA3: `pi_alt_pattern`, `pi_alt_with_interleaving_text`, `pi_alt_multiline`
- CA4: `claude_standard_pattern`, `claude_standard_large`
- CA5: `claude_alt_pattern`, `claude_alt_with_text_between`
- CA6: `codex_pattern`, `codex_extra_whitespace`
- CA7: `opencode_pattern`, `opencode_with_extra_text`
- CA8: `commas_in_pi_pattern`, `commas_in_claude_pattern`, `commas_in_codex_pattern`, `commas_in_opencode_pattern`, `commas_in_both_numbers`, `multiple_commas_millions`
- CA9: `returns_none_for_irrelevant_text`, `returns_none_for_empty_string`, `returns_none_for_whitespace_only`, `returns_none_for_unknown_format`, `token_word_in_other_context`, `returns_none_for_numbers_without_keywords`
- CA10: (implícito, LazyLock en static variables)
- CA11: `returns_none_for_input_only`, `returns_none_for_output_only`, `decimal_numbers_not_recognized`, `negative_numbers_not_recognized`, `pattern_embedded_in_large_output`, `multiple_patterns_returns_first_match`, `all_patterns_stable_across_calls`, `deterministic_across_multiple_calls`, `whitespace_resilience`, `realistic_pi_output`, `realistic_claude_output`, `realistic_opencode_output`
- CA12: todos pasan

### ✅ Formato
```
cargo fmt --check → sin cambios pendientes
```

### ✅ Linting
```
cargo clippy -- -D warnings → 0 warnings
```

---

## ❌ RECHAZO: Violación de arquitectura

### Problema

```
tests/architecture.rs → architecture_layers_are_respected: FAILED
```

**Violación**: `src/infra/agent.rs:9`
```rust
use crate::domain::state::TokenCount;
```

La capa `infra/` **solo puede importar `config`** (regla R2/R4).  
Importar `domain::state::TokenCount` rompe la arquitectura en capas.

### Contexto

- STORY-020 definió `TokenCount` en `domain::state` y la arquitectura estaba **limpia** (11/11 tests pasaban).
- STORY-021 añade `use crate::domain::state::TokenCount` en `src/infra/agent.rs:9` para que `parse_token_count()` pueda construir y devolver `TokenCount`.
- Este import es el que causa el fallo del test de arquitectura.

### Fix sugerido

**Opción A (más simple)**: Definir `TokenCount` localmente en `infra/agent.rs`:
```rust
/// Conteo de tokens (copia local para evitar dependencia cross-layer).
#[derive(Debug, Clone, Default)]
struct TokenCount {
    pub input: u64,
    pub output: u64,
}
```
Eliminar `use crate::domain::state::TokenCount;`.

**Opción B**: Mover `TokenCount` a `src/types.rs` (el architecture test lo clasifica como capa `Cli`, accesible tanto para `domain` como `infra`).

### CA1

El CA1 pide explícitamente "importa TokenCount desde domain::state". Esto es contradictorio con la arquitectura. Si se elige la Opción A, el CA1 debe reinterpretarse como "parse_token_count devuelve una estructura equivalente a TokenCount con campos input/output: u64".

---

## Conclusión

❌ DoD técnico NO cumple — falla `tests/architecture.rs`.  
Transición a **In Progress** para corregir la violación de capas.
