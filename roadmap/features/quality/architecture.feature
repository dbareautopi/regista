# language: es
@quality @architecture @R1 @R2 @R3 @R4 @R5

Feature: Verificación de reglas de arquitectura
  Como maintainer del proyecto,
  quiero tests automáticos que verifiquen las reglas de dependencia entre capas,
  para que ninguna violación arquitectónica llegue a producción.

  Background:
    Given el código fuente en src/ con la estructura de capas:
      | capa    | directorio      | restricción                                      |
      | cli     | src/cli/        | puede importar cualquier capa (R4)               |
      | app     | src/app/        | no puede importar cli/ (R3)                      |
      | domain  | src/domain/     | no puede importar infra/, app/, cli/, config (R1)|
      | infra   | src/infra/      | no puede importar domain/, app/, cli/ (R2)       |
      | config  | src/config.rs   | no puede importar ningún módulo del crate (R5)   |

  # ── R1: domain/ ─────────────────────────────────────

  Scenario: domain/task.rs no importa infra, app, cli ni config
    When se analizan los imports de domain/task.rs
    Then no contiene use crate::infra::
    And no contiene use crate::app::
    And no contiene use crate::cli::
    And no contiene use crate::config::

  Scenario: domain/workflow.rs solo importa std y otros módulos domain
    When se analizan los imports de domain/workflow.rs
    Then todos los use crate:: apuntan a domain:: (misma capa)
    And no hay imports de crate::infra, crate::app, crate::cli, crate::config

  Scenario: domain/templates.rs no depende de infraestructura
    When se analizan los imports de domain/templates.rs
    Then no contiene use anyhow::
    And no contiene use serde::
    And no contiene use reqwest::

  # ── R2: infra/ y infra/llm/ ─────────────────────────

  Scenario: infra/llm/openai.rs no importa dominio ni aplicación
    When se analizan los imports de infra/llm/openai.rs
    Then no contiene use crate::domain::
    And no contiene use crate::app::

  Scenario: infra/llm/types.rs no depende de domain/task.rs
    When se analizan los imports de infra/llm/types.rs
    Then no contiene use crate::domain::task::
    And los tipos Message y ChatResponse son autocontenidos

  Scenario: infra/llm/retry.rs opera solo sobre el trait LlmProvider
    When se analizan los imports de infra/llm/retry.rs
    Then no contiene use crate::domain::
    And no contiene use crate::app::
    And todos los use crate:: son a infra::llm:: o crate::config::

  # ── R3: app/ ────────────────────────────────────────

  Scenario: app/pipeline.rs no importa cli
    When se analizan los imports de app/pipeline.rs
    Then no contiene use crate::cli::

  Scenario: app/presets/software_dev.rs no importa infraestructura
    When se analizan los imports de app/presets/software_dev.rs
    Then no contiene use crate::infra::llm::
    And solo referencia tipos de dominio y configuración

  # ── R4: cli/ (sin restricciones) ────────────────────

  Scenario: cli/handlers.rs puede importar cualquier capa
    When se analizan los imports de cli/handlers.rs
    Then no se reportan violaciones de capa
    And el test de arquitectura no falla por imports legítimos desde cli/

  # ── R5: config ──────────────────────────────────────

  Scenario: config.rs no importa módulos del crate
    When se analizan los imports de config.rs
    Then no existe ningún use crate:: (excepto use crate::config:: para submódulos si los hay)
    And todos los imports son de std, serde, o toml

  # ── Tests de integridad ─────────────────────────────

  Scenario: No hay ciclos de dependencia entre capas
    Given el grafo de dependencias entre capas (A importa B)
    When se construye el grafo dirigido de imports entre capas
    Then no existen ciclos (A→B y B→A simultáneamente)

  Scenario: Los módulos eliminados en v1.0 ya no existen
    Given el código fuente en src/
    When se verifica la existencia de los siguientes archivos:
      | archivo                    |
      | infra/providers.rs         |
      | infra/agent.rs             |
      | domain/story.rs            |
    Then ninguno de ellos existe
