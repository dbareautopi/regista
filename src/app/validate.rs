//! Validador de integridad del proyecto (`regista validate`).
//!
//! Verifica configuración, historias, skills, dependencias y git
//! sin ejecutar agentes. Ideal como paso previo en CI/CD.

use crate::config::{AgentsConfig, Config};
use crate::domain::graph::DependencyGraph;
use crate::domain::state::Status;
use crate::domain::story::Story;
use crate::infra::providers;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// Severidad de un hallazgo de validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warning")]
    Warning,
}

/// Un hallazgo individual de validación.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_id: Option<String>,
}

/// Resultado global de la validación.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub ok: usize,
    pub warnings: usize,
    pub errors: usize,
    pub findings: Vec<Finding>,
}

impl ValidationResult {
    /// Añade un hallazgo y actualiza contadores.
    fn add(
        &mut self,
        severity: Severity,
        category: &str,
        message: String,
        story_id: Option<String>,
    ) {
        match severity {
            Severity::Error => self.errors += 1,
            Severity::Warning => self.warnings += 1,
        }
        self.findings.push(Finding {
            severity,
            category: category.to_string(),
            message,
            story_id,
        });
    }
}

/// Ejecuta todas las validaciones sobre un proyecto.
pub fn validate(project_root: &Path, config_path: Option<&Path>) -> ValidationResult {
    let mut result = ValidationResult {
        ok: 0,
        warnings: 0,
        errors: 0,
        findings: vec![],
    };

    // ── 1. Config ───────────────────────────────────────────────────
    let cfg = validate_config(project_root, config_path, &mut result);

    // ── 2. Skills ───────────────────────────────────────────────────
    if let Some(ref cfg) = cfg {
        validate_skills(project_root, cfg, &mut result);
    }

    // ── 3. Historias ────────────────────────────────────────────────
    let stories = if let Some(ref cfg) = cfg {
        validate_stories(project_root, cfg, &mut result)
    } else {
        vec![]
    };

    // ── 4. Dependencias ─────────────────────────────────────────────
    if !stories.is_empty() {
        validate_dependencies(&stories, &mut result);
    }

    // ── 5. Git ──────────────────────────────────────────────────────
    if let Some(ref cfg) = cfg {
        validate_git(project_root, cfg, &mut result);
    }

    // Contar OKs: cada categoría sin hallazgos cuenta como OK
    let categories: HashSet<&str> = result
        .findings
        .iter()
        .map(|f| f.category.as_str())
        .collect();
    let all_categories = ["config", "skills", "stories", "dependencies", "git"];
    result.ok = all_categories
        .iter()
        .filter(|c| !categories.contains(*c))
        .count();

    result
}

// ── Validaciones individuales ──────────────────────────────────────────

fn validate_config(
    project_root: &Path,
    config_path: Option<&Path>,
    result: &mut ValidationResult,
) -> Option<Config> {
    let default_config_path = project_root.join(".regista/config.toml");
    let config_path = config_path.unwrap_or(&default_config_path);

    if !config_path.exists() {
        result.add(
            Severity::Warning,
            "config",
            format!(
                "Archivo {} no encontrado — se usarán defaults.",
                config_path.display()
            ),
            None,
        );
        // Usar defaults
        return Some(Config::default());
    }

    match std::fs::read_to_string(config_path) {
        Ok(content) => match toml::from_str::<Config>(&content) {
            Ok(cfg) => {
                // Verificar que stories_dir existe
                let stories_path = project_root.join(&cfg.project.stories_dir);
                if !stories_path.exists() {
                    result.add(
                        Severity::Error,
                        "config",
                        format!(
                            "El directorio de historias '{}' no existe.",
                            stories_path.display()
                        ),
                        None,
                    );
                }
                Some(cfg)
            }
            Err(e) => {
                result.add(
                    Severity::Error,
                    "config",
                    format!("Error parseando {}: {e}", config_path.display()),
                    None,
                );
                None
            }
        },
        Err(e) => {
            result.add(
                Severity::Error,
                "config",
                format!("No se pudo leer {}: {e}", config_path.display()),
                None,
            );
            None
        }
    }
}

fn validate_skills(project_root: &Path, cfg: &Config, result: &mut ValidationResult) {
    let roles = AgentsConfig::all_roles();
    let role_names = ["PO", "QA", "Dev", "Reviewer"];

    let mut found = 0;
    for (i, role) in roles.iter().enumerate() {
        let path_str = providers::skill_for_role(&cfg.agents, role);
        let path = project_root.join(&path_str);
        let label = role_names[i];
        if path.exists() && path.is_file() {
            found += 1;
        } else {
            result.add(
                Severity::Error,
                "skills",
                format!("Skill de {label} no encontrado: {}", path.display()),
                None,
            );
        }
    }

    if found == roles.len() {
        // All good - counted in final ok
    }
}

fn validate_stories(
    project_root: &Path,
    cfg: &Config,
    result: &mut ValidationResult,
) -> Vec<Story> {
    let stories_dir = project_root.join(&cfg.project.stories_dir);

    if !stories_dir.exists() || !stories_dir.is_dir() {
        result.add(
            Severity::Error,
            "stories",
            format!(
                "Directorio de historias no accesible: {}",
                stories_dir.display()
            ),
            None,
        );
        return vec![];
    }

    let pattern = stories_dir.join(&cfg.project.story_pattern);
    let mut stories = vec![];

    let entries = match glob::glob(pattern.to_str().unwrap_or("*.md")) {
        Ok(e) => e,
        Err(e) => {
            result.add(
                Severity::Error,
                "stories",
                format!("Patrón glob inválido '{}': {e}", cfg.project.story_pattern),
                None,
            );
            return vec![];
        }
    };

    for entry in entries {
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                result.add(
                    Severity::Warning,
                    "stories",
                    format!("Error leyendo entrada: {e}"),
                    None,
                );
                continue;
            }
        };

        match Story::load(&path) {
            Ok(story) => {
                // Validar ID: STORY-NNN
                if !story.id.chars().any(|c| c.is_ascii_digit()) {
                    result.add(
                        Severity::Warning,
                        "stories",
                        format!("{}: ID no contiene número ({})", story.id, path.display()),
                        Some(story.id.clone()),
                    );
                }

                // Verificar que tiene Activity Log
                let has_activity_log = story
                    .raw_content
                    .lines()
                    .any(|l| l.to_lowercase().trim().starts_with("## activity log"));
                if !has_activity_log {
                    result.add(
                        Severity::Warning,
                        "stories",
                        format!("{}: no tiene sección '## Activity Log'", story.id),
                        Some(story.id.clone()),
                    );
                }

                // Verificar que el status no es None/unknown
                if story.status == Status::Draft && story.raw_content.is_empty() {
                    // This shouldn't happen since load() fails on unknown status
                }

                stories.push(story);
            }
            Err(e) => {
                let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                result.add(
                    Severity::Error,
                    "stories",
                    format!("{id}: error al parsear — {e}"),
                    Some(id.to_string()),
                );
            }
        }
    }

    if stories.is_empty() {
        result.add(
            Severity::Warning,
            "stories",
            format!("No se encontraron historias en {}", stories_dir.display()),
            None,
        );
    }

    stories
}

fn validate_dependencies(stories: &[Story], result: &mut ValidationResult) {
    let story_ids: HashSet<&str> = stories.iter().map(|s| s.id.as_str()).collect();

    // Verificar referencias a historias inexistentes
    for story in stories {
        for blocker in &story.blockers {
            if !story_ids.contains(blocker.as_str()) {
                result.add(
                    Severity::Error,
                    "dependencies",
                    format!(
                        "{}: referencia a {} que no existe en {}",
                        story.id,
                        blocker,
                        stories
                            .first()
                            .map(|s| s
                                .path
                                .parent()
                                .unwrap_or(Path::new("."))
                                .display()
                                .to_string())
                            .unwrap_or_default()
                    ),
                    Some(story.id.clone()),
                );
            }
        }
    }

    // Verificar ciclos
    let graph = DependencyGraph::from_stories(stories);
    if graph.has_any_cycle() {
        let cycle_members = graph.find_cycle_members();
        let members_str: Vec<String> = {
            let mut v: Vec<String> = cycle_members.iter().cloned().collect();
            v.sort();
            v
        };
        result.add(
            Severity::Error,
            "dependencies",
            format!(
                "Ciclo de dependencias detectado entre: {}",
                members_str.join(", ")
            ),
            None,
        );
    }
}

fn validate_git(project_root: &Path, cfg: &Config, result: &mut ValidationResult) {
    if !cfg.git.enabled {
        return;
    }

    if !project_root.join(".git").is_dir() {
        result.add(
            Severity::Warning,
            "git",
            "git.enabled = true pero no hay repositorio git. Se auto-inicializará.".into(),
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::state::Status;
    use std::path::PathBuf;

    fn story_fixture(id: &str, status: Status, blockers: &[&str]) -> Story {
        Story {
            id: id.to_string(),
            path: PathBuf::from(format!("stories/{id}.md")),
            status,
            epic: None,
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            last_rejection: None,
            raw_content: format!(
                "# {id}\n\n## Status\n**{status}**\n\n## Activity Log\n- 2026-04-30 | PO | ok\n"
            ),
        }
    }

    #[test]
    fn validate_no_dependency_issues() {
        let stories = vec![
            story_fixture("STORY-001", Status::Done, &[]),
            story_fixture("STORY-002", Status::Ready, &["STORY-001"]),
        ];
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_dependencies(&stories, &mut result);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn validate_missing_dependency_detected() {
        let stories = vec![story_fixture("STORY-001", Status::Blocked, &["STORY-999"])];
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_dependencies(&stories, &mut result);
        assert!(result.errors > 0);
    }

    #[test]
    fn validate_cycle_detected() {
        let stories = vec![
            story_fixture("STORY-001", Status::Blocked, &["STORY-002"]),
            story_fixture("STORY-002", Status::Blocked, &["STORY-001"]),
        ];
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_dependencies(&stories, &mut result);
        assert!(result.errors > 0);
        assert!(result.findings.iter().any(|f| f.message.contains("Ciclo")));
    }
}
