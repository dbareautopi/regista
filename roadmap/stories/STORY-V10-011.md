# STORY-V10-011: Parseo de respuesta del agente

## Status
**Draft**

## Epic
EPIC-V10-03

## Descripción
Implementar el parseo de la respuesta del agente LLM para extraer acciones estructuradas. El prompt de cada fase incluye instrucciones de formato estricto para que el agente responda con uno de estos marcadores:

- `[STATUS: <estado>]` — transición exitosa al estado destino
- `[REJECT: <motivo>]` — rechazo, vuelve al estado `on_reject`
- `[DEPENDS_ON: <id>]` — la task depende de otra task
- `[BLOCKED: <motivo>]` — la task se bloquea manualmente

El parser debe validar que el estado destino existe en el workflow. Si el agente no sigue el formato, se reintenta inyectando feedback en el prompt.

**Valor de negocio**: Interfaz robusta entre el LLM y el orquestador. El formato estructurado permite automatizar el pipeline sin intervención humana.

## Criterios de aceptación
- [ ] CA1: `parse_agent_action(response: &str, workflow: &ConfigurableWorkflow) -> Result<AgentAction>` extrae con regex `[STATUS: X]`, `[REJECT: Y]`, `[DEPENDS_ON: Z]`, `[BLOCKED: W]` de la respuesta del agente, devolviendo un enum `AgentAction` con las variantes `Transition`, `Reject`, `AddDependency`, `Block`
- [ ] CA2: Si el agente responde con `[STATUS: X]` pero X no es un estado válido del workflow (no está en `states.terminal` ni es destino de ninguna fase), se devuelve error con mensaje "Estado 'X' no definido en el workflow"
- [ ] CA3: Si la respuesta no contiene ningún marcador reconocido, se devuelve `AgentParseError::NoMarkerFound` con la respuesta completa. El pipeline usa este error para reintentar con feedback ("Tu respuesta no incluye [STATUS: ...]. Por favor, indica el nuevo estado.")

## Dependencias
- Bloqueado por: STORY-V10-007, STORY-V10-010

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 3
