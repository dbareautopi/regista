# language: es
@domain @workflow @STORY-V10-007

Feature: Workflow configurable desde TOML
  Como usuario que define su propio pipeline,
  quiero declarar estados, roles y fases en config.toml,
  para que el orquestador siga mi flujo de trabajo sin modificar código.

  Background:
    Given una configuración TOML con:
      """
      [workflow.states]
      initial = "draft"
      terminal = ["done", "failed"]

      [[workflow.roles]]
      name = "developer"
      system_prompt = "Eres un desarrollador senior."
      model = "gpt4o"

      [[workflow.roles]]
      name = "reviewer"
      system_prompt = "Eres un revisor de código."
      model = "claude"

      [[workflow.phases]]
      name = "implement"
      from = "ready"
      to = "review"
      role = "developer"
      model = "gpt4o"
      prompt = "Implementa la tarea {{task_id}}"
      on_reject = "ready"
      max_reject_cycles = 3

      [[workflow.phases]]
      name = "review"
      from = "review"
      to = "done"
      role = "reviewer"
      model = "claude"
      prompt = "Revisa {{task_id}}"
      on_reject = "ready"
      max_reject_cycles = 2
      """

  # ── Capa: Config (config/workflow.rs) — Deserialización ──────────────

  # CA1: WorkflowConfig se deserializa desde TOML
  Scenario: Deserializar WorkflowConfig completo desde TOML
    When se carga la configuración desde el archivo TOML
    Then WorkflowConfig.states.initial es "draft"
    And WorkflowConfig.states.terminal contiene ["done", "failed"]
    And WorkflowConfig.roles tiene 2 roles
    And WorkflowConfig.roles[0].name es "developer"
    And WorkflowConfig.roles[1].name es "reviewer"
    And WorkflowConfig.phases tiene 2 fases
    And WorkflowConfig.phases[0].name es "implement"
    And WorkflowConfig.phases[1].name es "review"

  Scenario: WorkflowConfig.task_format se deserializa correctamente
    Given el TOML incluye:
      """
      [workflow.task_format]
      id_pattern = "ISSUE-\\d+"
      dependency_marker = "Depends on:"
      [workflow.task_format.section_markers]
      status = "## Status"
      priority = "## Priority"
      """
    When se carga la configuración
    Then WorkflowConfig.task_format.id_pattern es "ISSUE-\d+"
    And WorkflowConfig.task_format.dependency_marker es "Depends on:"
    And WorkflowConfig.task_format.section_markers["status"] es "## Status"
    And WorkflowConfig.task_format.section_markers["priority"] es "## Priority"

  Scenario: Fase incluye campos opcionales timeout_seconds
    Given el TOML incluye una fase con timeout_seconds = 300
    When se carga la configuración
    Then PhaseConfig.timeout_seconds es Some(300)

  Scenario: Fase sin timeout_seconds se deserializa como None
    Given el TOML incluye una fase sin el campo timeout_seconds
    When se carga la configuración
    Then PhaseConfig.timeout_seconds es None

  Scenario: Estados terminales pueden ser vacíos
    Given el TOML tiene terminal = []
    When se carga la configuración
    Then ningún estado se considera terminal (is_terminal siempre false)

  # ── Capa: Dominio (domain/workflow.rs) — Lógica de runtime ───────────

  # CA2: phases_for_status devuelve fases desde un estado dado
  Scenario: Consultar fases aplicables desde un estado
    Given un ConfigurableWorkflow inicializado con el WorkflowConfig del Background
    When se invoca phases_for_status("ready")
    Then devuelve exactamente 1 fase
    And la fase tiene name="implement", from="ready", to="review", role="developer"

  Scenario: Bifurcación — múltiples fases desde el mismo estado
    Given el workflow define 2 fases con from="in_review":
      | name            | to         | role       |
      | approve         | done       | reviewer   |
      | request_changes | in_progress| reviewer   |
    When se invoca phases_for_status("in_review")
    Then devuelve exactamente 2 fases
    And las fases son "approve" y "request_changes"
    And corresponde al agente elegir cuál aplicar (bifurcación)

  Scenario: phases_for_status devuelve vacío si no hay fases desde ese estado
    Given el workflow no define fases con from="done"
    When se invoca phases_for_status("done")
    Then devuelve un vector vacío

  Scenario: phases_for_status con estado desconocido devuelve vacío
    Given el estado "unknown_state" no está definido en ninguna fase
    When se invoca phases_for_status("unknown_state")
    Then devuelve un vector vacío (sin paniquear)

  # CA3: is_terminal detecta estados terminales
  Scenario: Detectar estado terminal configurado
    Given un ConfigurableWorkflow con terminal = ["done", "failed"]
    When se invoca is_terminal("done")
    Then devuelve true
    When se invoca is_terminal("failed")
    Then devuelve true

  Scenario: Estado no terminal devuelve false
    Given un ConfigurableWorkflow con terminal = ["done", "failed"]
    When se invoca is_terminal("draft")
    Then devuelve false
    When se invoca is_terminal("in_progress")
    Then devuelve false

  Scenario: Estado no definido en el workflow devuelve false
    Given un ConfigurableWorkflow inicializado
    When se invoca is_terminal("estado_inexistente")
    Then devuelve false (sin paniquear)

  Scenario: Estados terminales detienen el pipeline para esa task
    Given una task con status="done"
    And "done" es un estado terminal en el workflow
    When el orquestador evalúa si debe procesar la task
    Then la task se salta (no se invoca ningún agente)
    And el pipeline continúa con la siguiente task

  Scenario: Estados terminales customizados por el usuario
    Given el TOML define terminal = ["completed", "cancelled", "archived"]
    When se invoca is_terminal("completed")
    Then devuelve true
    And is_terminal("cancelled") devuelve true
    And is_terminal("done") devuelve false (no está en la lista)

  # ── Validación de integridad del workflow ───────────────────────────

  Scenario: Validar que el estado inicial existe en alguna fase como 'from'
    Given un WorkflowConfig con states.initial = "new"
    And ninguna fase tiene from = "new"
    When se valida el WorkflowConfig
    Then devuelve un warning: "el estado inicial 'new' no tiene fases de salida"

  Scenario: Validar que todas las fases referencian roles definidos
    Given una fase con role = "tester"
    And el workflow no define ningún rol llamado "tester"
    When se valida el WorkflowConfig
    Then devuelve un Error: "rol 'tester' referenciado en fase 'X' no está definido en roles"

  Scenario: ConfigurableWorkflow recibe &WorkflowConfig, no lo posee
    Given un WorkflowConfig cargado desde TOML por la capa app
    When se construye ConfigurableWorkflow::new(&workflow_config)
    Then ConfigurableWorkflow no clona el WorkflowConfig
    And ConfigurableWorkflow puede consultar fases y estados vía referencia

  # ── Integridad cross-layer ──────────────────────────────────────────

  Scenario: domain/workflow.rs solo importa tipos de config, no lógica
    Given el código fuente de domain/workflow.rs
    Then puede importar config::workflow::WorkflowConfig (tipo de datos)
    And NO importa config::load() ni config::save() (lógica de infraestructura)
    And NO importa ningún módulo de app/ ni infra/ ni cli/

  Scenario: config/workflow.rs no importa otras capas del crate
    Given el código fuente de config/workflow.rs
    Then solo contiene use std::*, use serde::*, use toml::*
    And NO contiene use crate::domain::
    And NO contiene use crate::app::
    And NO contiene use crate::infra::
