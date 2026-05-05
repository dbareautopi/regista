//! Generación automática de historias desde un documento de requisitos
//! (`regista plan`).
//!
//! El Product Owner lee una spec de alto nivel, la descompone en historias
//! atómicas con criterios de aceptación, las agrupa en épicas, y las escribe
//! en el directorio de historias. Un bucle de validación asegura que el grafo
//! de dependencias resultante es correcto antes de dar el backlog por bueno.

use crate::app::validate;
use crate::config::Config;
use crate::infra::agent::{self, AgentOptions};
use crate::infra::providers;
use std::path::Path;

/// Resultado de la operación de plan.
#[derive(Debug, Clone)]
pub struct PlanResult {
    /// Número de historias generadas.
    pub stories_created: usize,
    /// Número de épicas generadas.
    pub epics_created: usize,
    /// Número de iteraciones del bucle plan→validate.
    pub iterations: u32,
    /// Si terminó con el grafo de dependencias limpio.
    pub dependencies_clean: bool,
}

/// Ejecuta el plan completo: generar historias desde una spec y validar
/// dependencias en bucle hasta que estén limpias.
pub fn run(
    project_root: &Path,
    spec_path: &Path,
    cfg: &Config,
    max_stories: u32,
    replace: bool,
) -> anyhow::Result<PlanResult> {
    // ── 1. Validar que la spec existe ──────────────────────────────
    if !spec_path.exists() {
        anyhow::bail!(
            "El archivo de especificación no existe: {}",
            spec_path.display()
        );
    }
    if !spec_path.is_file() {
        anyhow::bail!(
            "La ruta de especificación no es un archivo: {}",
            spec_path.display()
        );
    }

    let stories_dir = project_root.join(&cfg.project.stories_dir);
    let epics_dir = project_root.join(&cfg.project.epics_dir);
    let decisions_dir = project_root.join(&cfg.project.decisions_dir);

    // ── 2. Si --replace, limpiar directorios ───────────────────────
    if replace {
        if stories_dir.exists() {
            tracing::info!("🧹 Limpiando {} ...", stories_dir.display());
            std::fs::remove_dir_all(&stories_dir)?;
        }
        if epics_dir.exists() {
            tracing::info!("🧹 Limpiando {} ...", epics_dir.display());
            std::fs::remove_dir_all(&epics_dir)?;
        }
    }

    // Crear directorios si no existen
    std::fs::create_dir_all(&stories_dir)?;
    std::fs::create_dir_all(&epics_dir)?;
    std::fs::create_dir_all(&decisions_dir)?;

    // ── 3. Snapshot git inicial ────────────────────────────────────
    let snapshot_hash = if cfg.git.enabled {
        crate::infra::git::snapshot(project_root, "plan-start")
    } else {
        None
    };

    // ── 4. Bucle plan → validate ───────────────────────────────────
    let provider_name = cfg.agents.provider_for_role("product_owner");
    let provider = providers::from_name(&provider_name)?;
    let skill_path_str = cfg.agents.skill_for_role("product_owner");
    let skill_path = project_root.join(&skill_path_str);
    let max_loop = cfg.limits.plan_max_iterations.max(1);

    let spec_content = std::fs::read_to_string(spec_path)?;

    let ctx = PlanCtx {
        spec_path,
        spec_content: &spec_content,
        stories_dir: &stories_dir,
        epics_dir: &epics_dir,
        decisions_dir: &decisions_dir,
        story_pattern: &cfg.project.story_pattern,
        max_stories,
    };

    let mut loop_iteration: u32 = 0;
    let mut stories_count: usize = 0;
    let mut epics_count: usize = 0;
    let mut deps_clean = false;

    loop {
        loop_iteration += 1;

        let prompt = if loop_iteration == 1 {
            plan_prompt_initial(&ctx)
        } else {
            // Validar y obtener errores de dependencias
            let config_path = project_root.join(".regista/config.toml");
            let config_path_opt = if config_path.exists() {
                Some(config_path.as_path())
            } else {
                None
            };
            let validation = validate::validate(project_root, config_path_opt);
            let dep_errors: Vec<String> = validation
                .findings
                .iter()
                .filter(|f| f.severity == validate::Severity::Error && f.category == "dependencies")
                .map(|f| f.message.clone())
                .collect();

            if dep_errors.is_empty() {
                tracing::info!(
                    "✅ Grafo de dependencias correcto tras {loop_iteration} iteraciones."
                );
                deps_clean = true;
                break;
            }

            if loop_iteration > max_loop {
                tracing::warn!(
                    "⚠️  Máximo de {max_loop} iteraciones alcanzado. El grafo aún tiene errores."
                );
                for err in &dep_errors {
                    tracing::warn!("  • {err}");
                }
                break;
            }

            tracing::info!(
                "🔁 Iteración {loop_iteration}/{max_loop}: corrigiendo {} errores...",
                dep_errors.len()
            );

            plan_prompt_fix(&ctx, &dep_errors)
        };

        tracing::info!("🤖 Invocando PO para generar/corregir historias...");

        match agent::invoke_with_retry_blocking(
            provider.as_ref(),
            &skill_path,
            &prompt,
            &cfg.limits,
            &AgentOptions::default(),
        ) {
            Ok(_) => {
                stories_count = count_files(&stories_dir, &cfg.project.story_pattern);
                epics_count = count_files(&epics_dir, "EPIC-*.md");
                tracing::info!("  📊 Generadas: {stories_count} historias, {epics_count} épicas");

                if stories_count == 0 && loop_iteration == 1 {
                    tracing::warn!(
                        "⚠️  El PO no generó ninguna historia. ¿El skill tiene permisos de escritura?"
                    );
                }
            }
            Err(e) => {
                tracing::error!("❌ Falló la invocación del PO: {e}");
                if let Some(ref hash) = snapshot_hash {
                    crate::infra::git::rollback(project_root, hash, "plan-failed");
                }
                anyhow::bail!("Plan falló: {e}");
            }
        }
    }

    Ok(PlanResult {
        stories_created: stories_count,
        epics_created: epics_count,
        iterations: loop_iteration,
        dependencies_clean: deps_clean,
    })
}

// ── Prompts ─────────────────────────────────────────────────────────────

/// Contexto común para los prompts de plan.
struct PlanCtx<'a> {
    spec_path: &'a Path,
    spec_content: &'a str,
    stories_dir: &'a Path,
    epics_dir: &'a Path,
    decisions_dir: &'a Path,
    story_pattern: &'a str,
    max_stories: u32,
}

/// Prompt para la primera generación de historias.
fn plan_prompt_initial(ctx: &PlanCtx) -> String {
    let limit_line = if ctx.max_stories > 0 {
        format!(
            "\nGenera como **máximo {} historias** en total.\n",
            ctx.max_stories
        )
    } else {
        String::new()
    };

    format!(
        "Eres un Product Owner. Tu tarea es descomponer una especificación \
         de producto en historias de usuario atómicas y épicas.\n\
         \n\
         ## Especificación fuente\n\
         Archivo: {spec_path}\n\
         \n\
         ```\n\
         {spec_content}\n\
         ```\n\
         \n\
         ## Instrucciones\n\
         \n\
         1. Lee la especificación completa.\n\
         2. Identifica **épicas** (grupos de funcionalidades relacionadas).\n\
         3. Para cada épica, descompón en **historias de usuario atómicas**.\n\
         4. Cada historia debe ser pequeña, independiente, y entregar valor.\n\
         {limit_line}\
         \n\
         ## Formato de cada historia (archivo STORY-NNN.md en {stories_dir})\n\
         \n\
         ```markdown\n\
         # STORY-NNN: Título descriptivo\n\
         \n\
         ## Status\n\
         **Draft**\n\
         \n\
         ## Epic\n\
         EPIC-XXX\n\
         \n\
         ## Descripción\n\
         [Descripción clara de la funcionalidad. No ambigua.]\n\
         \n\
         ## Criterios de aceptación\n\
         - [ ] CA1: criterio específico y verificable\n\
         - [ ] CA2: ...\n\
         \n\
         ## Dependencias\n\
         - Bloqueado por: STORY-XXX, STORY-YYY\n\
         \n\
         ## Activity Log\n\
         - [FECHA] | PO | Historia generada desde {spec_path}.\n\
         ```\n\
         \n\
         ## Formato de cada épica (archivo EPIC-NNN.md en {epics_dir})\n\
         \n\
         ```markdown\n\
         # EPIC-NNN: Título de la épica\n\
         \n\
         ## Descripción\n\
         [Descripción de la épica]\n\
         \n\
         ## Historias\n\
         - STORY-XXX\n\
         - STORY-YYY\n\
         ```\n\
         \n\
         ## Reglas importantes\n\
         \n\
         - Los IDs de historia deben seguir el patrón {story_pattern}.\n\
         - Los criterios de aceptación deben ser **específicos y testeables**.\n\
           Nada de \"debe funcionar bien\". Sé concreto.\n\
         - Si una historia depende de otra, indícalo en \"Bloqueado por:\".\n\
           Solo referenciar historias que TÚ has creado en esta sesión.\n\
         - Cada historia comienza en estado **Draft**.\n\
         - El Activity Log debe tener una entrada inicial con la fecha de hoy.\n\
         - Documenta las decisiones de diseño del backlog en {decisions_dir}/plan-decision.md.\n\
         - Escribe los archivos reales en el filesystem. No los imprimas en pantalla.\n\
         - **NO preguntes nada al usuario. Trabaja de forma 100% autónoma.**\n\
         \n\
         Empieza ya. Lee la spec, descompón, y escribe los archivos.",
        spec_path = ctx.spec_path.display(),
        spec_content = ctx.spec_content,
        stories_dir = ctx.stories_dir.display(),
        epics_dir = ctx.epics_dir.display(),
        decisions_dir = ctx.decisions_dir.display(),
        story_pattern = ctx.story_pattern,
        limit_line = limit_line,
    )
}

/// Prompt para corregir historias tras fallos de validación de dependencias.
fn plan_prompt_fix(ctx: &PlanCtx, errors: &[String]) -> String {
    let limit_line = if ctx.max_stories > 0 {
        format!(
            "\nNo generes más de {} historias en total.\n",
            ctx.max_stories
        )
    } else {
        String::new()
    };

    let errors_formatted: String = errors
        .iter()
        .enumerate()
        .map(|(i, e)| format!("{}. {e}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Eres un Product Owner. Las historias que generaste desde la especificación \
         tienen **errores en el grafo de dependencias**. Debes corregirlos.\n\
         \n\
         ## Especificación original\n\
         Archivo: {spec_path}\n\
         \n\
         ```\n\
         {spec_content}\n\
         ```\n\
         \n\
         ## Errores de dependencias detectados\n\
         \n\
         {errors_formatted}\n\
         \n\
         ## Lo que debes hacer\n\
         \n\
         1. Lee los archivos de historia en {stories_dir}.\n\
         2. Corrige **solo los archivos que tengan errores** de dependencias:\n\
            - Si una historia referencia un ID que no existe, elimina o corrige la referencia.\n\
            - Si hay un ciclo de dependencias, rompe el ciclo eliminando la dependencia menos crítica.\n\
            - NO borres historias completas a menos que sean redundantes.\n\
         3. Actualiza el Activity Log de cada historia modificada:\n\
            `- [FECHA] | PO | Corregidas dependencias tras validación.`\n\
         4. Si eliminaste dependencias, asegúrate de que las épicas en {epics_dir} sigan siendo correctas.\n\
         {limit_line}\
         \n\
         ## Reglas\n\
         - Solo modifica archivos existentes. No crees nuevas historias a menos que sea inevitable.\n\
         - Los IDs deben seguir el patrón {story_pattern}.\n\
         - Documenta los cambios en {decisions_dir}/plan-correcciones.md.\n\
         - **NO preguntes nada al usuario. 100% autónomo.**\n\
         \n\
         Corrige los errores ahora.",
        spec_path = ctx.spec_path.display(),
        spec_content = ctx.spec_content,
        stories_dir = ctx.stories_dir.display(),
        epics_dir = ctx.epics_dir.display(),
        decisions_dir = ctx.decisions_dir.display(),
        story_pattern = ctx.story_pattern,
        errors_formatted = errors_formatted,
        limit_line = limit_line,
    )
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Cuenta archivos que coinciden con un patrón glob en un directorio.
fn count_files(dir: &Path, pattern: &str) -> usize {
    let full_pattern = dir.join(pattern);
    match glob::glob(full_pattern.to_str().unwrap_or("*.md")) {
        Ok(entries) => entries.filter_map(|e| e.ok()).count(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_fixture() -> PlanCtx<'static> {
        // Safety: all &str and Path references are to statics
        let spec_content: &'static str = "contenido de prueba";
        let spec_path = Path::new("spec.md");
        let stories_dir = Path::new("stories");
        let epics_dir = Path::new("epics");
        let decisions_dir = Path::new("decisions");
        let story_pattern = "STORY-*.md";
        PlanCtx {
            spec_path,
            spec_content,
            stories_dir,
            epics_dir,
            decisions_dir,
            story_pattern,
            max_stories: 0,
        }
    }

    #[test]
    fn count_files_counts_md_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("STORY-001.md"), "test").unwrap();
        std::fs::write(tmp.path().join("STORY-002.md"), "test").unwrap();
        std::fs::write(tmp.path().join("NOTES.txt"), "test").unwrap();
        assert_eq!(count_files(tmp.path(), "STORY-*.md"), 2);
    }

    #[test]
    fn count_files_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(count_files(tmp.path(), "STORY-*.md"), 0);
    }

    #[test]
    fn plan_prompt_initial_contains_spec() {
        let prompt = plan_prompt_initial(&ctx_fixture());
        assert!(prompt.contains("contenido de prueba"));
        assert!(prompt.contains("stories"));
        assert!(prompt.contains("NO preguntes"));
    }

    #[test]
    fn plan_prompt_initial_respects_max_stories() {
        let mut ctx = ctx_fixture();
        ctx.max_stories = 10;
        let prompt = plan_prompt_initial(&ctx);
        assert!(prompt.contains("máximo 10 historias"));
    }

    #[test]
    fn plan_prompt_initial_no_limit_when_zero() {
        let prompt = plan_prompt_initial(&ctx_fixture());
        assert!(!prompt.contains("máximo"));
    }

    #[test]
    fn plan_prompt_fix_includes_errors() {
        let errors = vec![
            "STORY-003: referencia a STORY-999 que no existe".to_string(),
            "Ciclo entre STORY-005 y STORY-007".to_string(),
        ];
        let prompt = plan_prompt_fix(&ctx_fixture(), &errors);
        assert!(prompt.contains("STORY-003"));
        assert!(prompt.contains("STORY-999"));
        assert!(prompt.contains("Ciclo"));
        assert!(prompt.contains("NO preguntes"));
    }
}
