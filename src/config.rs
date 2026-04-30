//! Carga y validación de la configuración de regista.
//!
//! La configuración se lee de un archivo TOML (por defecto `.regista.toml`
//! en la raíz del proyecto). Todos los campos tienen valores por defecto razonables
//! para que un proyecto mínimo solo necesite indicar dónde están las historias
//! y qué skills usar.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuración completa del orquestador.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub project: ProjectConfig,
    pub agents: AgentsConfig,
    pub limits: LimitsConfig,
    pub hooks: HooksConfig,
    pub git: GitConfig,
}

/// Dónde encontrar los artefactos del workflow.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Directorio que contiene las historias de usuario (relativo a la raíz del proyecto).
    #[serde(default = "default_stories_dir")]
    pub stories_dir: String,

    /// Patrón glob para encontrar archivos de historia dentro de `stories_dir`.
    #[serde(default = "default_story_pattern")]
    pub story_pattern: String,

    /// Directorio de épicas (opcional, necesario si se usa filtro --epics).
    #[serde(default = "default_epics_dir")]
    #[allow(dead_code)]
    pub epics_dir: String,

    /// Directorio donde los agentes documentan decisiones.
    #[serde(default = "default_decisions_dir")]
    pub decisions_dir: String,

    /// Directorio donde se guardan los logs del orquestador.
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
}

/// Rutas a los skills de `pi` para cada rol del workflow.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Skill que actúa como Product Owner (groom + validate).
    #[serde(default = "default_po_skill")]
    pub product_owner: String,

    /// Skill que actúa como QA Engineer (escribe tests).
    #[serde(default = "default_qa_skill")]
    pub qa_engineer: String,

    /// Skill que actúa como Developer (implementa código).
    #[serde(default = "default_dev_skill")]
    pub developer: String,

    /// Skill que actúa como Reviewer (puerta técnica).
    #[serde(default = "default_reviewer_skill")]
    pub reviewer: String,
}

/// Límites operacionales para evitar bucles infinitos o bloqueos.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    /// Número máximo de iteraciones del loop principal.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Reintentos máximos por invocación de agente (con backoff exponencial).
    #[serde(default = "default_max_retries")]
    pub max_retries_per_step: u32,

    /// Número máximo de ciclos de rechazo (InReview ↔ InProgress) antes de marcar Failed.
    #[serde(default = "default_max_reject_cycles")]
    pub max_reject_cycles: u32,

    /// Timeout en segundos para cada invocación de `pi`.
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout_seconds: u64,

    /// Tiempo máximo total de pared en segundos (seguridad).
    #[serde(default = "default_max_wall_time")]
    pub max_wall_time_seconds: u64,

    /// Delay base en segundos para el backoff exponencial entre reintentos.
    #[serde(default = "default_retry_delay_base")]
    pub retry_delay_base_seconds: u64,

    /// Máximo de iteraciones del bucle groom→validate→corregir.
    #[serde(default = "default_groom_max_iterations")]
    pub groom_max_iterations: u32,

    /// Inyectar stderr del intento fallido en el prompt del reintento.
    #[serde(default = "default_inject_feedback")]
    pub inject_feedback_on_retry: bool,
}

/// Comandos opcionales de verificación post-fase.
///
/// Si un hook falla (exit code ≠ 0), se hace rollback del paso.
/// Si no se define, esa fase no tiene verificación.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct HooksConfig {
    /// Comando a ejecutar después de que QA escriba tests.
    pub post_qa: Option<String>,

    /// Comando a ejecutar después de que Dev implemente o corrija.
    pub post_dev: Option<String>,

    /// Comando a ejecutar después de que Reviewer apruebe.
    pub post_reviewer: Option<String>,
}

/// Configuración de snapshots git (para rollback en caso de fallo).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Si es `true`, se crean snapshots git antes de cada paso y se hace
    /// rollback si la verificación falla.
    #[serde(default = "default_git_enabled")]
    pub enabled: bool,
}

// ── defaults ─────────────────────────────────────────────────────────────

fn default_stories_dir() -> String {
    "product/stories".into()
}
fn default_story_pattern() -> String {
    "STORY-*.md".into()
}
fn default_epics_dir() -> String {
    "product/epics".into()
}
fn default_decisions_dir() -> String {
    "product/decisions".into()
}
fn default_log_dir() -> String {
    "product/logs".into()
}
fn default_po_skill() -> String {
    ".pi/skills/product-owner/SKILL.md".into()
}
fn default_qa_skill() -> String {
    ".pi/skills/qa-engineer/SKILL.md".into()
}
fn default_dev_skill() -> String {
    ".pi/skills/developer/SKILL.md".into()
}
fn default_reviewer_skill() -> String {
    ".pi/skills/reviewer/SKILL.md".into()
}
fn default_max_iterations() -> u32 {
    10
}
fn default_max_retries() -> u32 {
    5
}
fn default_max_reject_cycles() -> u32 {
    3
}
fn default_agent_timeout() -> u64 {
    1800
}
fn default_max_wall_time() -> u64 {
    28800
}
fn default_retry_delay_base() -> u64 {
    10
}
fn default_groom_max_iterations() -> u32 {
    5
}
fn default_inject_feedback() -> bool {
    true
}
fn default_git_enabled() -> bool {
    true
}

// ── defaults para types que lo necesitan ─────────────────────────────────

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            stories_dir: default_stories_dir(),
            story_pattern: default_story_pattern(),
            epics_dir: default_epics_dir(),
            decisions_dir: default_decisions_dir(),
            log_dir: default_log_dir(),
        }
    }
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            product_owner: default_po_skill(),
            qa_engineer: default_qa_skill(),
            developer: default_dev_skill(),
            reviewer: default_reviewer_skill(),
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            max_retries_per_step: default_max_retries(),
            max_reject_cycles: default_max_reject_cycles(),
            agent_timeout_seconds: default_agent_timeout(),
            max_wall_time_seconds: default_max_wall_time(),
            retry_delay_base_seconds: default_retry_delay_base(),
            groom_max_iterations: default_groom_max_iterations(),
            inject_feedback_on_retry: default_inject_feedback(),
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: default_git_enabled(),
        }
    }
}

// ── carga ────────────────────────────────────────────────────────────────

impl Config {
    /// Carga la configuración desde un proyecto.
    ///
    /// Busca `config_path` dentro de `project_root`. Si `config_path` es `None`,
    /// usa `project_root/.regista.toml`. Si el archivo no existe, devuelve
    /// la configuración por defecto (todos los paths relativos a `project_root`).
    pub fn load(project_root: &Path, config_path: Option<&Path>) -> anyhow::Result<Self> {
        let default_config_path = project_root.join(".regista.toml");
        let config_path = config_path.unwrap_or(&default_config_path);

        let config = if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            toml::from_str(&content)?
        } else {
            tracing::warn!(
                "No se encontró {} — usando configuración por defecto",
                config_path.display()
            );
            Config::default()
        };

        config.validate(project_root)?;
        Ok(config)
    }

    /// Valida que los campos de configuración sean coherentes.
    fn validate(&self, project_root: &Path) -> anyhow::Result<()> {
        // Verificar que stories_dir existe
        let stories_path = project_root.join(&self.project.stories_dir);
        if !stories_path.exists() {
            anyhow::bail!(
                "El directorio de historias no existe: {}",
                stories_path.display()
            );
        }
        if !stories_path.is_dir() {
            anyhow::bail!(
                "La ruta de historias no es un directorio: {}",
                stories_path.display()
            );
        }

        // Crear directorios necesarios
        for dir in [&self.project.decisions_dir, &self.project.log_dir] {
            let path = project_root.join(dir);
            std::fs::create_dir_all(&path)?;
        }

        Ok(())
    }

    /// Resuelve una ruta relativa al proyecto como PathBuf absoluto.
    #[allow(dead_code)]
    pub fn resolve(&self, project_root: &Path, relative: &str) -> PathBuf {
        project_root.join(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.project.stories_dir, "product/stories");
        assert_eq!(cfg.project.story_pattern, "STORY-*.md");
        assert_eq!(
            cfg.agents.product_owner,
            ".pi/skills/product-owner/SKILL.md"
        );
        assert_eq!(cfg.limits.max_iterations, 10);
        assert_eq!(cfg.limits.max_retries_per_step, 5);
        assert_eq!(cfg.limits.max_reject_cycles, 3);
        assert_eq!(cfg.limits.agent_timeout_seconds, 1800);
        assert!(cfg.hooks.post_qa.is_none());
        assert!(cfg.git.enabled);
    }

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
[agents]
product_owner = "skills/po.md"
qa_engineer = "skills/qa.md"
developer = "skills/dev.md"
reviewer = "skills/rev.md"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.product_owner, "skills/po.md");
        assert_eq!(cfg.agents.qa_engineer, "skills/qa.md");
        assert_eq!(cfg.agents.developer, "skills/dev.md");
        assert_eq!(cfg.agents.reviewer, "skills/rev.md");
        // El resto debe tener valores por defecto
        assert_eq!(cfg.project.stories_dir, "product/stories");
        assert_eq!(cfg.limits.max_iterations, 10);
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
[project]
stories_dir = "docs/stories"
story_pattern = "*.md"
epics_dir = "docs/epics"
decisions_dir = "docs/decisions"
log_dir = "docs/logs"

[agents]
product_owner = "a.md"
qa_engineer = "b.md"
developer = "c.md"
reviewer = "d.md"

[limits]
max_iterations = 5
max_retries_per_step = 3
max_reject_cycles = 2
agent_timeout_seconds = 600
max_wall_time_seconds = 3600
retry_delay_base_seconds = 5

[hooks]
post_qa = "echo qa"
post_dev = "echo dev"
post_reviewer = "echo rev"

[git]
enabled = false
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.project.stories_dir, "docs/stories");
        assert_eq!(cfg.project.story_pattern, "*.md");
        assert_eq!(cfg.limits.max_iterations, 5);
        assert_eq!(cfg.hooks.post_qa.as_deref(), Some("echo qa"));
        assert!(!cfg.git.enabled);
    }
}
