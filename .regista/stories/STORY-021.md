# STORY-021: Implementar `parse_token_count()` con patrones multi-provider

## Status
**Blocked**

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
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 6: Tracking de tokens — Parseo).