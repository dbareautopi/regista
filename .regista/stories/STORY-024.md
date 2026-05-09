# STORY-024: Flag `--compact` en `CommonArgs` y propagación al pipeline

## Status
**Done**

## Epic
EPIC-09

## Descripción
Añadir el flag `--compact` (bool, default false) a la struct `CommonArgs` en `cli/args.rs`. Este flag se propaga a través de `PipelineOptions` hasta el pipeline, donde controla el valor del parámetro `verbose` que se pasa a `invoke_with_retry()`. En modo compacto (`--compact`), `verbose = false` (sin streaming, sin diffs). Sin `--compact` (default), `verbose = true`.

## Criterios de aceptación
- [x] CA1: `CommonArgs` tiene campo `pub compact: bool` con `#[arg(long = "compact")]` y default `false`
- [x] CA2: `PipelineOptions` (o la struct equivalente en `app/pipeline.rs`) tiene campo `pub compact: bool`
- [x] CA3: `handle_plan()`, `handle_auto()`, y `handle_run()` propagan `args.common.compact` a las opciones del pipeline
- [x] CA4: El pipeline usa `!options.compact` como valor de `verbose` al llamar a `invoke_with_retry()`
- [x] CA5: `cargo build` compila sin errores
- [x] CA6: `cargo test` pasa todos los tests existentes
- [x] CA7: Al ejecutar `regista run --compact`, no se muestra streaming (no hay líneas con `│`) ni diffs (no hay `📁`)

## Dependencias
- Bloqueado por: STORY-022

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 1: Dos niveles de verbosidad).
- 2026-05-08 | Dev | Implementado: build_run_options recibe compact, build_daemon_args propaga --compact, !compact → verbose en invoke_with_retry. 523 tests pasan.