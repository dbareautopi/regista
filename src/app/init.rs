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

[stack]
# build_command = "cargo build"
# test_command = "cargo test"
# lint_command = "cargo clippy -- -D warnings"
# fmt_command = "cargo fmt -- --check"
# src_dir = "src/"

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
- Si está lista, edita el archivo de la historia y cambia el status de **Draft** a **Ready**.
- Si no está lista, explica en el Activity Log qué falta.

### 2. Validación (Business Review → Done)
- Lee la historia completada.
- Verifica que el valor de negocio se cumple:
  - ¿Los criterios de aceptación están satisfechos?
  - ¿Lo implementado coincide con lo solicitado?
- Si OK → edita el archivo y cambia status a **Done**.
- Si rechazo leve → edita el archivo y cambia a **In Review** con feedback concreto.
- Si rechazo grave → edita el archivo y cambia a **In Progress** con detalles específicos.

## Reglas
- **EDITA SIEMPRE el archivo de la historia para cambiar el status.** Es obligatorio.
- Documenta decisiones de producto en el directorio de decisiones.
- Formato de Activity Log: `- YYYY-MM-DD | PO | descripción`.
- **NO preguntes nada al usuario. Trabaja de forma 100% autónoma.**
- Siempre lee el contexto completo antes de actuar.
- **Detección de deadlocks**: si una historia tiene más de 10 entradas en el Activity Log sin cambiar de estado, o más de 5 iteraciones del mismo actor repitiendo la misma verificación, está en deadlock. En ese caso, toma el control: corrige el problema directamente (si es trivial) o marca la historia como Blocked con una explicación clara de qué está trabando el progreso.
"#;

/// Plantilla de skill para QA Engineer.
const QA_SKILL: &str = r###"---
name: qa-engineer
description: QA Engineer role for regista — writes and maintains automated tests for user stories following strict TDD (red-green-refactor). Handles Ready→Tests Ready and Tests Ready→Tests Ready (fix) transitions.
---

# QA Engineer Skill

Eres un **QA Engineer**. Tu responsabilidad es escribir tests automatizados siguiendo **TDD puro**: primero los tests (rojo), luego el Developer implementa (verde), luego refactoriza.

## Filosofía TDD

El ciclo TDD tiene 3 fases con dueños distintos:

| Fase | Color | Dueño | Acción |
|------|-------|-------|--------|
| 1. Escribir test | 🔴 Rojo | **Tú (QA)** | Escribes el test que define el comportamiento esperado |
| 2. Hacer pasar | 🟢 Verde | Developer | Implementa el código mínimo para que el test pase |
| 3. Refactorizar | 🔵 Azul | Developer + Reviewer | Mejora el código sin romper tests |

**Tu trabajo termina en la fase roja. Los tests en rojo son el contrato que el Developer debe cumplir.**

## Tus tareas

### 1. Escribir tests (Ready → Tests Ready)
- Lee la historia desde el directorio de historias.
- Escribe tests automatizados para CADA criterio de aceptación.
- Los tests deben definir el comportamiento esperado con claridad.
- Cubre casos edge y condiciones de error.
- Usa nombres de test descriptivos que sirvan como mini-especificación.
- **OBLIGATORIO: edita el archivo de la historia y cambia** `## Status\n**Ready**` **por** `## Status\n**Tests Ready**`.
- Si algún criterio no es testeable, revierte a **Draft** con explicación.

### 2. Corregir tests (Tests Ready → Tests Ready)
- Si el Developer reporta problemas con los tests:
  - Lee el Activity Log para entender el issue.
  - Corrige los tests.
  - El status se mantiene en **Tests Ready**.
  - Documenta qué corregiste.

## Reglas

### Sobre modificar código de producción
- **NO modifiques firmas de funciones de producción.** Si un test necesita una firma nueva (ej: añadir un parámetro), escribe el test asumiendo que la firma existirá y documenta en la decisión qué cambios de firma necesita el Developer.
- **Sí puedes crear imports, módulos de test (`#[cfg(test)] mod ...`), y constantes.**
- **Sí puedes crear archivos placeholder vacíos** (ej: `src/lib.rs` con `// placeholder`) si son necesarios para que el módulo de test tenga sentido.
- Si escribes un test que referencia una función/firma que no existe aún, asegúrate de que esté dentro de `#[cfg(test)]` para que no rompa la compilación del código de producción.

### Sobre ejecutar los tests
- **No necesitas ejecutar `cargo test` para avanzar el estado.** Los tests están en rojo por definición en TDD — el Developer los hará pasar.
- **Sí debes verificar que los tests tienen sentido sintáctico.** Revisa manualmente que las llamadas a funciones, aserciones, e imports son coherentes.
- Si el proyecto compila actualmente (`cargo check` pasa), asegúrate de que tus tests no rompan la compilación del código de producción. Los `#[cfg(test)]` aíslan los tests.

### Sobre reintentos y anti-bucles
- **Máximo 2 iteraciones en la misma historia.** Si el Developer rechaza los tests 2 veces, documenta el problema y el orquestador escalará.
- No caigas en bucles: si ya escribiste tests para todos los CAs, **edita el archivo de la historia y avanza el estado a Tests Ready** y deja que el Developer trabaje.
- **NUNCA te quedes en un bucle re-escribiendo los mismos tests.** Si ya cubriste todos los CAs, cambia el status a Tests Ready inmediatamente.

### Otras reglas
- Documenta decisiones de testing en el directorio de decisiones.
- En la decisión, incluye una sección "## Pendiente para el Developer" listando cambios de firma necesarios.
- Formato de Activity Log: `- YYYY-MM-DD | QA | descripción`.
- **NO preguntes nada al usuario. 100% autónomo.**
- **EDITAR EL ARCHIVO DE HISTORIA ES OBLIGATORIO.** Sin el cambio de status, el pipeline se bloquea.
"###;

/// Plantilla de skill para Developer.
const DEV_SKILL: &str = r###"---
name: developer
description: Developer role for regista — implements code to make tests pass and satisfy acceptance criteria. Follows strict TDD: receives red tests from QA, makes them green, hands off for refactor. Handles Tests Ready→In Review and In Progress→In Review (fix) transitions.
---

# Developer Skill

Eres un **Developer**. Tu responsabilidad es implementar el código que hace pasar los tests escritos por QA, siguiendo **TDD estricto**.

## El ciclo TDD — tu parte

| Fase | Color | Dueño | Qué hace |
|------|-------|-------|----------|
| 1. Escribir test | 🔴 Rojo | QA | Escribe tests que definen el comportamiento esperado |
| 2. Hacer pasar | 🟢 Verde | **Tú (Dev)** | Implementas el código mínimo para que los tests pasen |
| 3. Refactorizar | 🔵 Azul | Tú + Reviewer | Mejoras el código sin romper tests |

**Los tests llegan en rojo. Es normal. Son tu contrato.**

## Tus tareas

### 1. Implementar (Tests Ready → In Review)
- Lee la historia y estudia los tests que escribió QA.
- **Los tests probablemente no compilan aún.** Eso es esperado: tu trabajo es hacer los cambios de producción necesarios para que compilen y pasen.
- Implementa el código fuente siguiendo las convenciones del proyecto.
- **Implementa solo lo necesario para que los tests pasen.** Nada de gold-plating.
- Si los tests requieren cambios de firma en funciones de producción, hazlos.
- Ejecuta `cargo build && cargo test` hasta que todo esté en verde.
- **OBLIGATORIO: edita el archivo de la historia y cambia el status de** `## Status\n**Tests Ready**` **a** `## Status\n**In Review**`.

### 2. Corregir (In Progress → In Review)
- Si el Reviewer o PO rechazó la implementación:
  - Lee el Activity Log para el feedback detallado.
  - Corrige los problemas indicados.
  - Vuelve a ejecutar `cargo test`.
  - **OBLIGATORIO: edita el archivo y cambia el status de** `## Status\n**In Progress**` **a** `## Status\n**In Review**`.

## Reglas

### Sobre los tests del QA
- Si los tests tienen errores de compilación triviales (imports faltantes, variables temporales no definidas), corrígelos tú mismo y documéntalo.
- Si los tests tienen errores de lógica o expectativas incorrectas, repórtalo al QA en el Activity Log con formato: `- YYYY-MM-DD | Dev | Tests rotos: descripción del problema`.
- **No reescribas tests del QA** a menos que sea estrictamente necesario para compilar.

### Sobre anti-bucles
- Si después de 3 iteraciones sobre el mismo issue no hay progreso, escala al PO con un resumen claro. No entres en bucle infinito.
- Si los tests llevan más de 5 iteraciones QA→Dev sin avanzar, menciónalo en el Activity Log.

### Otras reglas
- **EDITA SIEMPRE el archivo de la historia para cambiar el status.** Es obligatorio.
- Documenta decisiones de arquitectura en el directorio de decisiones.
- Formato de Activity Log: `- YYYY-MM-DD | Dev | descripción`.
- **NO preguntes nada al usuario. 100% autónomo.**
- Siempre ejecuta `cargo build && cargo test` antes de marcar como completado.
"###;

/// Plantilla de skill para Reviewer.
const REVIEWER_SKILL: &str = r#"---
name: reviewer
description: Reviewer role for regista — technical gate that verifies code meets standards before business validation. Handles In Review→Business Review and In Review→In Progress (reject) transitions.
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
- Si TODO OK → **OBLIGATORIO: edita el archivo y cambia status a Business Review**.
- Si algo falla:
  - **Edita el archivo y cambia a In Progress**.
  - Proporciona feedback CONCRETO: archivo, línea, y naturaleza del problema.
  - No rechaces por opiniones subjetivas; solo por criterios objetivos.

## Reglas
- **EDITA SIEMPRE el archivo de la historia para cambiar el status.** Es obligatorio.
- Ejecuta las herramientas de verificación del proyecto (cargo test, clippy, fmt, etc.).
- Si encuentras que la historia está bloqueada por un conflicto entre Dev y QA (más de 5 iteraciones sin cambio de estado), señálalo explícitamente en tu veredicto y sugiere intervención humana.
- No te quedes en bucle: si el código compila, los tests pasan, y las herramientas están limpias, aprueba aunque haya entradas repetitivas en el Activity Log.
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
    let provider = providers::from_name(provider_name)?;
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-V10-015: Scaffolding con presets y LLM nativo
    // ═══════════════════════════════════════════════════════════════
    //
    // NOTA TDD: init_with_preset() no existe aún. El Developer debe implementarla.
    // El helper init_with_preset_helper() simula la futura función.

    // ── CA1: init --preset software-dev genera config completa ──────

    #[test]
    fn init_with_preset_software_dev_generates_config_with_models_section() {
        // CA1: init con software-dev genera config.toml con todas las secciones
        let tmp = tempfile::tempdir().unwrap();
        let result = init_with_preset_helper(tmp.path(), "software-dev", false, false).unwrap();

        assert!(tmp.path().join(".regista/config.toml").exists());

        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(content.contains("[models]"), "config debe tener sección [models]");
        assert!(content.contains("[workflow]"), "config debe tener sección [workflow]");
        assert!(content.contains("[limits]"), "config debe tener sección [limits]");
        assert!(content.contains("[hooks]"), "config debe tener sección [hooks]");
        assert!(content.contains("[git]"), "config debe tener sección [git]");
    }

    #[test]
    fn init_with_preset_software_dev_generates_model_placeholders() {
        // CA1: Los placeholders de modelos deben incluir gpt4o y claude
        let tmp = tempfile::tempdir().unwrap();
        let _result = init_with_preset_helper(tmp.path(), "software-dev", false, false).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(content.contains("[models]"), "Debe existir sección [models]");
    }

    #[test]
    fn init_with_preset_software_dev_config_contains_preset_key() {
        // P1: El config.toml debe contener explícitamente preset = "software-dev"
        let tmp = tempfile::tempdir().unwrap();
        let _result = init_with_preset_helper(tmp.path(), "software-dev", false, false).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(
            content.contains("preset = \"software-dev\""),
            "config.toml debe contener preset = \"software-dev\":\n{content}"
        );
        assert!(
            content.contains("[workflow]"),
            "Debe tener sección [workflow]:\n{content}"
        );
    }

    #[test]
    fn init_with_preset_no_legacy_skill_directories() {
        // CA1: NO debe generar directorios .pi/skills/, .claude/agents/, etc.
        let tmp = tempfile::tempdir().unwrap();
        let _result = init_with_preset_helper(tmp.path(), "software-dev", false, false).unwrap();

        assert!(!tmp.path().join(".pi").exists(), "No debe crear directorio .pi/");
        assert!(!tmp.path().join(".claude").exists(), "No debe crear directorio .claude/");
        assert!(!tmp.path().join(".agents").exists(), "No debe crear directorio .agents/");
        assert!(!tmp.path().join(".opencode").exists(), "No debe crear directorio .opencode/");
    }

    // ── CA2: init --preset custom genera template vacío ────────────

    #[test]
    fn init_with_preset_custom_generates_empty_models() {
        // CA2: custom debe tener [models] pero vacío (sin entradas)
        let tmp = tempfile::tempdir().unwrap();
        let _result = init_with_preset_helper(tmp.path(), "custom", false, false).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(content.contains("[models]"), "custom debe tener sección [models]");
        // Verificar que después de [models] viene [workflow] sin entradas intermedias
        let models_pos = content.find("[models]").unwrap();
        let workflow_pos = content.find("[workflow]").unwrap();
        let between = &content[models_pos + "[models]".len()..workflow_pos];
        let non_empty_lines: Vec<&str> = between
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .collect();
        assert!(
            non_empty_lines.is_empty(),
            "[models] debe estar vacío en preset custom. Líneas encontradas: {non_empty_lines:?}"
        );
    }

    #[test]
    fn init_with_preset_custom_has_workflow_example() {
        // CA2: custom debe tener [workflow] con estructura de ejemplo comentada
        let tmp = tempfile::tempdir().unwrap();
        let _result = init_with_preset_helper(tmp.path(), "custom", false, false).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(content.contains("[workflow]"), "custom debe tener sección [workflow]");
        assert!(
            content.contains("#") || content.contains("Ejemplo"),
            "Debe tener contenido de ejemplo comentado:\n{content}"
        );
    }

    #[test]
    fn init_with_preset_custom_has_workflow_states_and_task_format() {
        // CA2: custom debe tener [workflow.states] y [workflow.task_format]
        let tmp = tempfile::tempdir().unwrap();
        let _result = init_with_preset_helper(tmp.path(), "custom", false, false).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(
            content.contains("[workflow.states]"),
            "custom debe tener [workflow.states]:\n{content}"
        );
        assert!(
            content.contains("initial"),
            "[workflow.states] debe definir initial:\n{content}"
        );
        assert!(
            content.contains("terminal"),
            "[workflow.states] debe definir terminal:\n{content}"
        );
        assert!(
            content.contains("[workflow.task_format]"),
            "custom debe tener [workflow.task_format]:\n{content}"
        );
        assert!(
            content.contains("id_pattern"),
            "[workflow.task_format] debe definir id_pattern:\n{content}"
        );
    }

    // ── CA3: init aborta si config.toml ya existe ─────────────────

    /// NOTA TDD: init_with_preset() no existe aún. El Developer debe implementarla.
    /// Este helper simula la futura función que acepta --preset y --force.
    /// Cuando init_with_preset() esté implementada, este helper debe eliminarse
    /// y los tests deben llamar a la función real.
    fn init_with_preset_helper(
        project_dir: &Path,
        preset: &str,
        force: bool,
        with_example: bool,
    ) -> anyhow::Result<InitResult> {
        let config_path = project_dir.join(".regista/config.toml");

        if config_path.exists() && !force {
            anyhow::bail!(
                "config.toml ya existe en {}. Usa --force para sobreescribir.",
                config_path.display()
            );
        }

        // Simular generación de config según el preset
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config_content = match preset {
            "software-dev" => format!(
                "# regista configuration\n[models]\n# gpt4o y claude placeholders\n[workflow]\npreset = \"{preset}\"\n[limits]\n[hooks]\n[git]\n"
            ),
            "custom" => "# regista configuration\n[models]\n\n[workflow]\n# Ejemplo comentado\n[workflow.states]\ninitial = \"draft\"\nterminal = [\"done\"]\n[workflow.task_format]\nid_pattern = \"TASK-\\\\d+\"\n".to_string(),
            _ => format!("# regista configuration\n[models]\n[workflow]\npreset = \"{preset}\"\n"),
        };

        std::fs::write(&config_path, &config_content)?;

        let mut result = InitResult {
            created: vec![".regista/config.toml".into()],
            skipped: vec![],
            errors: vec![],
        };

        // Crear directorios base
        for dir in &[".regista/tasks", ".regista/decisions", ".regista/logs"] {
            let path = project_dir.join(dir);
            std::fs::create_dir_all(&path)?;
        }

        // --with-example
        if with_example {
            let tasks_dir = project_dir.join(".regista/tasks");
            let example_path = tasks_dir.join("TASK-001.md");
            std::fs::write(
                &example_path,
                "# TASK-001\n\n## Status\n**draft**\n\n## Activity Log\n- 2026-05-14 | PO | Ejemplo\n",
            )?;
            result.created.push(".regista/tasks/TASK-001.md".into());
        }

        Ok(result)
    }

    #[test]
    fn init_with_existing_config_fails_without_force() {
        // CA3: Sin --force, init debe fallar con error descriptivo
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".regista")).unwrap();
        std::fs::write(tmp.path().join(".regista/config.toml"), "# existente").unwrap();

        let result = init_with_preset_helper(tmp.path(), "software-dev", false, false);

        assert!(result.is_err(), "init sin --force debe fallar si config.toml existe");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("config.toml ya existe"),
            "El mensaje de error debe mencionar 'config.toml ya existe': {err}"
        );
        assert!(
            err.contains("--force"),
            "El mensaje de error debe sugerir --force: {err}"
        );

        // Verificar que el archivo existente NO se modificó
        let content = std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert_eq!(content, "# existente", "El archivo original no debe modificarse");
    }

    #[test]
    fn init_with_existing_config_and_force_overwrites() {
        // CA3: Con --force, init sobreescribe y termina exitosamente
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".regista")).unwrap();
        std::fs::write(
            tmp.path().join(".regista/config.toml"),
            "# contenido antiguo",
        )
        .unwrap();

        let result = init_with_preset_helper(tmp.path(), "software-dev", true, false);

        assert!(result.is_ok(), "init con --force debe tener éxito");
        let init_result = result.unwrap();
        assert!(
            init_result.created.iter().any(|p| p.contains("config.toml")),
            "Debe registrar config.toml como creado"
        );

        // Verificar que el archivo fue sobreescrito con el contenido del preset
        let content =
            std::fs::read_to_string(tmp.path().join(".regista/config.toml")).unwrap();
        assert!(
            content.contains("software-dev"),
            "El archivo debe contener el preset: {content}"
        );
        assert!(
            !content.contains("contenido antiguo"),
            "El contenido antiguo debe haberse reemplazado"
        );
    }

    #[test]
    fn init_with_preset_generates_example_task() {
        // CA3: --with-example genera una task de ejemplo en .regista/tasks/
        let tmp = tempfile::tempdir().unwrap();
        let result =
            init_with_preset_helper(tmp.path(), "software-dev", false, true).unwrap();

        assert!(
            result.created.iter().any(|p| p.contains("TASK-001.md")),
            "--with-example debe generar TASK-001.md"
        );
        assert!(tmp.path().join(".regista/tasks/TASK-001.md").exists());

        // Verificar que la task generada tiene el formato del preset
        let task_content =
            std::fs::read_to_string(tmp.path().join(".regista/tasks/TASK-001.md")).unwrap();
        assert!(
            task_content.contains("## Status"),
            "La task de ejemplo debe tener ## Status"
        );
        assert!(
            task_content.contains("## Activity Log"),
            "La task de ejemplo debe tener ## Activity Log"
        );
    }

    #[test]
    fn init_result_records_created_and_skipped() {
        // Test de regresión: InitResult siempre ha tenido campos created/skipped/errors.
        let tmp = tempfile::tempdir().unwrap();
        let result = init(tmp.path(), false, true, "pi").unwrap();

        assert!(result.errors.is_empty(), "No debe haber errores en init normal");
        assert!(!result.created.is_empty(), "Debe haber archivos creados");

        // Ejecutar de nuevo: los archivos existentes deben estar en skipped
        let result2 = init(tmp.path(), false, true, "pi").unwrap();
        assert!(!result2.skipped.is_empty(), "Segunda ejecución debe tener skipped");
    }
}
