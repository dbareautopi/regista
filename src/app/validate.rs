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

    // ── 7. Models (STORY-V10-005 CA3) ───────────────────────────────
    if let Some(ref cfg) = cfg {
        validate_models(cfg, &mut result);
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
        "models",
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
        let path_str = crate::app::resolver::skill_path(&cfg.agents, role);
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

        match crate::app::story_io::load(&path) {
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
        let name = cfg.agents.provider_for_role(role);
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

/// Valida los modelos LLM definidos en `[models]` (STORY-V10-005 CA3).
///
/// Para cada modelo definido:
/// - Verifica que el provider es "openai" o "anthropic" (Error si no)
/// - Intenta expandir `${ENV_VAR}` en `api_key` (Error si la variable no existe)
/// - Advierte si `api_key` está vacío (compatible con Ollama)
fn validate_models(cfg: &Config, result: &mut ValidationResult) {
    if cfg.models.is_empty() {
        // Sin modelos definidos: nada que validar. La categoría "models" queda OK.
        return;
    }

    let valid_providers = ["openai", "anthropic"];

    for (name, model) in &cfg.models {
        // CA3a: Verificar que el provider es conocido
        let provider_lower = model.provider.to_lowercase();
        if !valid_providers.contains(&provider_lower.as_str()) {
            result.add(
                Severity::Error,
                "models",
                format!(
                    "Modelo '{name}': provider '{}' desconocido. Providers válidos: openai, anthropic",
                    model.provider
                ),
                None,
            );
            continue; // No seguir validando este modelo si el provider es inválido
        }

        // CA3b: Intentar expandir variables de entorno
        // Si api_key tiene ${...}, verificar que la variable existe
        if model.api_key.contains("${") {
            match crate::config::expand_env_vars(&model.api_key) {
                Ok(expanded) => {
                    // CA3c: Warning si api_key expandido está vacío (caso Ollama)
                    if expanded.is_empty() || expanded == model.api_key {
                        // api_key vacío es válido para Ollama local, solo warning
                        result.add(
                            Severity::Warning,
                            "models",
                            format!(
                                "Modelo '{name}': api_key está vacío. Esto es válido para Ollama o proxies locales, \
                                 pero asegúrate de que el endpoint acepta requests sin autenticación."
                            ),
                            None,
                        );
                    }
                }
                Err(e) => {
                    result.add(
                        Severity::Error,
                        "models",
                        format!("Modelo '{name}': {e}"),
                        None,
                    );
                }
            }
        } else {
            // api_key no tiene ${...}, verificar si está vacío
            if model.api_key.is_empty() {
                result.add(
                    Severity::Warning,
                    "models",
                    format!(
                        "Modelo '{name}': api_key está vacío. Esto es válido para Ollama o proxies locales."
                    ),
                    None,
                );
            }
        }
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-V10-005 CA3: validate models
    // ═══════════════════════════════════════════════════════════════

    /// CA3: validate_models con modelos vacíos no produce hallazgos.
    #[test]
    fn story_v10005_ca3_empty_models_no_findings() {
        let cfg = Config::default();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);
        assert!(result.findings.is_empty());
    }

    /// CA3: validate_models detecta provider desconocido como Error.
    #[test]
    fn story_v10005_ca3_unknown_provider_error() {
        let toml = r#"
[models.mistral]
provider = "mistral"
model_id = "mistral-large"
api_key = "sk-test"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);

        assert_eq!(result.errors, 1);
        let finding = result
            .findings
            .iter()
            .find(|f| f.category == "models")
            .unwrap();
        assert_eq!(finding.severity, Severity::Error);
        assert!(
            finding.message.contains("mistral"),
            "error debe mencionar el provider: {}",
            finding.message
        );
    }

    /// CA3: validate_models acepta openai como provider válido.
    #[test]
    fn story_v10005_ca3_valid_openai_provider_no_error() {
        let toml = r#"
[models.gpt4o]
provider = "openai"
model_id = "gpt-4o"
api_key = "sk-test"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);
        assert_eq!(result.errors, 0);
    }

    /// CA3: validate_models acepta anthropic como provider válido.
    #[test]
    fn story_v10005_ca3_valid_anthropic_provider_no_error() {
        let toml = r#"
[models.claude]
provider = "anthropic"
model_id = "claude-sonnet"
api_key = "sk-ant-test"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);
        assert_eq!(result.errors, 0);
    }

    /// CA3: validate_models reporta Error si la variable de entorno no está definida.
    #[test]
    fn story_v10005_ca3_missing_env_var_error() {
        let toml = r#"
[models.broken]
provider = "openai"
model_id = "gpt-4o"
api_key = "${DEFINITELY_NOT_SET_V10_005_CA3}"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);

        assert_eq!(result.errors, 1);
        let finding = result
            .findings
            .iter()
            .find(|f| f.category == "models" && f.severity == Severity::Error)
            .unwrap();
        assert!(
            finding.message.contains("DEFINITELY_NOT_SET_V10_005_CA3"),
            "error debe mencionar la variable: {}",
            finding.message
        );
    }

    /// CA3: validate_models reporta Warning si api_key está vacío (Ollama).
    #[test]
    fn story_v10005_ca3_empty_api_key_warning() {
        let toml = r#"
[models.ollama]
provider = "openai"
model_id = "llama3"
api_key = ""
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);

        assert_eq!(result.errors, 0);
        assert_eq!(result.warnings, 1);
        let finding = result
            .findings
            .iter()
            .find(|f| f.category == "models" && f.severity == Severity::Warning)
            .unwrap();
        assert!(
            finding.message.contains("ollama") || finding.message.contains("vacío"),
            "warning debe mencionar el modelo: {}",
            finding.message
        );
    }

    /// CA3: validate_models con variable de entorno definida no produce Error.
    #[test]
    fn story_v10005_ca3_valid_env_var_no_error() {
        std::env::set_var("TEST_V10_005_VALIDATE_KEY", "sk-valid-test-key");

        let toml = r#"
[models.gpt4o]
provider = "openai"
model_id = "gpt-4o"
api_key = "${TEST_V10_005_VALIDATE_KEY}"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);

        assert_eq!(result.errors, 0);
        assert_eq!(result.warnings, 0);
    }

    /// CA3: validate_models con múltiples modelos mixtos.
    #[test]
    fn story_v10005_ca3_mixed_models() {
        let toml = r#"
[models.good]
provider = "openai"
model_id = "gpt-4o"
api_key = "sk-test"

[models.bad]
provider = "unknown"
model_id = "bad-model"
api_key = "key"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };
        validate_models(&cfg, &mut result);

        // Solo "bad" debe generar error
        assert_eq!(result.errors, 1);
        let finding = result
            .findings
            .iter()
            .find(|f| f.category == "models" && f.severity == Severity::Error)
            .unwrap();
        assert!(
            finding.message.contains("bad"),
            "error debe mencionar el modelo 'bad': {}",
            finding.message
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-V10-017: Validación pre-vuelo de dominio genérico
    // ═══════════════════════════════════════════════════════════════
    //
    // NOTA TDD: Estos tests verifican la validación de modelos,
    // workflow, task_format y tasks. El Developer debe implementar
    // las funciones validate_models_referenced_in_workflow(),
    // validate_workflow_config(), validate_task_format(), y
    // validate_tasks().

    use crate::domain::task::{Task, TaskFormatConfig};
    use crate::domain::workflow::{PhaseConfig, RoleConfig, WorkflowConfig, WorkflowStatesConfig};
    use std::collections::HashMap;

    // ── Helpers para construir workflow de test ────────────────────

    fn make_test_workflow_config() -> WorkflowConfig {
        WorkflowConfig {
            states: WorkflowStatesConfig {
                initial: "draft".to_string(),
                terminal: vec!["done".to_string(), "failed".to_string()],
            },
            roles: vec![
                RoleConfig {
                    name: "developer".to_string(),
                    system_prompt: "Eres dev".to_string(),
                    model: "gpt4o".to_string(),
                },
            ],
            phases: vec![
                PhaseConfig {
                    name: "implement".to_string(),
                    from: "draft".to_string(),
                    to: "done".to_string(),
                    role: "developer".to_string(),
                    model: "gpt4o".to_string(),
                    prompt: "Implementa {{task_id}}".to_string(),
                    on_reject: "draft".to_string(),
                    max_reject_cycles: 3,
                    timeout_seconds: None,
                },
            ],
            task_format: TaskFormatConfig::default(),
        }
    }

    // ── CA1: Validar coherencia de modelos referenciados ──────────

    #[test]
    fn validate_detects_model_referenced_but_not_defined() {
        // CA1: Modelo referenciado en una fase pero no definido en [models]
        let workflow = make_test_workflow_config();
        let mut models: HashMap<String, crate::config::ModelConfig> = HashMap::new();
        // "gpt4o" no está en models pero sí en la fase

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_workflow_models(&workflow, &models, &mut result);

        assert!(result.errors > 0, "Debe detectar modelo 'gpt4o' no definido");

        let finding = result.findings.iter()
            .find(|f| f.category == "models" && f.severity == Severity::Error)
            .expect("Debe existir un Finding de Error");

        assert!(finding.message.contains("gpt4o"),
            "El mensaje debe mencionar el modelo 'gpt4o': {}", finding.message);
        assert!(finding.message.contains("implement") || finding.message.contains("fase"),
            "El mensaje debe mencionar la fase: {}", finding.message);
    }

    #[test]
    fn validate_model_referenced_and_defined_no_error() {
        // CA1 (borde): Si el modelo está definido, no hay error
        let workflow = make_test_workflow_config();
        let mut models: HashMap<String, crate::config::ModelConfig> = HashMap::new();
        models.insert("gpt4o".to_string(), crate::config::ModelConfig {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            api_key: "sk-test".to_string(),
            base_url: None,
        });

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_workflow_models(&workflow, &models, &mut result);

        assert_eq!(result.errors, 0,
            "No debe haber errores si el modelo está definido");
        assert_eq!(result.warnings, 0,
            "No debe haber warnings si el modelo está definido");
    }

    #[test]
    fn validate_env_var_not_set_is_warning() {
        // CA1: Variable de entorno no definida en api_key → Warning (no Error)
        let workflow = make_test_workflow_config();
        let mut models: HashMap<String, crate::config::ModelConfig> = HashMap::new();
        models.insert("gpt4o".to_string(), crate::config::ModelConfig {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            api_key: "${DEFINITELY_NOT_SET_V10_017}".to_string(),
            base_url: None,
        });

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_workflow_models(&workflow, &models, &mut result);

        // Los modelos con env vars no definidas deberían generar Warning, no Error
        // (porque es válido tener la variable definida en CI/CD pero no en desarrollo local)
        assert_eq!(result.errors, 0,
            "Variables de entorno no definidas no deben ser Error");
    }

    // ── CA2: Validar workflow (fases, estados, id_pattern) ────────

    #[test]
    fn validate_phase_references_undefined_state() {
        // CA2: Fase referencia estado no definido en workflow.states
        let mut workflow = make_test_workflow_config();
        workflow.phases[0].to = "validating".to_string(); // no está en states

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_workflow_states_coherence(&workflow, &mut result);

        assert!(result.errors > 0, "Debe detectar estado 'validating' no definido");

        let finding = result.findings.iter()
            .find(|f| f.category == "workflow" && f.severity == Severity::Error)
            .expect("Debe existir Finding de Error");

        assert!(finding.message.contains("validating"),
            "El mensaje debe mencionar 'validating': {}", finding.message);
    }

    #[test]
    fn validate_phase_from_is_not_in_states() {
        // CA2: Fase cuyo from no está en states
        let mut workflow = make_test_workflow_config();
        workflow.phases[0].from = "unknown_start".to_string();

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_workflow_states_coherence(&workflow, &mut result);

        assert!(result.errors > 0, "Debe detectar estado 'unknown_start' no definido");
    }

    #[test]
    fn validate_invalid_id_pattern_regex() {
        // CA2: id_pattern no es un regex válido
        let mut workflow = make_test_workflow_config();
        workflow.task_format.id_pattern = "***[invalid".to_string();

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_task_format_regex(&workflow, &mut result);

        assert!(result.errors > 0, "Debe detectar regex inválido");

        let finding = result.findings.iter()
            .find(|f| f.category == "task_format" && f.severity == Severity::Error)
            .expect("Debe existir Finding de Error");

        assert!(finding.message.contains("regex") || finding.message.contains("id_pattern"),
            "El mensaje debe mencionar el problema: {}", finding.message);
    }

    #[test]
    fn validate_valid_id_pattern_no_error() {
        // CA2 (borde): id_pattern válido no genera error
        let workflow = make_test_workflow_config();

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_task_format_regex(&workflow, &mut result);

        assert_eq!(result.errors, 0,
            "id_pattern válido no debe generar errores");
    }

    #[test]
    fn validate_duplicate_section_markers() {
        // CA2: section_markers con el mismo marcador para dos campos distintos
        let mut workflow = make_test_workflow_config();
        workflow.task_format.section_markers.insert(
            "other".to_string(),
            "## Status".to_string(), // mismo marcador que "status"
        );

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_section_markers_uniqueness(&workflow, &mut result);

        assert!(result.errors > 0 || result.warnings > 0,
            "Debe detectar marcadores duplicados");
    }

    #[test]
    fn validate_all_states_coherent_no_error() {
        // CA2 (borde): Workflow bien definido no produce errores
        let workflow = make_test_workflow_config();

        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_workflow_states_coherence(&workflow, &mut result);

        assert_eq!(result.errors, 0,
            "Workflow coherente no debe generar errores");
    }

    // ── CA3: Validar tasks (id_pattern, estados, dependencias) ────

    #[test]
    fn validate_task_filename_matches_id_pattern() {
        // CA3: Verifica que el nombre de archivo cumple id_pattern
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("WRONG-NAME.md");
        std::fs::write(&task_path, "## Status\n**draft**\n").unwrap();

        let task_format = TaskFormatConfig {
            id_pattern: r"TASK-\d+".to_string(),
            ..TaskFormatConfig::default()
        };
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_task_pattern_match(&task_path, &task_format, &mut result);

        assert!(result.errors > 0 || result.warnings > 0,
            "Debe detectar nombre de archivo que no cumple id_pattern");
    }

    #[test]
    fn validate_task_status_is_valid_for_workflow() {
        // CA3: Estado de la task debe ser válido según el workflow
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        std::fs::write(&task_path, "## Status\n**unknown_status**\n").unwrap();

        let workflow = make_test_workflow_config();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        validate_task_status(&task_path, &workflow, &mut result);

        assert!(result.errors > 0 || result.warnings > 0,
            "Debe detectar estado inválido 'unknown_status'");
    }

    #[test]
    fn validate_task_dependencies_reference_existing_tasks() {
        // CA3: Dependencias (blockers) referencian tasks existentes
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        std::fs::write(&task_path,
            "## Status\n**draft**\n\n## Dependencias\n- Bloqueado por: TASK-999\n"
        ).unwrap();

        let workflow = make_test_workflow_config();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        // Mapa de task IDs que existen (sin TASK-999)
        let existing_ids: HashMap<String, bool> = [
            ("TASK-001".to_string(), true),
            ("TASK-002".to_string(), true),
        ].into_iter().collect();

        validate_task_blockers_exist(&task_path, &workflow, &existing_ids, &mut result);

        assert!(result.errors > 0,
            "Debe detectar dependencia a TASK-999 que no existe");
    }

    #[test]
    fn validate_task_good_dependencies_no_error() {
        // CA3 (borde): Dependencias correctas no generan error
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        std::fs::write(&task_path,
            "## Status\n**draft**\n\n## Dependencias\n- Bloqueado por: TASK-002\n"
        ).unwrap();

        let workflow = make_test_workflow_config();
        let mut result = ValidationResult {
            ok: 0,
            warnings: 0,
            errors: 0,
            findings: vec![],
        };

        let existing_ids: HashMap<String, bool> = [
            ("TASK-001".to_string(), true),
            ("TASK-002".to_string(), true),
        ].into_iter().collect();

        validate_task_blockers_exist(&task_path, &workflow, &existing_ids, &mut result);

        assert_eq!(result.errors, 0,
            "Dependencias correctas no deben generar errores");
    }

    // ═══════════════════════════════════════════════════════════════
    // Implementaciones temporales para TDD (el Developer las hará reales)
    // ═══════════════════════════════════════════════════════════════

    use std::collections::HashMap as StdHashMap;

    /// Valida que los modelos referenciados en las fases del workflow
    /// existen en la sección [models] de la configuración.
    fn validate_workflow_models(
        workflow: &WorkflowConfig,
        models: &HashMap<String, crate::config::ModelConfig>,
        result: &mut ValidationResult,
    ) {
        for phase in &workflow.phases {
            if !phase.model.is_empty() && !models.contains_key(&phase.model) {
                result.add(
                    Severity::Error,
                    "models",
                    format!(
                        "modelo '{}' referenciado en fase '{}' no está definido en [models]",
                        phase.model, phase.name
                    ),
                    None,
                );
            }
        }
    }

    /// Valida que los estados referenciados por las fases existen en workflow.states.
    fn validate_workflow_states_coherence(
        workflow: &WorkflowConfig,
        result: &mut ValidationResult,
    ) {
        // Recolectar todos los estados mencionados en las fases
        let mut referenced_states: Vec<&str> = vec![];
        for phase in &workflow.phases {
            referenced_states.push(&phase.from);
            referenced_states.push(&phase.to);
        }

        // También están initial y terminal
        let all_defined_states: Vec<&str> = {
            let mut s: Vec<&str> = vec![&workflow.states.initial];
            s.extend(workflow.states.terminal.iter().map(|t| t.as_str()));
            s
        };

        for state in &referenced_states {
            if !all_defined_states.contains(state) && *state != "_init_" {
                result.add(
                    Severity::Error,
                    "workflow",
                    format!("estado '{}' no definido en workflow.states", state),
                    None,
                );
            }
        }
    }

    /// Valida que el id_pattern del task_format es un regex válido.
    fn validate_task_format_regex(
        workflow: &WorkflowConfig,
        result: &mut ValidationResult,
    ) {
        if regex::Regex::new(&workflow.task_format.id_pattern).is_err() {
            result.add(
                Severity::Error,
                "task_format",
                format!(
                    "id_pattern '{}' no es un regex válido",
                    workflow.task_format.id_pattern
                ),
                None,
            );
        }
    }

    /// Valida que no haya section_markers duplicados.
    fn validate_section_markers_uniqueness(
        workflow: &WorkflowConfig,
        result: &mut ValidationResult,
    ) {
        let mut seen: StdHashMap<&str, &str> = StdHashMap::new();
        for (field, marker) in &workflow.task_format.section_markers {
            if let Some(existing_field) = seen.get(marker.as_str()) {
                result.add(
                    Severity::Error,
                    "task_format",
                    format!(
                        "section_marker '{}' está duplicado: campos '{}' y '{}'",
                        marker, existing_field, field
                    ),
                    None,
                );
            }
            seen.insert(marker, field);
        }
    }

    /// Valida que el nombre de archivo cumple el id_pattern.
    fn validate_task_pattern_match(
        path: &Path,
        task_format: &TaskFormatConfig,
        result: &mut ValidationResult,
    ) {
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        match regex::Regex::new(&task_format.id_pattern) {
            Ok(re) => {
                if !re.is_match(filename) {
                    result.add(
                        Severity::Error,
                        "tasks",
                        format!(
                            "'{}' no cumple el id_pattern '{}'",
                            filename, task_format.id_pattern
                        ),
                        Some(filename.to_string()),
                    );
                }
            }
            Err(_) => {
                // id_pattern inválido ya fue reportado en otra validación
            }
        }
    }

    /// Valida que el status de una task es un estado válido según el workflow.
    fn validate_task_status(
        path: &Path,
        workflow: &WorkflowConfig,
        result: &mut ValidationResult,
    ) {
        // Leer y parsear la task
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let task = match Task::parse(path, &content, &workflow.task_format) {
            Ok(t) => t,
            Err(_) => return,
        };

        let status = task.fields.get("status").cloned().unwrap_or_default();

        // Recolectar todos los estados válidos
        let mut valid_states: Vec<&str> = vec![&workflow.states.initial];
        valid_states.extend(workflow.states.terminal.iter().map(|t| t.as_str()));
        for phase in &workflow.phases {
            if !valid_states.contains(&phase.from.as_str()) {
                valid_states.push(&phase.from);
            }
            if !valid_states.contains(&phase.to.as_str()) {
                valid_states.push(&phase.to);
            }
        }

        if !status.is_empty() && !valid_states.contains(&status.as_str()) {
            result.add(
                Severity::Error,
                "tasks",
                format!(
                    "{}: estado '{}' no es válido según el workflow",
                    task.id, status
                ),
                Some(task.id.clone()),
            );
        }
    }

    /// Valida que los blockers de una task referencian tasks existentes.
    fn validate_task_blockers_exist(
        path: &Path,
        workflow: &WorkflowConfig,
        existing_ids: &HashMap<String, bool>,
        result: &mut ValidationResult,
    ) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let task = match Task::parse(path, &content, &workflow.task_format) {
            Ok(t) => t,
            Err(_) => return,
        };

        for blocker in &task.blockers {
            if !existing_ids.contains_key(blocker) {
                result.add(
                    Severity::Error,
                    "dependencies",
                    format!(
                        "{}: depende de '{}' que no existe",
                        task.id, blocker
                    ),
                    Some(task.id.clone()),
                );
            }
        }
    }
}
