# STORY-V10-013: Preset software-dev

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Crear el preset `software-dev` como constante embebida en `app/presets/software_dev.rs`. Este preset replica el pipeline de desarrollo de software de v0.x pero simplificado a 3 fases (plan → implement → validate), ya que los modelos modernos no necesitan la separación QA/Dev/Reviewer.

El formato de task debe ser compatible con el formato STORY-NNN de v0.x (CA, épicas, dependencias, Activity Log) para que los proyectos existentes migren sin cambios en sus archivos de historia. El preset incluye system prompts para los 3 roles y la configuración completa del workflow.

**Valor de negocio**: Experiencia out-of-the-box para el caso de uso principal (desarrollo de software). Los usuarios de v0.x migran ejecutando `regista init --preset software-dev`.

## Criterios de aceptación
- [ ] CA1: El preset define 3 fases: `plan` (draft→ready, rol=product_owner), `implement` (ready→review, rol=developer), `validate` (review→done, rol=reviewer), con `max_reject_cycles=8` y `on_reject` retornando al estado anterior
- [ ] CA2: `task_format` compatible con v0.x: `id_pattern = "STORY-\\d+"`, `section_markers` con Status/Epic/Descripción/CA/Dependencias/Activity Log, `dependency_marker = "Bloqueado por:"`
- [ ] CA3: Los system prompts de los 3 roles incluyen instrucciones de formato estricto (`[STATUS: X]`, `[REJECT: Y]`) y referencian el formato de task (CA, épicas, dependencias)

## Dependencias
- Bloqueado por: STORY-V10-007, STORY-V10-008

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
