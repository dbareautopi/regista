# STORY-V10-022: Tests de ConfigurableWorkflow y Task genérico

## Status
**Draft**

## Epic
EPIC-V10-06

## Descripción
Crear tests unitarios para `domain/workflow.rs` y `domain/task.rs` que validen:

- Deserialización de `WorkflowConfig` desde TOML con distintas configuraciones (2 fases, 5 fases, con/sin bifurcaciones)
- `phases_for_status()` devuelve las fases correctas y detecta bifurcaciones
- `Task::load()` con distintos `task_format` (STORY-NNN, TASK-NNN, formatos personalizados)
- `Task::set_status()` escribe y verifica el nuevo estado
- `render_template()` sustituye correctamente todas las variables

**Valor de negocio**: Garantiza que el dominio genérico funciona para cualquier configuración que el usuario defina en TOML.

## Criterios de aceptación
- [ ] CA1: Tests de `ConfigurableWorkflow`: deserializar TOML con 3 fases encadenadas (draft→ready→review→done), verificar `phases_for_status("ready")` devuelve solo la fase `implement`, verificar `is_terminal("done")` es true y `is_terminal("draft")` es false
- [ ] CA2: Tests de `Task::load()`: parsear un archivo `.md` con `task_format` que define campos `topic` y `depth`, verificar que `task.fields["topic"]` y `task.fields["depth"]` se extraen correctamente. Parsear con `id_pattern = "TASK-\\d+"` y verificar que un archivo `# STORY-001` es rechazado
- [ ] CA3: Tests de `render_template()`: verificar `{{task_id}}` → `TASK-005`, `{{task_fields.priority}}` → `high`, `{{task_fields.inexistente}}` → `"(no definido)"`, `{{task_fields.*}}` → bullet list con todos los campos, `{{blockers}}` → lista de IDs, `{{last_rejection}}` → último rechazo del activity log

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-007, STORY-V10-008

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 6
