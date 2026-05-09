# language: es
@app @init @STORY-V10-015

Feature: Scaffolding con presets y LLM nativo
  Como usuario nuevo,
  quiero ejecutar regista init --preset <nombre> para generar un proyecto listo para usar,
  para no tener que escribir config.toml ni prompts manualmente.

  Background:
    Given un directorio vacío /tmp/nuevo-proyecto

  Scenario: init --preset software-dev genera configuración completa
    When se ejecuta regista init --preset software-dev en /tmp/nuevo-proyecto
    Then existe .regista/config.toml
    And config.toml contiene sección [models] con placeholders para gpt4o y claude
    And config.toml contiene sección [workflow] con preset = "software-dev"
    And config.toml contiene [limits], [hooks], [git]
    And NO existe el directorio .pi/skills/
    And NO existe el directorio .claude/agents/

  Scenario: init --preset custom genera template vacío
    When se ejecuta regista init --preset custom
    Then config.toml tiene [models] vacío (sin entradas)
    And [workflow] tiene una fase de ejemplo comentada
    And [workflow.states] tiene initial="draft", terminal=["done"]
    And [workflow.task_format] tiene placeholders para id_pattern y section_markers

  Scenario: init aborta si config.toml ya existe
    Given ya existe .regista/config.toml en el directorio
    When se ejecuta regista init --preset software-dev
    Then el comando falla con mensaje "config.toml ya existe"
    And el archivo existente no se modifica
    When se ejecuta regista init --preset software-dev --force
    Then el archivo existente se sobreescribe
    And el comando termina exitosamente
