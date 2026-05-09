# STORY-V10-010: Loop principal del pipeline con lookup dinámico de fases

## Status
**Draft**

## Epic
EPIC-V10-03

## Descripción
Reescribir `app/pipeline.rs` para que el loop de orquestación sea genérico y funcione con cualquier workflow definido en TOML. El nuevo `process_task()` debe:

1. Buscar las fases aplicables para el estado actual de la task (`workflow.phases_for_status`)
2. Si hay bifurcación (múltiples fases desde el mismo estado), presentar las opciones al agente para que elija
3. Construir `Vec<Message>` con el system prompt del rol + el prompt de fase renderizado + el historial de conversación de la task
4. Invocar `llm.chat()` y devolver el resultado

El loop principal (carga de tasks, transiciones automáticas, detección de deadlock, checkpoint) se conserva pero adaptado a `Task` y `ConfigurableWorkflow`.

**Valor de negocio**: El corazón del orquestador se vuelve completamente configurable. Cualquier workflow (2 fases, 10 fases) funciona sin cambios en el código del pipeline.

## Criterios de aceptación
- [ ] CA1: `process_task(task, workflow, llm, shared_state) -> Result<AgentAction>` busca fases con `workflow.phases_for_status(task.status)`, selecciona la fase (si hay bifurcación, incluye las opciones en el prompt), renderiza el template con `render_template()`, construye los mensajes con system prompt + prompt de fase + historial, e invoca `llm.chat()`
- [ ] CA2: El historial multi-turn se construye desde `task.activity_log`: cada entrada es un mensaje con el rol del actor. La conversación completa se pasa al LLM en cada invocación, no solo el prompt actual
- [ ] CA3: El loop principal (`run()`) carga tasks con `Task::load()`, aplica transiciones automáticas, detecta deadlock, procesa una task por iteración, guarda checkpoint, y termina cuando todas las tasks están en estado terminal

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-007, STORY-V10-008, STORY-V10-001

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 3
