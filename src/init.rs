//! Generador de estructura de proyecto (`regista init`).
//!
//! Crea la estructura mínima necesaria para usar regista:
//! `.regista.toml`, skills de `pi`, e historias de ejemplo.

use std::path::Path;

/// Contenido del archivo `.regista.toml` generado por `init`.
const DEFAULT_CONFIG_TOML: &str = r#"# regista — AI agent director configuration
# Todos los campos son opcionales (se usan los defaults mostrados aquí).

[project]
stories_dir = "product/stories"
story_pattern = "STORY-*.md"
epics_dir = "product/epics"
decisions_dir = "product/decisions"
log_dir = "product/logs"

[agents]
product_owner = ".pi/skills/product-owner/SKILL.md"
qa_engineer = ".pi/skills/qa-engineer/SKILL.md"
developer = ".pi/skills/developer/SKILL.md"
reviewer = ".pi/skills/reviewer/SKILL.md"

[limits]
max_iterations = 10
max_retries_per_step = 5
max_reject_cycles = 3
agent_timeout_seconds = 1800
max_wall_time_seconds = 28800
retry_delay_base_seconds = 10

[hooks]
# post_qa = "echo 'QA phase verified'"
# post_dev = "echo 'Dev phase verified'"
# post_reviewer = "echo 'Reviewer phase verified'"

[git]
enabled = true
"#;

/// Plantilla de skill para Product Owner.
const PO_SKILL: &str = r#"# Product Owner Skill

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
const QA_SKILL: &str = r#"# QA Engineer Skill

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
const DEV_SKILL: &str = r#"# Developer Skill

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
const REVIEWER_SKILL: &str = r#"# Reviewer Skill

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
/// No sobrescribe archivos existentes (los salta con advertencia).
pub fn init(project_dir: &Path, light: bool, with_example: bool) -> anyhow::Result<InitResult> {
    let mut result = InitResult {
        created: vec![],
        skipped: vec![],
        errors: vec![],
    };

    // Crear directorio del proyecto si no existe
    std::fs::create_dir_all(project_dir)?;

    // ── .regista.toml ──────────────────────────────────────────────
    let config_path = project_dir.join(".regista.toml");
    if config_path.exists() {
        result.skipped.push(".regista.toml (ya existe)".into());
    } else {
        std::fs::write(&config_path, DEFAULT_CONFIG_TOML)?;
        result.created.push(".regista.toml".into());
    }

    // ── Directorios ────────────────────────────────────────────────
    let dirs = [
        "product/stories",
        "product/epics",
        "product/decisions",
        "product/logs",
    ];
    for dir in &dirs {
        let path = project_dir.join(dir);
        std::fs::create_dir_all(&path)?;
    }

    if !light {
        // ── Skills ─────────────────────────────────────────────────
        let skills: &[(&str, &str)] = &[
            ("product-owner", PO_SKILL),
            ("qa-engineer", QA_SKILL),
            ("developer", DEV_SKILL),
            ("reviewer", REVIEWER_SKILL),
        ];

        for (name, content) in skills {
            let skill_dir = project_dir.join(".pi/skills").join(name);
            std::fs::create_dir_all(&skill_dir)?;
            let skill_path = skill_dir.join("SKILL.md");

            if skill_path.exists() {
                result
                    .skipped
                    .push(format!(".pi/skills/{name}/SKILL.md (ya existe)"));
            } else {
                std::fs::write(&skill_path, *content)?;
                result.created.push(format!(".pi/skills/{name}/SKILL.md"));
            }
        }
    }

    // ── Historia de ejemplo ────────────────────────────────────────
    if with_example {
        let story_path = project_dir.join("product/stories/STORY-001.md");
        if story_path.exists() {
            result
                .skipped
                .push("product/stories/STORY-001.md (ya existe)".into());
        } else {
            std::fs::write(&story_path, EXAMPLE_STORY)?;
            result.created.push("product/stories/STORY-001.md".into());
        }

        let epic_path = project_dir.join("product/epics/EPIC-001.md");
        if epic_path.exists() {
            result
                .skipped
                .push("product/epics/EPIC-001.md (ya existe)".into());
        } else {
            std::fs::write(&epic_path, EXAMPLE_EPIC)?;
            result.created.push("product/epics/EPIC-001.md".into());
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
        let result = init(tmp.path(), false, false).unwrap();
        assert!(result.created.iter().any(|p| p == ".regista.toml"));
        assert!(tmp.path().join(".regista.toml").exists());
        assert!(tmp.path().join("product/stories").is_dir());
    }

    #[test]
    fn init_light_skips_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), true, false).unwrap();
        assert!(!tmp
            .path()
            .join(".pi/skills/product-owner/SKILL.md")
            .exists());
        assert!(!result.created.iter().any(|p| p.contains("SKILL.md")));
    }

    #[test]
    fn init_with_example_creates_story() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, true).unwrap();
        assert!(result.created.iter().any(|p| p.contains("STORY-001.md")));
        assert!(tmp.path().join("product/stories/STORY-001.md").exists());
        assert!(tmp.path().join("product/epics/EPIC-001.md").exists());
    }

    #[test]
    fn init_skips_existing_config() {
        let tmp = tempfile::tempdir().unwrap();
        // Crear config primero
        std::fs::write(tmp.path().join(".regista.toml"), "# ya existe").unwrap();
        let result = init(tmp.path(), false, false).unwrap();
        assert!(result.skipped.iter().any(|p| p.contains(".regista.toml")));
    }

    #[test]
    fn init_creates_full_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, true).unwrap();
        assert!(result.created.len() >= 6); // config + 4 skills + story + epic
        assert!(tmp.path().join("product/decisions").is_dir());
        assert!(tmp.path().join("product/logs").is_dir());
    }
}
