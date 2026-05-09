# language: es
@app @pipeline @STORY-V10-010 @STORY-V10-011 @STORY-V10-012

Feature: Pipeline de orquestación genérico
  Como orquestador configurable,
  quiero un loop que procese tareas siguiendo el workflow definido en TOML,
  para que cualquier pipeline (2 fases o 10 fases) funcione sin cambios en el código.

  Background:
    Given un workflow con fases: plan(draft→ready), implement(ready→review), validate(review→done)
    And un mock LlmProvider que devuelve respuestas predefinidas según la fase
    And 1 tarea "TASK-001" en estado "draft"

  # ── STORY-V10-010: Loop con lookup dinámico ────────────

  Scenario: El pipeline avanza una tarea por todas las fases
    Given el mock devuelve "[STATUS: ready]" para fase plan
    And el mock devuelve "[STATUS: review]" para fase implement
    And el mock devuelve "[STATUS: done]" para fase validate
    When se ejecuta run() hasta PipelineComplete
    Then TASK-001.status es "done"
    And se realizaron exactamente 3 invocaciones al LLM

  Scenario: Bifurcación presenta opciones al agente
    Given el workflow tiene 2 fases desde "review": approve(review→done) y reject(review→ready)
    And TASK-001.status es "review"
    When se procesa TASK-001
    Then el prompt incluye ambas opciones: "[STATUS: done]" y "[STATUS: ready]"
    And el agente elige una y el orquestador la aplica

  Scenario: El historial multi-turn se acumula en las invocaciones
    Given TASK-001 ha pasado por 2 fases (activity_log con 2 entradas)
    When se procesa la tercera fase
    Then los mensajes enviados al LLM incluyen las 2 entradas anteriores del activity_log
    And cada entrada tiene el rol del actor y la descripción

  # ── STORY-V10-011: Parseo de respuesta ─────────────────

  Scenario: Parsear transición exitosa [STATUS: X]
    Given la respuesta del agente es "He completado la implementación.\n[STATUS: review]"
    When se invoca parse_agent_action()
    Then devuelve AgentAction::Transition("review")
    And no hay error

  Scenario: Parsear rechazo [REJECT: motivo]
    Given la respuesta es "[REJECT: los tests no compilan]"
    When se invoca parse_agent_action()
    Then devuelve AgentAction::Reject("los tests no compilan")

  Scenario: Reintentar cuando el agente no sigue el formato
    Given la respuesta es "Parece que está todo bien, creo que podemos avanzar"
    When se invoca parse_agent_action()
    Then devuelve AgentParseError::NoMarkerFound
    And el pipeline reintenta con feedback: "Tu respuesta no incluye [STATUS: ...]"

  # ── STORY-V10-012: Transiciones automáticas ────────────

  Scenario: Tarea se bloquea por dependencias no resueltas
    Given TASK-002 (ready) depende de TASK-001 (draft, no terminal)
    When se aplican transiciones automáticas
    Then TASK-002.status pasa a "blocked"

  Scenario: Tarea se desbloquea cuando sus dependencias terminan
    Given TASK-002 está "blocked" porque dependía de TASK-001
    And TASK-001.status ahora es "done" (terminal)
    When se aplican transiciones automáticas
    Then TASK-002.status vuelve a "ready" (estado inicial del workflow)

  Scenario: Tarea pasa a failed por superar max_reject_cycles
    Given TASK-003 ha sido rechazada 4 veces en la fase "implement"
    And la fase "implement" tiene max_reject_cycles = 3
    When se aplican transiciones automáticas
    Then TASK-003.status pasa a "failed"
    And el Activity Log registra "4 ciclos de rechazo superados en fase 'implement'"
