# STORY-V10-012: Transiciones automáticas y manejo de rechazos

## Status
**Draft**

## Epic
EPIC-V10-03

## Descripción
Implementar las transiciones automáticas que el orquestador aplica sin intervención de un agente LLM, equivalente a las transiciones 12-14 del `CanonicalWorkflow` de v0.x pero generalizadas para cualquier workflow:

1. **Blocked**: si una task tiene dependencias (`blockers`) y al menos una no está en estado terminal, pasa automáticamente a `blocked`
2. **Unblocked**: si todas las dependencias están en estado terminal, la task vuelve a su estado anterior (o al inicial)
3. **Failed**: si `reject_cycles` supera `max_reject_cycles` de la fase actual

**Valor de negocio**: El orquestador gestiona dependencias y rechazos sin gastar créditos de LLM. Son reglas deterministas que no requieren inteligencia artificial.

## Criterios de aceptación
- [ ] CA1: `apply_automatic_transitions(tasks, graph, workflow)` itera sobre todas las tasks no terminales: si tiene dependencias no resueltas → `blocked`; si estaba `blocked` y todas sus dependencias están en estado terminal → vuelve al estado inicial del workflow; si `reject_cycles > phase.max_reject_cycles` → `failed`
- [ ] CA2: El estado `blocked` y `failed` deben estar definidos en `workflow.states.terminal` (o ser implícitamente terminales). Si el workflow no define estos estados, se usan `"blocked"` y `"failed"` como strings y se tratan como terminales
- [ ] CA3: Cuando una task pasa a `failed`, se registra en el `Activity Log` con el motivo (ej. "8 ciclos de rechazo superados en fase 'review'"). Las tasks que dependían de ella se reevalúan en la siguiente iteración (pueden desbloquearse si `failed` es terminal)

## Dependencias
- Bloqueado por: STORY-V10-009

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 3
