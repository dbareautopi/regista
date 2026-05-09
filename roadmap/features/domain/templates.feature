# language: es
@domain @templates @STORY-V10-008

Feature: Sistema de templates para prompts
  Como definidor de fases de workflow,
  quiero usar {{variables}} en los prompts que se sustituyen en runtime,
  para que los prompts referencien dinámicamente campos de la task sin hardcodearlos.

  Background:
    Given un Task con:
      | id          | TASK-005                |
      | status      | pending                 |
      | fields      | {"priority": "high", "topic": "IA generativa"} |
      | blockers    | ["TASK-002", "TASK-003"] |
    And una entrada en activity_log: "2026-05-08 | reviewer | rechazado: tests no compilan"
    And un context con {"fecha_limite": "2026-06-01"}

  Scenario: Sustituir variables básicas de task
    Given el template "Procesa {{task_id}} con estado {{task_status}}"
    When se invoca render_template()
    Then el resultado es "Procesa TASK-005 con estado pending"

  Scenario: Sustituir campos dinámicos con {{task_fields.*}}
    Given el template "Campos: {{task_fields.*}}"
    When se invoca render_template()
    Then el resultado contiene "- priority: high"
    And el resultado contiene "- topic: IA generativa"

  Scenario: Variables no definidas no rompen el renderizado
    Given el template "{{task_fields.inexistente}}"
    When se invoca render_template()
    Then el resultado es "(no definido)"
    And se emite un warning en el log
