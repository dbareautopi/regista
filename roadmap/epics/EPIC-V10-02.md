# EPIC-V10-02: Dominio Genérico

## Objetivo
Reemplazar el dominio hardcodeado de desarrollo de software (`Story` con formato STORY-NNN, `Status`/`Actor`/`Transition` fijos, `CanonicalWorkflow` de 14 transiciones) por un dominio configurable donde el usuario define el formato de task, los estados, los roles y las fases en `.regista/config.toml`.

## Alcance
- `domain/task.rs` — `Task` genérico con `fields: HashMap<String, String>` y parseo configurable vía `section_markers`
- `domain/workflow.rs` — `ConfigurableWorkflow` cargado desde TOML con estados, roles, fases y políticas de rechazo
- `domain/prompts.rs` — Sistema de templates con `{{variables}}` (`{{task_id}}`, `{{task_fields.*}}`, `{{last_rejection}}`)
- Adaptar `domain/deadlock.rs` para usar `Task` genérico y `WorkflowConfig`

## Historias
- STORY-V10-006: Task genérico con parseo configurable
- STORY-V10-007: ConfigurableWorkflow desde TOML
- STORY-V10-008: Sistema de templates para prompts
- STORY-V10-009: Adaptar deadlock y graph a Task genérico

## Dependencias
- EPIC-V10-01 (usa `infra/llm/types.rs` para `TokenUsage`)

## Rama
`rework`

## Estimación
2 semanas
