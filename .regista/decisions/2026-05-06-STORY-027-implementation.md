# STORY-027: Decisiones de implementación

## Fecha
2026-05-06

## Resumen de cambios

### 1. `format_agent_line_with_model()` (CA5-CA6)
- Función a nivel de módulo en `app/pipeline.rs` (línea ~1053)
- Formato: `🎯 <label> | <story_id> | <provider> [<modelo>]`
- El modelo se obtiene via `AgentsConfig::model_for_role(role, &instruction_path)` donde `instruction_path` viene de `AgentsConfig::skill_for_role(role)`
- Integrada en `process_story()` para loguear cada invocación de agente con `tracing::info!`

### 2. Acumulación de tokens post-agente (CA7-CA8)
- Tras `invoke_with_retry()`, se concatena `agent_result.stdout + agent_result.stderr`
- Se parsea con `agent::parse_token_count(&combined)` que busca patrones de tokens en la salida
- Si se encuentran tokens, se crea un `TokenCount { input, output }` y se acumula en `state.token_usage` bajo el `story_id` (push al `Vec`)
- Se usa `.or_default()` (clippy-friendly) en lugar de `.or_insert_with(Vec::new)`

### 3. Diff post-agente (CA1-CA4)
- En `process_story()`, tras agente exitoso y hooks, si `cfg.git.enabled && !compact`:
  - Si hay `prev_hash` del snapshot: `git diff --stat <hash> HEAD`
  - Si no hay hash (ej. primer commit): `git diff --stat HEAD~1 HEAD`
- La salida se loguea con `tracing::info!("📁 Archivos modificados:")` y una línea por archivo
- En modo compacto (`--compact`) o con `git.enabled = false` se omite
- En dry-run no aplica (el flujo pasa por `run_dry`, no por `process_story`)

### 4. Resumen final enriquecido (CA9-CA10)
- En `run_real()`, tras el loop principal, se emite un bloque de cierre con:
  - Conteo de historias por estado (Total, Done, Failed, Blocked, Draft)
  - IDs de historias fallidas entre paréntesis
  - Iteraciones totales, tiempo transcurrido (formato Xh Ym Zs)
  - Tokens totales sumando `state.token_usage` via `saturating_add`
- Se usa `chrono::Utc::now()` para el timestamp
- El bloque usa bordes decorativos con `═══`

## Correcciones necesarias
- Se añadió el parámetro `compact: bool` a `process_story()` para soportar CA3
- Se corrigieron 6 llamadas a `process_story()` en tests que no pasaban el nuevo parámetro `compact` (se pasa `false` en esos tests ya que prueban early-return para estados no-procesables)
- Se corrigieron 2 warnings de clippy: `or_insert_with(Vec::new)` → `or_default()`, y `map(...).flatten()` → `and_then(...)`

## Resultados
- `cargo build`: compila sin errores
- `cargo test`: 520 tests pasan, 1 ignorado (requiere pi en PATH), 0 fallan
- `cargo clippy -- -D warnings`: 0 warnings
- `cargo fmt -- --check`: sin cambios pendientes
