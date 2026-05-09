# STORY-V10-007: ConfigurableWorkflow desde TOML

## Status
**Draft**

## Epic
EPIC-V10-02

## Descripción
Crear `domain/workflow.rs` con `ConfigurableWorkflow` que se carga desde la sección `[workflow]` de `.regista/config.toml`. El workflow define: estados (inicial y terminales), roles (con system prompt y modelo asignado), fases (transiciones de estado con prompt template, política de rechazo, y timeout), y formato de task.

A diferencia de v0.x donde las 14 transiciones eran inmutables, aquí el usuario define un DAG de fases arbitrario. El orquestador debe poder consultar dinámicamente qué fases son aplicables desde un estado dado.

**Valor de negocio**: Flexibilidad total. El usuario define su propio pipeline (2 fases, 10 fases, bifurcaciones) sin tocar código. Los presets de fábrica ofrecen defaults sensatos.

## Criterios de aceptación
- [ ] CA1: `WorkflowConfig` se deserializa desde TOML con campos `states` (inicial + terminales), `roles` (name, system_prompt, model), `phases` (name, from, to, role, model, prompt, on_reject, max_reject_cycles, timeout_seconds), y `task_format` (id_pattern, section_markers, dependency_marker)
- [ ] CA2: `ConfigurableWorkflow::phases_for_status(status: &str) -> Vec<&PhaseConfig>` devuelve todas las fases cuyo `from` coincide con el estado actual. Si hay más de una, hay bifurcación y el agente elige
- [ ] CA3: `ConfigurableWorkflow::is_terminal(status: &str) -> bool` devuelve true si el estado está en `states.terminal`. Los estados terminales detienen el pipeline para esa task

## Dependencias
- Ninguna (módulo de dominio puro, usa serde para deserialización)

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 2
