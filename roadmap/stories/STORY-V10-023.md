# STORY-V10-023: Tests de pipeline con mock LLM provider

## Status
**Draft**

## Epic
EPIC-V10-06

## Descripción
Crear tests de integración para `app/pipeline.rs` usando un mock `LlmProvider` que devuelve respuestas predefinidas. Los tests deben cubrir el ciclo completo del pipeline:

- Procesar una task desde draft hasta done (happy path multi-fase)
- Manejar un rechazo del agente y reintentar (flujo con `on_reject`)
- Task que supera `max_reject_cycles` y pasa a failed
- Task que se bloquea por dependencias no resueltas y se desbloquea cuando la dependencia termina
- Pipeline que llega a `PipelineComplete` cuando todas las tasks están en estado terminal

**Valor de negocio**: Validación end-to-end del pipeline sin gastar créditos de LLM. Detecta regresiones en la lógica de orquestación.

## Criterios de aceptación
- [ ] CA1: Mock `LlmProvider` que devuelve `[STATUS: ready]`, `[STATUS: review]`, `[STATUS: done]` secuencialmente según la fase. Test: cargar una task en `draft`, ejecutar `process_task()` 3 veces, verificar que llega a `done`
- [ ] CA2: Mock que responde `[REJECT: los tests no compilan]` en la fase `implement`. Test verifica que la task vuelve a `ready` (`on_reject`), el `reject_cycles` se incrementa, y en el siguiente intento (mock devuelve `[STATUS: review]`) avanza correctamente
- [ ] CA3: Test de pipeline completo con 3 tasks inter-dependientes: TASK-001 (sin dependencias), TASK-002 (depende de TASK-001), TASK-003 (depende de TASK-002). Verificar que TASK-002 se bloquea hasta que TASK-001 llega a `done`, y que el pipeline termina con las 3 en `done`

## Dependencias
- Bloqueado por: STORY-V10-010, STORY-V10-011, STORY-V10-012

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 6
