# STORY-V10-018: Adaptar plan.rs a agente LLM nativo

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Adaptar `app/plan.rs` para que `regista plan spec.md` use el LLM nativo (a través del `LlmProvider`) en lugar de providers CLI externos. El flujo sigue siendo el mismo: el agente recibe una especificación y genera tasks en el formato definido por el preset.

El bucle plan→validate (generar tasks, validar dependencias, dar feedback al agente, corregir) debe usar el `task_format` configurable para parsear y validar las dependencias entre tasks, en lugar de asumir el formato STORY-NNN.

**Valor de negocio**: La generación de backlog funciona con cualquier preset. Un usuario de `research` puede generar TASKs desde una spec igual que uno de `software-dev`.

## Criterios de aceptación
- [ ] CA1: `regista plan spec.md` invoca al LLM usando el modelo configurado para el rol `product_owner` (o el primer rol definido en el workflow), con un prompt que incluye el `task_format` para que el agente sepa qué campos debe rellenar
- [ ] CA2: El bucle de validación (máx `plan_max_iterations`) parsea las tasks generadas, verifica que las dependencias forman un DAG sin ciclos, y si hay problemas, reinyecta feedback concreto al agente ("STORY-003 referencia a STORY-999 que no existe")
- [ ] CA3: `--max-stories <N>` y `--replace` funcionan con el nuevo dominio: `--replace` borra todas las tasks existentes antes de generar; `--max-stories` limita el número de tasks generadas (0 = sin límite)

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-010

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
