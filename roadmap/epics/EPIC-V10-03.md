# EPIC-V10-03: Pipeline Genérico

## Objetivo
Reescribir `app/pipeline.rs` para que el loop de orquestación sea completamente genérico: busca las fases aplicables del workflow, construye los mensajes con el system prompt del rol y el template renderizado, invoca al LLM nativo, parsea la respuesta estructurada del agente, y aplica la acción resultante (transición de estado, rechazo, bloqueo, dependencia).

## Alcance
- `process_task()` — lookup dinámico de fases → build_messages → llm.chat → parse_action → apply
- Parseo de respuesta del agente con regex: `[STATUS: X]`, `[REJECT: Y]`, `[DEPENDS_ON: Z]`, `[BLOCKED: W]`
- Transiciones automáticas sin agente: task → blocked si dependencias no resueltas, task → unblocked si dependencias done/failed, task → failed si `reject_cycles > max`

## Historias
- STORY-V10-010: Loop principal con lookup dinámico de fases
- STORY-V10-011: Parseo de respuesta del agente
- STORY-V10-012: Transiciones automáticas y manejo de rechazos

## Dependencias
- EPIC-V10-01 (cliente LLM)
- EPIC-V10-02 (Task y Workflow genéricos)

## Rama
`rework`

## Estimación
2 semanas
