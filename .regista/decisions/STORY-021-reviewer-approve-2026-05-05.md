# STORY-021 — Reviewer Approval — 2026-05-05

## Resultado
✅ APROBADO → **Business Review**

## Verificaciones realizadas

| Verificación | Resultado |
|---|---|
| `cargo build` | ✅ Limpio (sin errores ni warnings) |
| `cargo test` | ✅ 463/463 pasan, 0 fallos, 1 ignorado (requiere `pi` instalado) |
| `cargo test --test architecture` | ✅ 11/11 pasan — sin violaciones R1-R5 |
| `cargo clippy -- -D warnings` | ✅ 0 warnings |
| `cargo fmt --check` | ✅ Formato correcto |

## Validación de CAs

| CA | Descripción | Estado |
|---|---|---|
| CA1 | `parse_token_count` pública en `infra/agent.rs`, `TokenCount` local | ✅ |
| CA2 | Patrón pi: `Tokens used: N input, M output` | ✅ |
| CA3 | Patrón pi alt: `N input tokens ... M output tokens` | ✅ |
| CA4 | Patrón Claude: `Token usage: N input, M output` | ✅ |
| CA5 | Patrón Claude alt: `Input tokens: N ... Output tokens: M` | ✅ |
| CA6 | Patrón Codex: `Tokens: N in / M out` | ✅ |
| CA7 | Patrón OpenCode: `N prompt tokens ... M completion tokens` | ✅ |
| CA8 | Números con comas: `1,234` → `1234` | ✅ |
| CA9 | Devuelve `None` para texto irrelevante | ✅ |
| CA10 | `LazyLock<Regex>` (6 estáticos, compilación única) | ✅ |
| CA11 | 42 tests unitarios cubren cada patrón y casos límite | ✅ |
| CA12 | `cargo test --lib infra::agent` pasa | ✅ |

## Corrección de la arquitectura (ciclo anterior)

En el ciclo anterior, el Reviewer rechazó por violación de capas R2/R4:
- `infra/agent.rs` importaba `crate::domain::state::TokenCount`
- `infra` solo puede importar `config`

**Fix aplicado por el Dev**: `TokenCount` se definió localmente en `infra/agent.rs` como struct con campos `input: u64, output: u64` y derivaciones `Debug, Clone, Default`. Se eliminó el import cross-layer.

`tests/architecture.rs` ahora confirma 0 violaciones (11/11).

## Notas

- La implementación usa 6 `LazyLock<Regex>` estáticos con `#[allow(dead_code)]` (aún no se invocan desde el pipeline; es para uso futuro en cost tracking #12).
- Todos los patrones prueban primero el estándar de cada provider, luego el alternativo.
- `str::replace(",", "")` maneja números con formato `1,234` → `1234`.
- El orden de prueba de patrones es: PI_STANDARD → PI_ALT → CLAUDE_STANDARD → CLAUDE_ALT → CODEX → OPENCODE. El primer match gana.
