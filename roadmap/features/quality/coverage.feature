# language: es
@quality @coverage @STORY-V10-021 @STORY-V10-022 @STORY-V10-023 @STORY-V10-024

Feature: Cobertura de tests para el rework v1.0
  Como equipo de desarrollo,
  queremos tests unitarios y de integración para todos los módulos nuevos y adaptados,
  para garantizar que el rework no introduce regresiones y cumple las reglas de arquitectura.

  # ── STORY-V10-021: Tests del cliente LLM ───────────────

  Scenario: OpenAiProvider con mock server — respuesta exitosa
    Given un mock HTTP server que responde 200 con {"choices":[{"message":{"content":"ok"}}]}
    When OpenAiProvider.chat() envía una request
    Then el ChatResponse.content es "ok"
    And el ChatResponse.finish_reason es "stop"

  Scenario: OpenAiProvider con mock server — error HTTP
    Given un mock HTTP server que responde 401 con {"error":{"message":"Invalid API key"}}
    When OpenAiProvider.chat() envía una request
    Then devuelve Err con mensaje que contiene "401"

  Scenario: AnthropicProvider con mock server — adaptación de mensajes
    Given un mock HTTP server que captura el body de la request
    And mensajes de entrada con rol "system"
    When AnthropicProvider.chat() envía la request
    Then el body contiene "system" como campo top-level (no dentro de messages)

  # ── STORY-V10-022: Tests de dominio ────────────────────

  Scenario: ConfigurableWorkflow con TOML de 5 fases
    Given un TOML con 5 fases encadenadas (A→B→C→D→E)
    When se deserializa a WorkflowConfig y se construye ConfigurableWorkflow
    Then phases_for_status("C") devuelve exactamente 1 fase (C→D)

  Scenario: Task::load con formato personalizado
    Given un archivo TASK-042.md con campos "topic" y "depth"
    And task_format define section_markers para esos campos
    When se invoca Task::load()
    Then task.fields["topic"] no está vacío
    And task.fields["depth"] no está vacío

  Scenario: render_template con variable desconocida
    Given template con {{task_fields.inexistente}}
    When se renderiza
    Then el resultado es "(no definido)" sin paniquear

  # ── STORY-V10-023: Tests de pipeline ───────────────────

  Scenario: Pipeline happy path con mock LLM provider
    Given 2 tareas independientes en estado "draft"
    And un mock LlmProvider que responde [STATUS: done] siempre
    When se ejecuta run()
    Then ambas tareas terminan en "done"
    And el reporte muestra 2 done, 0 failed

  Scenario: Pipeline maneja rechazo con reintento exitoso
    Given el mock responde [REJECT: error] en el primer intento
    And responde [STATUS: ready] en el segundo
    When se procesa la tarea
    Then la tarea avanza a "ready" en el segundo intento
    And reject_cycles se incrementó a 1

  Scenario: Pipeline con dependencias respeta orden
    Given TASK-002 depende de TASK-001
    And TASK-001 está en "draft", TASK-002 en "ready"
    When se ejecuta el pipeline
    Then TASK-002 se bloquea hasta que TASK-001 llega a "done"
    And el orden de procesamiento es TASK-001 primero, TASK-002 después

  # ── STORY-V10-024: Tests de presets + arquitectura ─────

  Scenario: Cada preset es válido y completo
    Given el registry de presets con software-dev, research, single-agent
    When se itera sobre cada preset
    Then cada uno tiene states.initial no vacío
    And states.terminal no está vacío
    And phases tiene al menos 1 entrada
    And cada PhaseConfig tiene from/to que existen en states

  Scenario: Migración de Story v0.x a Task con preset software-dev
    Given un archivo STORY-001.md con formato v0.x (CA, épica, dependencias)
    And el task_format del preset software-dev
    When se parsea con Task::load()
    Then task.fields["epic"] está presente
    And task.blockers se extraen correctamente
    And task.activity_log tiene entradas parseadas

  Scenario: Tests de arquitectura verifican reglas R1-R6
    Given el archivo tests/architecture.rs actualizado
    When se ejecuta cargo test --test architecture
    Then verifica que domain/ no importa infra/, app/, ni cli/
    And verifica que infra/llm/ no importa domain/, app/, ni cli/
    And verifica que config/ no importa ninguna capa del crate
    And todos los tests de arquitectura pasan (R1-R6)
