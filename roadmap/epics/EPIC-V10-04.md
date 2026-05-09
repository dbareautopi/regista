# EPIC-V10-04: Presets y CLI

## Objetivo
Crear presets de fábrica embebidos como constantes (`software-dev`, `research`, `single-agent`) que ofrezcan experiencia out-of-the-box sin configuración. Adaptar todos los subcomandos de CLI y casos de uso (`init`, `plan`, `board`, `validate`) al nuevo dominio genérico: workflows configurables, tareas con formato variable, y modelos LLM nativos.

## Alcance
- `app/presets/` — 3 presets de fábrica con workflow, roles, system prompts y task_format
- `app/init.rs` — scaffolding con `--preset <name>` que genera `.regista/config.toml` completo
- `app/board.rs` — columnas dinámicas derivadas de `workflow.states`
- `app/validate.rs` — validación de modelos LLM, regex de task_format, coherencia de fases
- `app/plan.rs` — spec → tasks usando LLM nativo + bucle plan→validate con task_format configurable
- `cli/` — adaptar args y handlers a los nuevos conceptos

## Historias
- STORY-V10-013: Preset software-dev
- STORY-V10-014: Presets research y single-agent
- STORY-V10-015: Adaptar init.rs para scaffolding con presets
- STORY-V10-016: Adaptar board.rs a columnas dinámicas
- STORY-V10-017: Adaptar validate.rs a dominio genérico
- STORY-V10-018: Adaptar plan.rs a LLM nativo

## Dependencias
- EPIC-V10-01 (cliente LLM)
- EPIC-V10-02 (Task y Workflow genéricos)
- EPIC-V10-03 (Pipeline genérico)

## Rama
`rework`

## Estimación
2 semanas
