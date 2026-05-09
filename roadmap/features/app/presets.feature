# language: es
@app @presets @STORY-V10-013 @STORY-V10-014

Feature: Presets de fábrica
  Como usuario que no quiere configurar un workflow desde cero,
  quiero elegir un preset (software-dev, research, single-agent) al inicializar el proyecto,
  para tener un pipeline funcional out-of-the-box.

  # ── STORY-V10-013: software-dev ────────────────────────

  Scenario: El preset software-dev define 3 fases encadenadas
    Given el preset "software-dev" está registrado en el registry de presets
    When se obtiene Preset::workflow_config()
    Then define 3 fases: plan(draft→ready), implement(ready→review), validate(review→done)
    And el estado inicial es "draft"
    And los estados terminales son ["done", "failed"]
    And max_reject_cycles por defecto es 8

  Scenario: El preset software-dev es compatible con el formato v0.x
    Given el task_format del preset "software-dev"
    When se examina id_pattern
    Then es "STORY-\\d+"
    And section_markers incluye "status", "epic", "descripcion", "criterios", "dependencias"
    And dependency_marker es "Bloqueado por:"

  Scenario: Los system prompts incluyen instrucciones de formato
    Given el rol "developer" del preset "software-dev"
    When se examina system_prompt
    Then contiene la instrucción "Responde con [STATUS: <estado>] o [REJECT: <motivo>]"
    And referencia el formato de criterios de aceptación

  # ── STORY-V10-014: research + single-agent ─────────────

  Scenario: El preset research define un pipeline de investigación
    Given el preset "research"
    When se obtiene Preset::workflow_config()
    Then define 2 fases: research(pending→draft), report(draft→done)
    And task_format.id_pattern es "TASK-\\d+"
    And section_markers incluye "topic", "depth", "sources"

  Scenario: El preset single-agent define el pipeline mínimo
    Given el preset "single-agent"
    When se obtiene Preset::workflow_config()
    Then define 1 fase: execute(pending→done)
    And task_format.id_pattern es "TASK-\\d+"
    And section_markers incluye "description" y "priority"

  Scenario: Ambos presets son seleccionables desde CLI
    When se ejecuta regista init --preset research
    Then genera .regista/config.toml con workflow del preset research
    When se ejecuta regista init --preset single-agent
    Then genera .regista/config.toml con workflow del preset single-agent
