# language: es
@domain @deadlock @STORY-V10-009

Feature: Detección de deadlock y grafo de dependencias genérico
  Como orquestador,
  quiero detectar bloqueos circulares y dependencias no resueltas en tareas genéricas,
  para que el pipeline no se quede bloqueado indefinidamente con cualquier formato de ID.

  Background:
    Given 4 tareas con formato TASK-NNN:
      | id       | status   | blockers            |
      | TASK-001 | done     | []                  |
      | TASK-002 | draft    | ["TASK-001"]        |
      | TASK-003 | draft    | ["TASK-002"]        |
      | TASK-004 | draft    | ["TASK-003"]        |

  Scenario: Construir grafo de dependencias con IDs genéricos
    When se construye DependencyGraph::from_tasks() con las 4 tareas
    Then el grafo contiene 4 nodos: TASK-001, TASK-002, TASK-003, TASK-004
    And la arista TASK-001→TASK-002 existe (forward dependency)
    And no se asume ningún formato de ID (no hardcodea STORY-NNN)

  Scenario: Detectar deadlock y priorizar por desbloqueo
    Given TASK-002 y TASK-003 están en estado "draft"
    And TASK-002 bloquea a TASK-003 y TASK-004
    And TASK-003 bloquea a TASK-004
    When se invoca analyze_deadlock()
    Then la resolución sugiere procesar TASK-002 primero
    And el mensaje de resolución menciona "TASK-002" (no hardcodea STORY)

  Scenario: Transición automática a blocked por dependencias no resueltas
    Given TASK-004.status es "ready" y TASK-004.blockers contiene ["TASK-003"]
    And TASK-003.status no es terminal (es "draft")
    When se aplican transiciones automáticas
    Then TASK-004.status pasa a "blocked"
    And el motivo registrado es "Dependencias no resueltas: TASK-003"
