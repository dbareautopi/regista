# STORY-027: Diff post-agente + acumulación de tokens + resumen final enriquecido

## Status
**Tests Ready**

## Epic
EPIC-10

## Descripción
Integrar tres funcionalidades en `app/pipeline.rs`:

1. **Diff de archivos post-agente**: tras cada `process_story()` exitosa, si `git.enabled = true` y el modo es detallado (`!compact`), ejecutar `git diff --stat` contra el commit del snapshot previo y loguear los archivos modificados con `📁 Archivos modificados`.

2. **Log de agente con modelo**: cada línea de agente muestra el modelo resuelto: `🎯 Dev (implement) | STORY-003 | pi [qwen2.5-coder]`.

3. **Acumulación de tokens y resumen final**: tras cada invocación de agente, parsear tokens con `parse_token_count()` y acumular en `SharedState.token_usage`. Al terminar el pipeline, emitir un resumen final enriquecido con conteo total de tokens y desglose por historia.

## Criterios de aceptación
- [ ] CA1: Tras cada `process_story()` exitosa en modo detallado, se ejecuta `git diff --stat <snapshot_hash> HEAD` (o `HEAD~1 HEAD` si no hay hash)
- [ ] CA2: La salida de `git diff --stat` se loguea con `tracing::info!` bajo el encabezado `📁 Archivos modificados:`
- [ ] CA3: En modo compacto (`--compact`), no se muestra el bloque `📁 Archivos modificados:`
- [ ] CA4: Si `git.enabled = false`, se omite el diff (sin error)
- [ ] CA5: Cada línea de invocación de agente incluye el modelo: `🎯 <rol> | <story_id> | <provider> [<modelo>]`
- [ ] CA6: El modelo se obtiene llamando a `AgentsConfig::model_for_role()` con el skill path correcto
- [ ] CA7: Tras `invoke_with_retry()`, se llama a `parse_token_count()` con `result.stdout + result.stderr`
- [ ] CA8: Los tokens parseados se acumulan en `shared_state.token_usage` bajo el `story_id` (push al Vec)
- [ ] CA9: Al finalizar el pipeline, se emite un bloque de cierre con:
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
- [ ] CA10: El conteo `📊 Tokens totales` suma todos los `TokenCount` de todas las historias
- [ ] CA11: `cargo build` compila sin errores
- [ ] CA12: `cargo test` pasa todos los tests existentes
- [ ] CA13: El dry-run (`--dry-run`) no intenta hacer git diff ni parsear tokens reales

## Dependencias
- Bloqueado por: STORY-019, STORY-020, STORY-021, STORY-022, STORY-026

## Activity Log
- 2026-05-06 | QA | 32 tests unitarios escritos en app/pipeline.rs cubriendo CA1-CA10 y CA13. Tests verifican: diff post-agente (should_run_post_diff), formato de línea con modelo (format_agent_line_with_model), acumulación de tokens en SharedState.token_usage (CA8), parseo combinado stdout+stderr (CA7), bloque de cierre enriquecido con conteo de tokens (CA9-CA10), y omisión en dry-run (CA13). Usa placeholders mínimos como esqueleto para que el Developer implemente la lógica real. Todos los 95 tests de pipeline.rs pasan (32 nuevos + 63 existentes).