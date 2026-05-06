# STORY-027: Diff post-agente + acumulación de tokens + resumen final enriquecido

## Status
**Done**

## Epic
EPIC-10

## Descripción
Integrar tres funcionalidades en `app/pipeline.rs`:

1. **Diff de archivos post-agente**: tras cada `process_story()` exitosa, si `git.enabled = true` y el modo es detallado (`!compact`), ejecutar `git diff --stat` contra el commit del snapshot previo y loguear los archivos modificados con `📁 Archivos modificados`.

2. **Log de agente con modelo**: cada línea de agente muestra el modelo resuelto: `🎯 Dev (implement) | STORY-003 | pi [qwen2.5-coder]`.

3. **Acumulación de tokens y resumen final**: tras cada invocación de agente, parsear tokens con `parse_token_count()` y acumular en `SharedState.token_usage`. Al terminar el pipeline, emitir un resumen final enriquecido con conteo total de tokens y desglose por historia.

## Criterios de aceptación
- [x] CA1: Tras cada `process_story()` exitosa en modo detallado, se ejecuta `git diff --stat <snapshot_hash> HEAD` (o `HEAD~1 HEAD` si no hay hash)
- [x] CA2: La salida de `git diff --stat` se loguea con `tracing::info!` bajo el encabezado `📁 Archivos modificados:`
- [x] CA3: En modo compacto (`--compact`), no se muestra el bloque `📁 Archivos modificados:`
- [x] CA4: Si `git.enabled = false`, se omite el diff (sin error)
- [x] CA5: Cada línea de invocación de agente incluye el modelo: `🎯 <rol> | <story_id> | <provider> [<modelo>]`
- [x] CA6: El modelo se obtiene llamando a `AgentsConfig::model_for_role()` con el skill path correcto
- [x] CA7: Tras `invoke_with_retry()`, se llama a `parse_token_count()` con `result.stdout + result.stderr`
- [x] CA8: Los tokens parseados se acumulan en `shared_state.token_usage` bajo el `story_id` (push al Vec)
- [x] CA9: Al finalizar el pipeline, se emite un bloque de cierre con:
  ```
  ══════════════════════════════════════════════════════════════
  🏁 Pipeline completado — <timestamp>
     Total        : N
     ✅ Done      : N
     ❌ Failed    : N (<lista>)
     🔒 Blocked   : N
     📝 Draft     : N
     🔄 Iteraciones: N
     ⏱️  Tiempo total: Xh Ym Zs
     📊 Tokens totales: N input + N output = N
  ══════════════════════════════════════════════════════════════
  ```
- [x] CA10: El conteo `📊 Tokens totales` suma todos los `TokenCount` de todas las historias
- [x] CA11: `cargo build` compila sin errores
- [x] CA12: `cargo test` pasa todos los tests existentes
- [x] CA13: El dry-run (`--dry-run`) no intenta hacer git diff ni parsear tokens reales

## Dependencias
- Bloqueado por: STORY-019, STORY-020, STORY-021, STORY-022, STORY-026

## Activity Log
- 2026-05-06 | Dev | Implementación completa de STORY-027 en app/pipeline.rs. Se integraron 3 funcionalidades: (1) format_agent_line_with_model() a nivel de módulo para loguear cada invocación con el modelo resuelto via AgentsConfig::model_for_role(); (2) parseo y acumulación de tokens post-agente en SharedState.token_usage usando agent::parse_token_count() sobre stdout+stderr combinados; (3) git diff --stat post-agente en modo detallado (!compact, git enabled) logueando archivos modificados con 📁. Se implementó el bloque de cierre enriquecido en run_real() con resumen de tokens totales (input+output), conteo por estado, IDs de fallidas, y timestamp via chrono. Se corrigieron 6 llamadas a process_story() en tests que no pasaban el nuevo parámetro compact. Se aplicaron correcciones de clippy (or_default, and_then). 520 tests pasan, clippy limpio, fmt OK.
- 2026-05-06 | Reviewer | ✅ Revisión técnica aprobada. DoD verificado: cargo build compila sin errores, 520 tests pasan (0 fallos, 1 ignorado), cargo clippy -- -D warnings limpio, cargo fmt -- --check limpio. Dependencias STORY-019, 020, 021, 022, 026 en Done. Los 13 CAs tienen cobertura de tests específicos en el módulo story027 del pipeline. Transición a Business Review.
- 2026-05-06 | PO | ✅ Validación de negocio aprobada. Los 13 CAs verificados contra el código en app/pipeline.rs: (1) format_agent_line_with_model() formatea 🎯 rol | id | provider [modelo] usando model_for_role(); (2) parse_token_count() sobre stdout+stderr acumula en shared_state.token_usage; (3) git diff --stat en modo detallado (!compact, git enabled) muestra 📁 Archivos modificados. Bloque de cierre enriquecido con tokens totales, conteo por estado, timestamp. 520 tests pasan, build/clippy/fmt limpios. Dependencias todas en Done. Valor de negocio cumplido. Transición a Done.