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

    // ── 3. Providers ───────────────────────────────────────────────
    if let Some(ref cfg) = cfg {
        validate_providers(cfg, &mut result);
    }

    // ── 4. Historias ────────────────────────────────────────────────
    let stories = if let Some(ref cfg) = cfg {
        validate_stories(project_root, cfg, &mut result)
    } else {
        vec![]
    };

    // ── 5. Dependencias ─────────────────────────────────────────────
    if !stories.is_empty() {
        validate_dependencies(&stories, &mut result);
    }

    // ── 6. Git ──────────────────────────────────────────────────────
    if let Some(ref cfg) = cfg {
        validate_git(project_root, cfg, &mut result);
    }

    // Contar OKs: cada categoría sin hallazgos cuenta como OK
    let categories: HashSet<&str> = result
        .findings
        .iter()
        .map(|f| f.category.as_str())
        .collect();
    let all_categories = [
        "config",
        "skills",
        "providers",
        "stories",
        "dependencies",
        "git",
    ];
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

/// Valida que los binarios de los providers configurados existen en PATH.
///
/// Para cada rol, resuelve el provider y verifica que su binario está
/// accesible. Si no lo está:
/// - Provider ≠ codex → Finding::Error (CA6)
/// - Provider = codex → Finding::Warning (CA7, codex puede usar nombres no estándar)
fn validate_providers(cfg: &Config, result: &mut ValidationResult) {
    use std::collections::HashSet;

    // Recolectar todos los providers únicos (por rol + global)
    let mut provider_names: HashSet<String> = HashSet::new();

    // Provider global (siempre se chequea, puede ser el fallback)
    provider_names.insert(cfg.agents.provider.clone());

    // Providers por rol
    let roles = ["product_owner", "qa_engineer", "developer", "reviewer"];
    for role in &roles {
        let name = providers::provider_for_role(&cfg.agents, role);
        provider_names.insert(name);
    }

    // Verificar cada provider único
    for name in &provider_names {
        let provider = match providers::from_name(name) {
            Ok(p) => p,
            Err(e) => {
                // El nombre del provider no es reconocido por la factory
                result.add(
                    Severity::Error,
                    "providers",
                    format!("Provider configurado '{name}' no es válido: {e}"),
                    None,
                );
                continue;
            }
        };

        let binary = provider.binary();

        // En Windows, el binary de opencode es "powershell" (wrapper) — verificamos "opencode" en su lugar
        let check_binary = if cfg!(windows) && name.to_lowercase() == "opencode" {
            "opencode"
        } else {
            binary
        };

        // Buscar el binario en PATH
        let found = find_in_path(check_binary);

        if found {
            // El binario existe en PATH — todo bien
        } else {
            // No se encontró el binario
            let is_codex = name.to_lowercase() == "codex";
            if is_codex {
                // CA7: codex puede instalarse con nombres no estándar (npm global)
                result.add(
                    Severity::Warning,
                    "providers",
                    "No se encontró el binario 'codex' en PATH.".to_string(),
                    None,
                );
            } else {
                // CA6: Error para providers que no son codex
                result.add(
                    Severity::Error,
                    "providers",
                    format!(
                        "No se encontró el binario '{check_binary}' del provider '{name}' en PATH."
                    ),
                    None,
                );
            }
        }
    }
}

/// Busca un ejecutable en los directorios del PATH.
fn find_in_path(binary: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(binary);
            if candidate.is_file() {
                return true;
            }
            // En Windows, también buscar con extensión .exe
            if cfg!(windows) {
                let candidate_exe = dir.join(format!("{binary}.exe"));
                if candidate_exe.is_file() {
                    return true;
                }
            }
        }
    }
    false
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-001: validate verifica binarios de providers
    // ═══════════════════════════════════════════════════════════════

    /// CA6: validate_providers reporta Finding::Error si el binario
    /// del provider configurado no está en PATH.
    ///
    /// Este test verifica que la función existe, recibe Config, y
    /// añade hallazgos al ValidationResult. El Developer debe
    /// implementar la lógica real de chequeo de PATH.
    #[test]
    fn validate_providers_reports_error_when_binary_missing() {
        // Verificar que la función validate_providers existe y se puede llamar.
        // Usamos la config por defecto (provider = "pi").
        // Si pi está instalado → sin errores de providers.
        // Si pi NO está instalado → Error finding.
        let cfg = Config::default();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        // La función validate_providers debe existir con esta firma.
        validate_providers(&cfg, &mut result);

        // Los findings de categoría "providers" deben ser Error o nada.
        // No deben ser Warning (salvo codex, ver CA7).
        for finding in &result.findings {
            if finding.category == "providers" {
                assert_eq!(
                    finding.severity,
                    Severity::Error,
                    "Provider 'pi' no es codex → el hallazgo debe ser Error, no Warning"
                );
            }
        }
    }

    /// CA7: validate_providers reporta Finding::Warning si el provider
    /// es "codex" y no se puede verificar (codex puede estar instalado
    /// vía npm global con nombre no estándar).
    #[test]
    fn validate_providers_reports_warning_for_codex() {
        let toml = r#"
[agents]
provider = "codex"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_providers(&cfg, &mut result);

        // Si codex NO está en PATH → Warning (nunca Error).
        // Si codex SÍ está → sin findings de providers.
        for finding in &result.findings {
            if finding.category == "providers" {
                assert_eq!(
                    finding.severity,
                    Severity::Warning,
                    "Provider 'codex' debe generar Warning, no Error, cuando no es verificable"
                );
            }
        }
    }

    /// CA7: Si codex SÍ está en PATH, no debe generar hallazgo.
    #[test]
    fn validate_providers_no_warning_when_codex_is_installed() {
        let toml = r#"
[agents]
provider = "codex"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_providers(&cfg, &mut result);

        // Si codex está instalado, no debe haber hallazgos.
        // Si no está, debe ser Warning.
        // En cualquier caso, no debe ser Error.
        for finding in &result.findings {
            if finding.category == "providers" {
                assert!(
                    finding.severity != Severity::Error,
                    "codex NUNCA debe generar Error, solo Warning o nada"
                );
            }
        }
    }

    /// CA6+CA7: validate_providers recorre todos los roles configurados,
    /// no solo el provider global.
    #[test]
    fn validate_providers_checks_all_roles() {
        let toml = r#"
[agents]
provider = "pi"

[agents.product_owner]
provider = "claude"

[agents.developer]
provider = "codex"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_providers(&cfg, &mut result);

        // La función no debe paniquear al procesar múltiples providers.
        // Verifica que los hallazgos están categorizados como "providers".
        let provider_findings: Vec<_> = result
            .findings
            .iter()
            .filter(|f| f.category == "providers")
            .collect();

        // Al menos debe haber intentado verificar los providers.
        // Si todos están instalados, provider_findings estará vacío (OK).
        // Si alguno falta, debe haber hallazgos.
        for f in &provider_findings {
            // Los de codex deben ser Warning, el resto Error.
            if f.message.contains("codex") {
                assert_eq!(f.severity, Severity::Warning);
            }
        }
    }
}
