# STORY-021 — PO: Validación de Negocio → Done

**Fecha**: 2026-05-05
**Actor**: Product Owner
**Transición**: Business Review → Done (validate)

---

## Verificación de valor de negocio

### ¿Qué se pidió?
Implementar `parse_token_count()` en `infra/agent.rs` para extraer conteos de tokens de la salida textual de cualquier provider soportado (pi, Claude Code, Codex, OpenCode). Esto es la base para el tracking de costes transparente (#12 del roadmap) y los logs multi-provider.

### ¿Qué se entregó?
- Función `pub fn parse_token_count(text: &str) -> Option<TokenCount>` en `src/infra/agent.rs`.
- 6 `LazyLock<Regex>` estáticos con patrones específicos por provider:
  - **pi estándar**: `Tokens used: (\d+) input, (\d+) output`
  - **pi alt**: `(\d+) input tokens ... (\d+) output tokens`
  - **Claude Code estándar**: `Token usage: (\d+) input, (\d+) output`
  - **Claude Code alt**: `Input tokens: (\d+) ... Output tokens: (\d+)`
  - **Codex**: `Tokens: (\d+) in / (\d+) out`
  - **OpenCode**: `(\d+) prompt tokens ... (\d+) completion tokens`
- `TokenCount` definido localmente como struct con `input: u64, output: u64` (Debug, Clone, Default).
- Manejo de comas (`1,234` → `1234`) con `str::replace(",", "")`.
- Retorno `None` para texto sin patrones reconocidos.
- 42 tests unitarios (`story021`) cubriendo los 12 CAs.

### Verificación de CAs

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
| CA10 | `LazyLock<Regex>` (6 estáticos) | ✅ |
| CA11 | 42 tests unitarios cubren todos los patrones y edge cases | ✅ |
| CA12 | Tests pasan | ✅ |

### Verificaciones técnicas

| Verificación | Resultado |
|---|---|
| `cargo build` | ✅ Limpio |
| `cargo test` | ✅ 463 pasan, 0 fallos, 1 ignorado |
| `cargo test --test architecture` | ✅ 11/11 pasan — sin violaciones R1-R5 |
| `cargo clippy -- -D warnings` | ✅ 0 warnings |
| `cargo fmt --check` | ✅ Formato correcto |

### Nota sobre CA1

El CA1 original decía «importa TokenCount desde domain::state». Esto fue detectado por el Reviewer como violación de arquitectura R2/R4 (infra solo puede importar config). El Dev corrigió definiendo `TokenCount` localmente en `infra/agent.rs`. La reinterpretación del CA1 es correcta: la estructura equivalente cumple el mismo propósito funcional sin romper la arquitectura.

---

## Conclusión

✅ **APROBADO → Done**. El valor de negocio está entregado:
1. Tracking transparente de tokens para los 4 providers soportados (pi, Claude Code, Codex, OpenCode).
2. Base sólida para cost tracking (#12) y logs multi-provider.
3. Implementación defensiva: None seguro cuando el formato no se reconoce.
4. Arquitectura limpia: 0 violaciones de capas.
5. Cobertura de tests exhaustiva: 42 tests que validan cada patrón.
