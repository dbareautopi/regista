# language: es
@quality @cleanup @STORY-V10-019 @STORY-V10-020

Feature: Limpieza de código obsoleto y migración de dependencias
  Como maintainer del proyecto,
  quiero eliminar todo el código de v0.x que ha sido reemplazado por los nuevos módulos,
  para reducir la superficie de mantenimiento y eliminar dependencias innecesarias.

  # ── STORY-V10-019: Eliminar providers + agent ──────────

  Scenario: infra/providers.rs ya no existe
    Given el código fuente en src/
    When se verifica la existencia de infra/providers.rs
    Then el archivo no existe

  Scenario: infra/agent.rs ya no existe
    Given el código fuente en src/
    When se verifica la existencia de infra/agent.rs
    Then el archivo no existe

  Scenario: La compilación es limpia sin los módulos eliminados
    Given todos los imports de crate::infra::providers han sido eliminados
    And todos los imports de crate::infra::agent han sido eliminados
    When se ejecuta cargo build
    Then compila sin errores
    And cargo test pasa todos los tests unitarios restantes
    And cargo clippy -- -D warnings no reporta warnings

  # ── STORY-V10-020: Eliminar dominio hardcodeado ────────

  Scenario: domain/story.rs ya no existe
    Given el código fuente en src/
    When se verifica la existencia de domain/story.rs
    Then el archivo no existe

  Scenario: domain/workflow.rs antiguo ha sido reemplazado
    Given el código fuente en src/domain/
    When se examina domain/workflow.rs
    Then solo contiene ConfigurableWorkflow (no CanonicalWorkflow ni las 14 transiciones fijas)
    And no importa spartito

  Scenario: Cargo.toml no depende de spartito ni ureq
    Given el archivo Cargo.toml
    When se examinan las dependencias
    Then no existe la entrada "spartito"
    And no existe la entrada "ureq"
    And existe la entrada "reqwest" con features ["json", "rustls-tls"]
