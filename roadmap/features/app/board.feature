# language: es
@app @board @STORY-V10-016

Feature: Dashboard Kanban con columnas dinámicas
  Como usuario que monitorea el progreso del pipeline,
  quiero ver un dashboard cuyas columnas reflejen exactamente los estados de mi workflow,
  para no ver estados irrelevantes ni columnas vacías.

  Background:
    Given un workflow con estados: draft, ready, review, done, failed, blocked
    And 5 tareas en distintos estados:
      | id       | status  |
      | TASK-001 | draft   |
      | TASK-002 | draft   |
      | TASK-003 | ready   |
      | TASK-004 | review  |
      | TASK-005 | done    |

  Scenario: Columnas siguen el orden topológico del workflow
    When se genera BoardData::from_tasks() con el workflow
    Then las columnas aparecen en orden: draft, ready, review, done
    And "blocked" y "failed" aparecen al final (estados terminales)
    And el conteo de draft es 2, ready es 1, review es 1, done es 1

  Scenario: Estados sin tareas se omiten
    Given el workflow define el estado "validating" pero ninguna tarea está en ese estado
    When se genera el board
    Then la columna "validating" no aparece

  Scenario: --json emite estructura compatible con CI/CD
    When se ejecuta regista board --json
    Then stdout contiene JSON válido con campos "columns", "tasks", "summary"
    And "columns" es un array ordenado según el workflow
    And "summary.total" es 5
