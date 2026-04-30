# 08 — Feedback rico de agentes

> ✅ **IMPLEMENTADO** — 2026-04-30

## 🎯 Objetivo

Capturar, estructurar y aprovechar el output (stdout/stderr) de los agentes
`pi`, especialmente cuando fallan, para mejorar la trazabilidad y el contexto
de reintentos.

## ❓ Problema actual

`agent.rs` solo verifica `exit code == 0` para determinar éxito/fallo. Cuando
un agente falla (exit code ≠ 0), el orquestador reintenta con backoff, pero:

1. **No guarda** lo que el agente dijo (stdout/stderr).
2. **No lo inyecta** en el prompt del siguiente intento.
3. **No lo documenta** en `decisions/` como trazabilidad.
4. El developer humano no tiene forma de saber *por qué* falló sin buscar en
   logs.

Los agentes LLM suelen dar errores detallados ("no encontré el archivo X",
"el test Y esperaba Z pero recibí W"). Esa información es oro para debugging.

## ✅ Solución propuesta

### Guardar output de agente

Después de cada invocación (éxito o fallo), guardar en:
```
product/decisions/<STORY-ID>-<actor>-<timestamp>.md
```

Contenido:
```markdown
# STORY-007 — Dev (implement) — 2026-04-30T14:32:00Z

## Resultado
❌ Fallo (exit code: 1, intento 2/5)

## stdout
...output del agente...

## stderr
...errores del agente...

## Prompt usado
...prompt completo...
```

### Inyectar feedback en reintentos

En `agent.rs`, antes de reintentar, modificar el prompt:

```
Tu intento anterior falló. Esto fue lo que ocurrió:

  [stderr del intento anterior]

Corrige el error e inténtalo de nuevo.
```

Esto le da al agente contexto de su propio fallo, aumentando la probabilidad
de éxito en el reintento.

### Estructurar el error

Si el output del agente sigue un formato conocido (ej. JSON), parsearlo para
acción automática:

```json
{
  "error": "missing_file",
  "file": "src/lib.rs",
  "suggestion": "crear el archivo con la estructura base"
}
```

## 📝 Notas de implementación

- `AgentResult` (ya existente) se expande con `attempt: u32` y se guarda.
- `agent::invoke_with_retry` acepta un callback `on_attempt_failed` para
  guardar output.
- El directorio `decisions_dir` debe tener subdirectorios por historia o
  prefijos claros.
- El prompt de reintento con feedback es opcional (configurable:
  `inject_feedback_on_retry = true`).
- Cuidado con prompts que crecen demasiado y exceden ventana de contexto.

## 🔗 Relacionado con

- [`07-checkpoint-resume.md`](./07-checkpoint-resume.md) — el output guardado
  es parte del estado resumible.
- [`10-cross-story-context.md`](./10-cross-story-context.md) — decisiones
  guardadas alimentan contexto de otras historias.
