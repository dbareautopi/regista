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

      [[workflow.phases]]
      name = "implement"
      from = "ready"
      to = "review"
      role = "developer"
      model = "gpt4o"
      prompt = "Implementa la tarea {{task_id}}"
      on_reject = "ready"
      max_reject_cycles = 3
      """

  Scenario: Deserializar WorkflowConfig completo desde TOML
    When se carga la configuración desde el archivo TOML
    Then WorkflowConfig.states.initial es "draft"
    And WorkflowConfig.states.terminal contiene ["done", "failed"]
    And WorkflowConfig.roles tiene 1 rol con nombre "developer"
    And WorkflowConfig.phases tiene 1 fase con nombre "implement"

  Scenario: Consultar fases aplicables desde un estado
    Given un ConfigurableWorkflow inicializado con WorkflowConfig
    When se invoca phases_for_status("ready")
    Then devuelve exactamente 1 fase: "implement"
    And la fase tiene from="ready" y to="review"

  Scenario: Detectar estado terminal
    Given un ConfigurableWorkflow inicializado
    When se invoca is_terminal("done")
    Then devuelve true
    When se invoca is_terminal("draft")
    Then devuelve false
