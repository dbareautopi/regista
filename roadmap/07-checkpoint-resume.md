# 07 — Checkpoint / Resume

## 🎯 Objetivo

Guardar el estado del orquestador tras cada iteración para poder reanudar
la ejecución si se interrumpe (crash, timeout, Ctrl+C, reinicio de máquina).

## ❓ Problema actual

Si el pipeline se interrumpe por cualquier motivo (se alcanza `max_iterations`,
`max_wall_time`, fallo de red, OOM, kill signal), la siguiente ejecución
empieza **desde cero**. No hay memoria de qué historias ya se procesaron ni
en qué iteración iba.

En un backlog de 21 historias, reiniciar desde cero desperdicia créditos de LLM
y tiempo.

## ✅ Solución propuesta

### Checkpoint automático

Tras cada `process_story()` exitoso, guardar estado en
`<project_dir>/.regista.state.toml`:

```toml
[orchestrator]
iteration = 7
elapsed_seconds = 1247
last_story_processed = "STORY-007"
max_iterations = 10
max_wall_time_seconds = 28800

[reject_cycles]
"STORY-013" = 2
"STORY-015" = 1

[story_snapshots]
"STORY-001" = "Done"
"STORY-002" = "Done"
"STORY-007" = "Ready"
```

### Reanudación

```bash
regista --resume                 # continúa desde último checkpoint
regista --resume --no-checkpoint # ignora checkpoint, empieza limpio
```

El flag `--resume`:
1. Carga `.regista.state.toml`.
2. Restaura `reject_cycles` y contador de iteración.
3. Continúa el loop desde donde estaba.
4. Si el checkpoint está corrupto o es de otra versión, advierte y pregunta.

### Limpieza

```bash
regista --clean-state            # borra .regista.state.toml
```

El checkpoint se limpia automáticamente cuando el pipeline llega a
`PipelineComplete`.

## 📝 Notas de implementación

- Nuevo módulo `src/checkpoint.rs` con `save()` y `load()`.
- La struct `OrchestratorState` es `Serialize + Deserialize`.
- El checkpoint se guarda **después** de `process_story()`, no antes (para
  evitar corrupción si falla a mitad).
- Formato TOML (consistente con el resto del proyecto).
- Posible riesgo: si las historias fueron modificadas manualmente entre
  ejecuciones, el checkpoint puede estar desincronizado. Solución: guardar
  hash de contenido de cada historia en el checkpoint y verificar al cargar.

## 🔗 Relacionado con

- [`01-paralelismo.md`](./01-paralelismo.md) — con concurrencia, el checkpoint
  debe ser atómico entre múltiples workers.
- [`03-dry-run.md`](./03-dry-run.md) — el dry-run podría generar un checkpoint
  simulado para análisis.
