# STORY-V10-014: Presets research y single-agent

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Crear los presets `research` y `single-agent` como constantes embebidas en `app/presets/`. Estos presets demuestran la flexibilidad del nuevo dominio genérico con casos de uso completamente distintos al desarrollo de software:

- **research**: pipeline de 2 fases para investigación (research → report) con formato TASK-NNN y campos topic/depth/sources
- **single-agent**: pipeline mínimo de 1 fase (execute) donde un solo agente decide si la tarea está completada

**Valor de negocio**: Demuestra que regista ya no está atado al desarrollo de software. Atrae nuevos casos de uso y valida la arquitectura genérica.

## Criterios de aceptación
- [ ] CA1: Preset `research`: 2 fases — `research` (pending→draft, rol=researcher) y `report` (draft→done, rol=analyst). `task_format` con `id_pattern = "TASK-\\d+"` y campos `topic`, `depth`, `sources`
- [ ] CA2: Preset `single-agent`: 1 fase — `execute` (pending→done, rol=agent). `task_format` mínimo con `id_pattern = "TASK-\\d+"` y campos `description`, `priority`
- [ ] CA3: Ambos presets son seleccionables con `regista init --preset research` y `regista init --preset single-agent`, generando la configuración completa en `.regista/config.toml`

## Dependencias
- Bloqueado por: STORY-V10-007, STORY-V10-008

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
