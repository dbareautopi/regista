# language: es
@domain @deadlock @graph @STORY-V10-009

Feature: Detección de deadlock y grafo de dependencias genérico
  Como orquestador,
  quiero detectar bloqueos circulares y dependencias no resueltas en tareas genéricas,
  para que el pipeline no se quede bloqueado indefinidamente con cualquier formato de ID.

  Background:
    Given 5 tareas con formato TASK-NNN:
      | id       | status      | blockers            |
      | TASK-001 | done        | []                  |
      | TASK-002 | draft       | ["TASK-001"]        |
      | TASK-003 | draft       | ["TASK-002"]        |
      | TASK-004 | ready       | ["TASK-003"]        |
      | TASK-005 | blocked     | ["TASK-002"]        |

  # ── Capa: Dominio (domain/graph.rs) — DependencyGraph ────────────────

  # CA1: Construir grafo con IDs genéricos
  Scenario: Construir grafo con IDs TASK-NNN
    When se construye DependencyGraph::from_tasks() con las 5 tareas
    Then el grafo contiene 5 nodos: TASK-001, TASK-002, TASK-003, TASK-004, TASK-005
    And la arista forward TASK-001→TASK-002 existe
    And la arista forward TASK-002→TASK-003 existe
    And la arista forward TASK-002→TASK-005 existe
    And la arista forward TASK-003→TASK-004 existe

  Scenario: Construir grafo con IDs ISSUE-NNN
    Given tareas con IDs ISSUE-010, ISSUE-011, ISSUE-012
    And ISSUE-012.blockers = ["ISSUE-010", "ISSUE-011"]
    When se construye DependencyGraph::from_tasks()
    Then los nodos son "ISSUE-010", "ISSUE-011", "ISSUE-012"
    And blocks_count("ISSUE-010") es 1
    And blocks_count("ISSUE-011") es 1

  Scenario: Construir grafo sin dependencias
    Given 3 tareas sin blockers
    When se construye DependencyGraph::from_tasks()
    Then el grafo tiene 3 nodos pero 0 aristas
    And blocks_count para cualquier nodo es 0

  # CA1: Detección de ciclos con IDs genéricos
  Scenario: Detectar ciclo entre dos nodos con IDs ISSUE-NNN
    Given ISSUE-001.blockers = ["ISSUE-002"]
    And ISSUE-002.blockers = ["ISSUE-001"]
    When se construye el grafo y se invoca has_cycle_from("ISSUE-001")
    Then devuelve true
    And has_cycle_from("ISSUE-002") también devuelve true

  Scenario: Detectar ciclo entre tres nodos
    Given TASK-A.blockers = ["TASK-C"]
    And TASK-B.blockers = ["TASK-A"]
    And TASK-C.blockers = ["TASK-B"]
    When se invoca has_any_cycle()
    Then devuelve true
    And find_cycle_members() contiene TASK-A, TASK-B, TASK-C

  Scenario: Sin ciclo en cadena lineal
    Given TASK-001 depende de TASK-002, que depende de TASK-003
    And TASK-003 no tiene dependencias
    When se invoca has_any_cycle()
    Then devuelve false

  Scenario: Nodo aislado sin dependencias no tiene ciclo
    Given TASK-001.blockers = []
    When se invoca has_cycle_from("TASK-001")
    Then devuelve false

  # ── Capa: Dominio (domain/deadlock.rs) — analyze_deadlock() ──────────

  # CA2: Detección de deadlock con Task genérico
  Scenario: Deadlock por tareas en Draft — priorizar por desbloqueo
    Given TASK-002.status = "draft" y TASK-003.status = "draft"
    And TASK-002 bloquea a TASK-003 y TASK-005 (2 tareas)
    And TASK-003 bloquea a TASK-004 (1 tarea)
    And no hay tareas accionables (ninguna en estado "ready" sin dependencias)
    When se invoca analyze_deadlock(tasks, graph, workflow)
    Then la resolución es InvokeAgentFor con story_id = "TASK-002"
    And el motivo menciona "draft" y "desbloquearía 2 tareas"

  Scenario: Deadlock — bloqueador en Draft causa deadlock en bloqueada
    Given TASK-002.status = "draft"
    And TASK-005.status = "blocked" con blockers = ["TASK-002"]
    And no hay tareas accionables
    When se invoca analyze_deadlock()
    Then la resolución apunta a TASK-002 (el bloqueador Draft)
    And el motivo menciona que TASK-002 "bloquea a TASK-005"

  Scenario: Deadlock por ciclo de dependencias
    Given TASK-006.status = "blocked" y TASK-006.blockers = ["TASK-007"]
    And TASK-007.status = "blocked" y TASK-007.blockers = ["TASK-006"]
    And no hay tareas accionables
    When se invoca analyze_deadlock()
    Then la resolución menciona "ciclo de dependencias"
    And el mensaje de resolución contiene IDs "TASK-006" o "TASK-007"

  Scenario: NoDeadlock cuando hay tareas accionables
    Given TASK-004.status = "ready" y TASK-004.blockers = [] (sin dependencias)
    When se invoca analyze_deadlock()
    Then la resolución es NoDeadlock

  Scenario: NoDeadlock cuando todas las bloqueadas esperan tareas en progreso
    Given TASK-002.status = "in_progress" (no Draft, no bloqueado)
    And TASK-005.status = "blocked" con blockers = ["TASK-002"]
    When se invoca analyze_deadlock()
    Then la resolución es NoDeadlock (TASK-002 está en progreso, no hay deadlock)

  Scenario: PipelineComplete cuando todas las tareas están en estados terminales
    Given todas las tareas tienen status "done" o "failed"
    When se invoca analyze_deadlock()
    Then la resolución es PipelineComplete

  Scenario: PipelineComplete con mix de done y failed
    Given TASK-001.status = "done", TASK-002.status = "failed", TASK-003.status = "done"
    When se invoca analyze_deadlock()
    Then la resolución es PipelineComplete

  Scenario: Los mensajes de resolución no hardcodean "STORY"
    Given una tarea con ID "TASK-042" en estado "draft"
    When analyze_deadlock() genera el mensaje de motivo
    Then el mensaje contiene "TASK-042"
    And el mensaje NO contiene la palabra "STORY" (no hardcodeada)

  Scenario: Priorización por número de desbloqueos, empate por ID numérico
    Given TASK-010 bloquea a 2 tareas y está en Draft
    And TASK-020 bloquea a 2 tareas y está en Draft
    When se invoca analyze_deadlock()
    Then la resolución elige TASK-010 (menor ID numérico en caso de empate)

  # CA3: Transiciones automáticas como métodos de ConfigurableWorkflow
  Scenario: Transición a blocked por dependencias no resueltas
    Given TASK-004.status = "ready"
    And TASK-004.blockers = ["TASK-003"]
    And TASK-003.status = "draft" (no terminal)
    When se aplican transiciones automáticas vía workflow
    Then TASK-004.status pasa a "blocked"
    And el motivo registrado es "Dependencias no resueltas: TASK-003"

  Scenario: Transición a blocked no afecta tareas sin dependencias
    Given TASK-001.status = "ready" y blockers = []
    When se aplican transiciones automáticas
    Then TASK-001.status sigue siendo "ready"

  Scenario: Transición blocked → unblocked cuando dependencias pasan a terminal
    Given TASK-004.status = "blocked"
    And TASK-004.blockers = ["TASK-003"]
    And TASK-003.status = "done" (terminal)
    When se aplican transiciones automáticas
    Then TASK-004.status pasa al estado inicial del workflow (desbloqueo)

  Scenario: Transición blocked → unblocked con múltiples dependencias
    Given TASK-005.status = "blocked" con blockers = ["TASK-001", "TASK-002"]
    And TASK-001.status = "done" y TASK-002.status = "done"
    When se aplican transiciones automáticas
    Then TASK-005 se desbloquea

  Scenario: Transición blocked se mantiene si alguna dependencia no es terminal
    Given TASK-004.status = "blocked" con blockers = ["TASK-002", "TASK-003"]
    And TASK-002.status = "done" pero TASK-003.status = "draft"
    When se aplican transiciones automáticas
    Then TASK-004.status sigue siendo "blocked"

  Scenario: Transición a failed por max_reject_cycles agotado
    Given una task con reject_cycles = 3
    And el workflow define max_reject_cycles = 3 para su fase actual
    When se aplican transiciones automáticas
    Then la task pasa a "failed"
    And el motivo es "max_reject_cycles agotado (3/3)"

  Scenario: No transición a failed si reject_cycles < max_reject_cycles
    Given una task con reject_cycles = 2
    And el workflow define max_reject_cycles = 3
    When se aplican transiciones automáticas
    Then la task NO pasa a "failed"
    And se permite otro ciclo de rechazo

  Scenario: Transición a failed se define en ConfigurableWorkflow, no en código fijo
    Given el workflow define failed_state = "rejected"
    And max_reject_cycles se agota para una task
    When se aplican transiciones automáticas
    Then la task pasa a "rejected" (nombre configurable, no hardcodeado "Failed")

  # ── Integridad cross-layer ──────────────────────────────────────────

  Scenario: domain/deadlock.rs y domain/graph.rs no importan otras capas
    Given el código fuente de domain/deadlock.rs y domain/graph.rs
    Then no contienen "use crate::app::"
    And no contienen "use crate::infra::"
    And no contienen "use crate::cli::"
    And no contienen "use crate::config::"
    And las funciones reciben &Task y &ConfigurableWorkflow por referencia

  Scenario: analyze_deadlock recibe el workflow como parámetro
    Given un ConfigurableWorkflow con estados terminales custom
    When se invoca analyze_deadlock(tasks, graph, &workflow)
    Then la lógica usa workflow.is_terminal() para decidir PipelineComplete
    And no hardcodea los nombres de estados terminales
