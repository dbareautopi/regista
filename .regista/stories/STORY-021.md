# STORY-021: Implementar `parse_token_count()` con patrones multi-provider

## Status
**In Review**

## Epic
EPIC-07

## Descripción
Implementar la función `pub fn parse_token_count(text: &str) -> Option<TokenCount>` en `infra/agent.rs` que examina un string (stdout + stderr combinados del agente) y extrae el conteo de tokens de entrada y salida usando patrones regex específicos para cada provider. Los patrones se compilan una sola vez con `LazyLock<Regex>`.

## Criterios de aceptación
- [ ] CA1: `parse_token_count` es pública y está en `infra/agent.rs`, importa `TokenCount` desde `domain::state`
- [ ] CA2: Reconoce patrón pi: `Tokens used: (\d+) input, (\d+) output` → parsea "1234" y "567"
- [ ] CA3: Reconoce patrón pi alternativo: `(\d+) input tokens.*(\d+) output tokens`
- [ ] CA4: Reconoce patrón Claude Code: `Token usage: (\d+) input, (\d+) output`
- [ ] CA5: Reconoce patrón Claude Code alternativo: `Input tokens: (\d+).*Output tokens: (\d+)`
- [ ] CA6: Reconoce patrón Codex: `Tokens: (\d+) in / (\d+) out`
- [ ] CA7: Reconoce patrón OpenCode: `(\d+) prompt tokens.*(\d+) completion tokens`
- [ ] CA8: Maneja números con comas: `1,234` → `1234` (usa `str::replace(",", "")`)
- [ ] CA9: Devuelve `None` para texto sin patrones de tokens reconocibles
- [ ] CA10: Los `Regex` se compilan con `LazyLock` (no en cada llamada)
- [ ] CA11: Tests unitarios cubren cada patrón de provider y casos límite (comas, texto irrelevante, solo input sin output)
- [ ] CA12: `cargo test --lib infra::agent` pasa todos los tests nuevos y existentes

## Dependencias
- Bloqueado por: STORY-020

## Activity Log
- 2026-05-05 | Dev | CORRECCIÓN tras rechazo Reviewer (violación R2/R4). Eliminado `use crate::domain::state::TokenCount` en src/infra/agent.rs:9. Definido struct TokenCount localmente en infra/agent.rs con campos input: u64, output: u64 y derivaciones Debug/Clone/Default. Arquitectura limpia: 11/11 architecture tests pasan. Build Ok, 463 tests pasan, fmt Ok, clippy Ok. → In Review.
- 2026-05-05 | Reviewer | RECHAZO TÉCNICO → In Progress. Violación de arquitectura R2/R4: src/infra/agent.rs:9 importa crate::domain::state::TokenCount. Infra solo puede importar config. tests/architecture.rs FAILS (1 violation). El resto OK: build limpio, 463 tests pasan, fmt Ok, clippy Ok. Ver decisión en .regista/decisions/STORY-021-reviewer-reject-2026-05-05.md. Fix sugerido: definir TokenCount localmente en infra/agent.rs (struct con input/output: u64) y eliminar el import cross-layer. O mover TokenCount a src/types.rs (capa Cli, accesible para ambos).
- 2026-05-05 | Dev | Implementado parse_token_count() con 6 LazyLock<Regex> (pi estándar, pi alt, Claude estándar, Claude alt, Codex, OpenCode). Captura con [\d,]+ y strip de comas para CA8. 42/42 tests story021 pasan. 463/463 tests totales pasan (0 fallos). Build release OK. Nota: test de arquitectura falla por violación R2 pre-existente (STORY-020: use crate::domain::state::TokenCount en infra/agent.rs).
- 2026-05-05 | QA | Verificados 47 tests unitarios existentes en infra::agent::story021 — cubren los 12 CAs (6 patrones multi-provider, comas, None, LazyLock, casos límite). Sin adiciones necesarias.
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 6: Tracking de tokens — Parseo).