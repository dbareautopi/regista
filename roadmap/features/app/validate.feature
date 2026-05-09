# language: es
@app @validate @STORY-V10-017

Feature: Validación pre-vuelo de configuración genérica
  Como usuario que quiere evitar pipelines fallidos,
  quiero ejecutar regista validate para detectar errores de configuración antes de lanzar,
  para no gastar créditos de LLM en un pipeline que va a fallar por mala configuración.

  Background:
    Given un proyecto con .regista/config.toml y tareas en .regista/tasks/

  Scenario: Validar coherencia de modelos referenciados
    Given el workflow referencia model="gemini" en una fase
    And [models] no contiene la clave "gemini"
    When se ejecuta regista validate
    Then se emite un Finding con severity=Error y category="models"
    And el mensaje contiene "modelo 'gemini' referenciado en fase 'X' no está definido"

  Scenario: Validar que id_pattern es un regex válido
    Given task_format.id_pattern es "***[invalid"
    When se ejecuta regista validate
    Then se emite un Finding con severity=Error y category="task_format"
    And el mensaje contiene "id_pattern no es un regex válido"

  Scenario: Validar coherencia de fases con estados
    Given una fase define from="validating" pero "validating" no está en workflow.states
    When se ejecuta regista validate
    Then se emite un Finding con severity=Error y category="workflow"
    And el mensaje contiene "estado 'validating' no definido en workflow.states"
