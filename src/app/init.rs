//! Generador de estructura de proyecto (`regista init`).
//!
//! Crea la estructura mínima necesaria para usar regista:
//! `.regista.toml`, skills de `pi`, e historias de ejemplo.

use crate::config::AgentsConfig;
use crate::infra::providers;
use std::path::Path;

/// Contenido del archivo `.regista/config.toml` generado por `init`.
/// Construye el contenido de `.regista/config.toml` para un provider dado.
fn build_config_toml(provider_name: &str) -> String {
    format!(
        r#"# regista — AI agent director configuration
# Todos los campos son opcionales (se usan los defaults mostrados aquí).

[project]
stories_dir = ".regista/stories"
story_pattern = "STORY-*.md"
epics_dir = ".regista/epics"
decisions_dir = ".regista/decisions"
log_dir = ".regista/logs"

[agents]
provider = "{provider_name}"

[limits]
max_iterations = 0  # 0 = auto: nº de historias × 6
max_retries_per_step = 5
max_reject_cycles = 8
agent_timeout_seconds = 1800
max_wall_time_seconds = 28800
retry_delay_base_seconds = 10

[hooks]
# post_qa = "echo 'QA phase verified'"
# post_dev = "echo 'Dev phase verified'"
# post_reviewer = "echo 'Reviewer phase verified'"

[git]
enabled = true
"#
    )
}

/// Devuelve el contenido del archivo de instrucciones para un rol dado.
fn role_instruction_content(role: &str) -> &'static str {
    match role {
        "product_owner" => PO_SKILL,
        "qa_engineer" => QA_SKILL,
        "developer" => DEV_SKILL,
        "reviewer" => REVIEWER_SKILL,
        _ => "# Unknown role\n",
    }
}

/// Plantilla de skill para Product Owner.
const PO_SKILL: &str = r#"---
name: product-owner
description: Product Owner role for regista — refines and validates user stories to ensure they deliver business value. Handles Draft→Ready and Business Review→Done transitions.
model: opencode/minimax-m2.5-free
---

# Product Owner Skill

Eres un **Product Owner**. Tu responsabilidad es refinar y validar historias de usuario para asegurar que entregan valor de negocio.

## Tus tareas

### 1. Refinamiento (Draft → Ready)
- Lee la historia desde el directorio de historias.
- Verifica que cumple el **Definition of Ready**:
  - Descripción clara y no ambigua.
  - Criterios de aceptación específicos y testeables.
  - Dependencias identificadas (si existen).
- Si está lista, cambia el status de **Draft** a **Ready**.
- Si no está lista, explica en el Activity Log qué falta.

### 2. Validación (Business Review → Done)
- Lee la historia completada.
- Verifica que el valor de negocio se cumple:
  - ¿Los criterios de aceptación están satisfechos?
  - ¿Lo implementado coincide con lo solicitado?
- Si OK → cambia status a **Done**.
- Si rechazo leve → cambia a **In Review** con feedback concreto.
- Si rechazo grave → cambia a **In Progress** con detalles específicos.

## Reglas
- Documenta decisiones de producto en el directorio de decisiones.
- Formato de Activity Log: `- YYYY-MM-DD | PO | descripción`.
- **NO preguntes nada al usuario. Trabaja de forma 100% autónoma.**
- Siempre lee el contexto completo antes de actuar.
"#;

/// Plantilla de skill para QA Engineer.
const QA_SKILL: &str = r#"---
name: qa-engineer
description: QA Engineer role for regista — writes and maintains automated tests for user stories. Handles Ready→Tests Ready and Tests Ready→Tests Ready (fix) transitions.
model: opencode/minimax-m2.5-free
---

# QA Engineer Skill

Eres un **QA Engineer**. Tu responsabilidad es escribir y mantener tests automatizados para las historias de usuario.

## Tus tareas

### 1. Escribir tests (Ready → Tests Ready)
- Lee la historia desde el directorio de historias.
- Escribe tests automatizados para CADA criterio de aceptación.
- Los tests deben ser ejecutables y cubrir casos edge.
- Cambia el status de **Ready** a **Tests Ready**.
- Si algún criterio no es testeable, revierte a **Draft** con explicación.

### 2. Corregir tests (Tests Ready → Tests Ready)
- Si el Developer reporta problemas con los tests:
  - Lee el Activity Log para entender el issue.
  - Corrige los tests.
  - El status se mantiene en **Tests Ready**.
  - Documenta qué corregiste y por qué.

## Reglas
- Si necesitas crear archivos placeholder (src/lib.rs, etc.) para que los tests compilen, hazlo.
- Documenta decisiones de testing en el directorio de decisiones.
- Formato de Activity Log: `- YYYY-MM-DD | QA | descripción`.
- **NO preguntes nada al usuario. 100% autónomo.**
- Ejecuta los tests antes de marcar como completado para verificar que compilan.
"#;

/// Plantilla de skill para Developer.
const DEV_SKILL: &str = r#"---
name: developer
description: Developer role for regista — implements code to make tests pass and satisfy acceptance criteria. Handles Tests Ready→In Review and In Progress→In Review (fix) transitions.
model: opencode/minimax-m2.5-free
---

# Developer Skill

Eres un **Developer**. Tu responsabilidad es implementar el código que hace pasar los tests y cumple los criterios de aceptación.

## Tus tareas

### 1. Implementar (Tests Ready → In Review)
- Lee la historia desde el directorio de historias.
- Los tests ya existen (QA los escribió). Búscalos y haz que pasen.
- Implementa en el código fuente siguiendo las convenciones del proyecto.
- Ejecuta build + tests para verificar.
- Cambia el status de **Tests Ready** a **In Review**.

### 2. Corregir (In Progress → In Review)
- Si el Reviewer o PO rechazó la implementación:
  - Lee el Activity Log para el feedback detallado.
  - Corrige los problemas indicados.
  - Cambia el status de **In Progress** a **In Review**.

## Reglas
- Si los tests no compilan o están rotos, repórtalo al QA en el Activity Log.
  El formato es: `- YYYY-MM-DD | Dev | Tests rotos: descripción del problema`.
- Documenta decisiones de arquitectura en el directorio de decisiones.
- Formato de Activity Log: `- YYYY-MM-DD | Dev | descripción`.
- **NO preguntes nada al usuario. 100% autónomo.**
- Siempre ejecuta build + tests antes de marcar como completado.
"#;

/// Plantilla de skill para Reviewer.
const REVIEWER_SKILL: &str = r#"---
name: reviewer
description: Reviewer role for regista — technical gate that verifies code meets standards before business validation. Handles In Review→Business Review and In Review→In Progress (reject) transitions.
model: opencode/minimax-m2.5-free
---

# Reviewer Skill

Eres un **Reviewer**. Tu responsabilidad es la puerta técnica: verificar que el código cumple los estándares antes de la validación de negocio.

## Tus tareas

### Revisión técnica (In Review → Business Review / In Progress)
- Lee la historia desde el directorio de historias.
- Verifica el **Definition of Done** técnico:
  - ¿Compila sin errores?
  - ¿Todos los tests pasan?
  - ¿El código sigue las convenciones del proyecto?
  - ¿No hay regresiones?
- Si TODO OK → cambia status a **Business Review**.
- Si algo falla:
  - Cambia a **In Progress**.
  - Proporciona feedback CONCRETO: archivo, línea, y naturaleza del problema.
  - No rechaces por opiniones subjetivas; solo por criterios objetivos.

## Reglas
- Ejecuta las herramientas de verificación del proyecto (cargo test, clippy, fmt, etc.).
- Documenta hallazgos en el directorio de decisiones.
- Formato de Activity Log: `- YYYY-MM-DD | Reviewer | resultado`.
- **NO preguntes nada al usuario. 100% autónomo.**
"#;

/// Plantilla de historia de ejemplo (STORY-001).
const EXAMPLE_STORY: &str = r#"# STORY-001: Ejemplo de historia de usuario

## Status
**Draft**

## Epic
EPIC-001

## Descripción
Esta es una historia de ejemplo para demostrar el formato esperado por regista.
Modifícala o elimínala para empezar tu propio proyecto.

## Criterios de aceptación
- [ ] CA1: El proyecto compila correctamente
- [ ] CA2: Los tests pasan

## Dependencias

## Activity Log
- 2026-04-30 | PO | Historia de ejemplo creada por `regista init`.
"#;

/// Plantilla de épica de ejemplo.
const EXAMPLE_EPIC: &str = r#"# EPIC-001: Épica de ejemplo

## Descripción
Épica de ejemplo generada por `regista init`.

## Historias
- STORY-001
"#;

/// Resultado de la operación `init`.
#[derive(Debug)]
pub struct InitResult {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

/// Genera la estructura de un proyecto regista.
///
/// `provider_name` determina qué agente usar y dónde guardar las
/// instrucciones de rol. Por defecto "pi".
///
/// No sobrescribe archivos existentes (los salta con advertencia).
pub fn init(
    project_dir: &Path,
    light: bool,
    with_example: bool,
    provider_name: &str,
) -> anyhow::Result<InitResult> {
    let provider = providers::from_name(provider_name);
    let mut result = InitResult {
        created: vec![],
        skipped: vec![],
        errors: vec![],
    };

    // Crear directorio del proyecto si no existe
    std::fs::create_dir_all(project_dir)?;

    // ── .regista/config.toml ────────────────────────────────────
    let config_path = project_dir.join(".regista/config.toml");
    if config_path.exists() {
        result
            .skipped
            .push(".regista/config.toml (ya existe)".into());
    } else {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let config_content = build_config_toml(provider_name);
        std::fs::write(&config_path, config_content)?;
        result.created.push(".regista/config.toml".into());
    }

    // ── Directorios ────────────────────────────────────────────────
    let dirs = [
        ".regista/stories",
        ".regista/epics",
        ".regista/decisions",
        ".regista/logs",
    ];
    for dir in &dirs {
        let path = project_dir.join(dir);
        std::fs::create_dir_all(&path)?;
    }

    if !light {
        // ── Instrucciones de rol ──────────────────────────────────
        let roles = AgentsConfig::all_roles();
        for role in &roles {
            let instruction_path_str = provider.instruction_dir(role);
            let instruction_path = project_dir.join(&instruction_path_str);

            if let Some(parent) = instruction_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            if instruction_path.exists() {
                result
                    .skipped
                    .push(format!("{instruction_path_str} (ya existe)"));
            } else {
                let content = role_instruction_content(role);
                std::fs::write(&instruction_path, content)?;
                result.created.push(instruction_path_str);
            }
        }
    }

    // ── Historia de ejemplo ────────────────────────────────────────
    if with_example {
        let story_path = project_dir.join(".regista/stories/STORY-001.md");
        if story_path.exists() {
            result
                .skipped
                .push(".regista/stories/STORY-001.md (ya existe)".into());
        } else {
            std::fs::write(&story_path, EXAMPLE_STORY)?;
            result.created.push(".regista/stories/STORY-001.md".into());
        }

        let epic_path = project_dir.join(".regista/epics/EPIC-001.md");
        if epic_path.exists() {
            result
                .skipped
                .push(".regista/epics/EPIC-001.md (ya existe)".into());
        } else {
            std::fs::write(&epic_path, EXAMPLE_EPIC)?;
            result.created.push(".regista/epics/EPIC-001.md".into());
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_config_in_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, false, "pi").unwrap();
        assert!(result.created.iter().any(|p| p == ".regista/config.toml"));
        assert!(tmp.path().join(".regista/config.toml").exists());
        assert!(tmp.path().join(".regista/stories").is_dir());
    }

    #[test]
    fn init_light_skips_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), true, false, "pi").unwrap();
        assert!(!tmp
            .path()
            .join(".pi/skills/product-owner/SKILL.md")
            .exists());
        assert!(!result.created.iter().any(|p| p.contains("SKILL.md")));
    }

    #[test]
    fn init_with_example_creates_story() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, true, "pi").unwrap();
        assert!(result.created.iter().any(|p| p.contains("STORY-001.md")));
        assert!(tmp.path().join(".regista/stories/STORY-001.md").exists());
        assert!(tmp.path().join(".regista/epics/EPIC-001.md").exists());
    }

    #[test]
    fn init_skips_existing_config() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".regista")).unwrap();
        std::fs::write(tmp.path().join(".regista/config.toml"), "# ya existe").unwrap();
        let result = init(tmp.path(), false, false, "pi").unwrap();
        assert!(result
            .skipped
            .iter()
            .any(|p| p.contains(".regista/config.toml")));
    }

    #[test]
    fn init_creates_full_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, true, "pi").unwrap();
        assert!(result.created.len() >= 6); // config + 4 skills + story + epic
        assert!(tmp.path().join(".regista/decisions").is_dir());
        assert!(tmp.path().join(".regista/logs").is_dir());
    }

    #[test]
    fn init_with_claude_creates_agent_files() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, false, "claude").unwrap();
        assert!(result
            .created
            .iter()
            .any(|p| p.contains(".claude/agents/product_owner.md")));
        assert!(tmp.path().join(".claude/agents/product_owner.md").exists());
    }

    #[test]
    fn init_with_codex_creates_skill_files() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, false, "codex").unwrap();
        assert!(result
            .created
            .iter()
            .any(|p| p.contains(".agents/skills/developer/SKILL.md")));
    }
}
