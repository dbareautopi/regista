# STORY-021: Decisiones de implementación — Dev

**Fecha**: 2026-05-05
**Autor**: Developer

---

## 1. Estrategia de patrones regex

Se definieron 6 `static LazyLock<Regex>` (uno por provider/patrón):

| Static | Provider | Patrón |
|--------|----------|--------|
| `PI_STANDARD` | pi | `Tokens used:\s+([\d,]+)\s+input,\s+([\d,]+)\s+output` |
| `PI_ALT` | pi (alt) | `([\d,]+)\s+input\s+tokens[\s\S]*?([\d,]+)\s+output\s+tokens` |
| `CLAUDE_STANDARD` | Claude Code | `Token usage:\s+([\d,]+)\s+input,\s+([\d,]+)\s+output` |
| `CLAUDE_ALT` | Claude Code (alt) | `Input tokens:\s+([\d,]+)[\s\S]*?Output tokens:\s+([\d,]+)` |
| `CODEX` | Codex | `Tokens:\s+([\d,]+)\s+in\s+/\s+([\d,]+)\s+out` |
| `OPENCODE` | OpenCode | `([\d,]+)\s+prompt\s+tokens[\s\S]*?([\d,]+)\s+completion\s+tokens` |

### Decisiones de diseño:

- **`[\d,]+` en vez de `\d+`**: para capturar números con comas (CA8). Se hace `str::replace(",", "")` antes del parseo.
- **`[\s\S]*?` (non-greedy)**: para los patrones con texto intermedio entre input y output, porque `.` no matchea `\n` por defecto en la crate `regex`. Esto permite texto multilínea.
- **`\s+` flexible**: permite cualquier cantidad de whitespace entre tokens (requerido por el test `whitespace_resilience`).
- **Sin flag case-insensitive**: los patrones matchean con el case exacto de los tests.
- **Orden de evaluación**: pi → pi_alt → Claude → Claude_alt → Codex → OpenCode. El primero que haga match gana.

## 2. Manejo de edge cases

- **Negativos**: `-5` no matchea porque `[\d,]+` no incluye `-`
- **Decimales**: `1.5` captura solo `1`, luego falla al esperar `\s+input`
- **Solo input / solo output**: requieren ambas capturas → `None`
- **Múltiples patrones**: devuelve el primer match encontrado
- **Comas múltiples**: `1,234,567` → `1234567` (strip total de comas)

## 3. Convenciones del proyecto

- `#[allow(dead_code)]` en statics y función porque clippy los marca en el binario (solo se usan en tests actualmente). El proyecto ya usa este patrón (e.g., `TokenCount`, `token_usage`).
- `LazyLock` para compilación única (CA10), siguiendo el patrón existente de `RUNTIME`.
- La violación de arquitectura R2 (`infra/agent.rs` importa `crate::domain::state::TokenCount`) es pre-existente de STORY-020. No se corrige aquí (fuera de alcance).

## 4. Resultados

- **42/42** tests `story021` pasan
- **463/463** tests totales pasan (0 fallos, 1 ignorado)
- **Build release**: OK
- **cargo fmt**: OK
- **cargo clippy**: OK (con `#[allow(dead_code)]`)
