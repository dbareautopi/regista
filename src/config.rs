//! Carga y validación de la configuración de regista.
//!
//! La configuración se lee de un archivo TOML (por defecto `.regista/config.toml`
//! en la raíz del proyecto). Todos los campos tienen valores por defecto razonables
//! para que un proyecto mínimo solo necesite indicar dónde están las historias
//! y qué provider usar.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuración del stack tecnológico del proyecto anfitrión.
///
/// Totalmente opcional. Si no se define, los prompts usan instrucciones
/// genéricas ("compila/construye el proyecto") y el skill del agente
/// se encarga de interpretarlas.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct StackConfig {
    /// Comando para compilar/construir el proyecto.
    pub build_command: Option<String>,
    /// Comando para ejecutar los tests.
    pub test_command: Option<String>,
    /// Comando para ejecutar el linter.
    pub lint_command: Option<String>,
    /// Comando para verificar el formato de código.
    pub fmt_command: Option<String>,
    /// Directorio de código fuente (para placeholders de tests).
    pub src_dir: Option<String>,
}

/// Configuración completa del orquestador.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub project: ProjectConfig,
    pub agents: AgentsConfig,
    pub limits: LimitsConfig,
    pub hooks: HooksConfig,
    pub git: GitConfig,
    /// Configuración del stack tecnológico (comandos de build, test, lint, fmt).
    #[serde(default)]
    pub stack: StackConfig,
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

/// Configuración de un rol específico del workflow.
///
/// Cada rol puede especificar opcionalmente un provider distinto al global
/// y un path explícito de instrucciones. Si no se especifican, heredan del
/// provider global y usan la convención de directorio del provider.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AgentRoleConfig {
    /// Nombre del provider para este rol ("pi", "claude", "codex", "opencode").
    /// Si es `None`, hereda el provider global de `AgentsConfig::provider`.
    pub provider: Option<String>,

    /// Ruta explícita al archivo de instrucciones (skill, agent, command).
    /// Si es `None`, se usa la convención de directorio del provider.
    pub skill: Option<String>,
}

/// Configuración de agentes: providers y skills para cada rol.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Provider por defecto para todos los roles.
    /// Si no se especifica, se usa "pi".
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Configuración del Product Owner.
    #[serde(default)]
    pub product_owner: AgentRoleConfig,

    /// Configuración del QA Engineer.
    #[serde(default)]
    pub qa_engineer: AgentRoleConfig,

    /// Configuración del Developer.
    #[serde(default)]
    pub developer: AgentRoleConfig,

    /// Configuración del Reviewer.
    #[serde(default)]
    pub reviewer: AgentRoleConfig,
}

impl AgentsConfig {
    /// Itera sobre los 4 roles con su nombre canónico.
    pub fn all_roles() -> [&'static str; 4] {
        ["product_owner", "qa_engineer", "developer", "reviewer"]
    }
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

    /// Timeout en segundos para cada invocación del agente.
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout_seconds: u64,

    /// Tiempo máximo total de pared en segundos (seguridad).
    #[serde(default = "default_max_wall_time")]
    pub max_wall_time_seconds: u64,

    /// Delay base en segundos para el backoff exponencial entre reintentos.
    #[serde(default = "default_retry_delay_base")]
    pub retry_delay_base_seconds: u64,

    /// Máximo de iteraciones del bucle plan→validate→corregir.
    #[serde(default = "default_plan_max_iterations")]
    pub plan_max_iterations: u32,

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
    ".regista/stories".into()
}
fn default_story_pattern() -> String {
    "STORY-*.md".into()
}
fn default_epics_dir() -> String {
    ".regista/epics".into()
}
fn default_decisions_dir() -> String {
    ".regista/decisions".into()
}
fn default_log_dir() -> String {
    ".regista/logs".into()
}
fn default_provider() -> String {
    "pi".into()
}
fn default_max_iterations() -> u32 {
    0
}
fn default_max_retries() -> u32 {
    5
}
fn default_max_reject_cycles() -> u32 {
    8
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
fn default_plan_max_iterations() -> u32 {
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
            provider: default_provider(),
            product_owner: AgentRoleConfig::default(),
            qa_engineer: AgentRoleConfig::default(),
            developer: AgentRoleConfig::default(),
            reviewer: AgentRoleConfig::default(),
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
            plan_max_iterations: default_plan_max_iterations(),
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
    /// Busca `<project_root>/.regista/config.toml`. Si el archivo no existe,
    /// devuelve la configuración por defecto con paths bajo `.regista/`.
    pub fn load(project_root: &Path, config_path: Option<&Path>) -> anyhow::Result<Self> {
        let default_config_path = project_root.join(".regista/config.toml");
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
    use crate::infra::providers;

    // ── Defaults ───────────────────────────────────────────────────────

    #[test]
    fn default_config_is_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.project.stories_dir, ".regista/stories");
        assert_eq!(cfg.project.story_pattern, "STORY-*.md");
        assert_eq!(cfg.agents.provider, "pi");
        assert_eq!(cfg.limits.max_iterations, 0);
        assert_eq!(cfg.limits.max_retries_per_step, 5);
        assert_eq!(cfg.limits.max_reject_cycles, 8);
        assert_eq!(cfg.limits.agent_timeout_seconds, 1800);
        assert!(cfg.hooks.post_qa.is_none());
        assert!(cfg.git.enabled);
    }

    #[test]
    fn default_skill_for_role_uses_pi_convention() {
        let cfg = Config::default();
        // Por defecto, el provider es pi → usa .pi/skills/<rol>/SKILL.md
        // Roles con underscore se convierten a hyphens (requisito de pi)
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "product_owner"),
            ".pi/skills/product-owner/SKILL.md"
        );
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "qa_engineer"),
            ".pi/skills/qa-engineer/SKILL.md"
        );
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "developer"),
            ".pi/skills/developer/SKILL.md"
        );
    }

    #[test]
    fn default_provider_for_role_is_pi() {
        let cfg = Config::default();
        for role in AgentsConfig::all_roles() {
            assert_eq!(providers::provider_for_role(&cfg.agents, role), "pi");
        }
    }

    // ── Parseo ─────────────────────────────────────────────────────────

    #[test]
    fn parse_minimal_config_just_provider() {
        let toml = r#"
[agents]
provider = "claude"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider, "claude");
        // Los roles heredan el provider global
        assert_eq!(
            providers::provider_for_role(&cfg.agents, "product_owner"),
            "claude"
        );
        assert_eq!(
            providers::provider_for_role(&cfg.agents, "developer"),
            "claude"
        );
    }

    #[test]
    fn parse_role_specific_provider() {
        let toml = r#"
[agents]
provider = "claude"

[agents.developer]
provider = "pi"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        // PO hereda claude del global
        assert_eq!(
            providers::provider_for_role(&cfg.agents, "product_owner"),
            "claude"
        );
        // Dev tiene su propio provider
        assert_eq!(providers::provider_for_role(&cfg.agents, "developer"), "pi");
    }

    #[test]
    fn parse_explicit_skill_path() {
        let toml = r#"
[agents]
provider = "pi"

[agents.reviewer]
skill = ".pi/skills/senior-reviewer/SKILL.md"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "reviewer"),
            ".pi/skills/senior-reviewer/SKILL.md"
        );
        // Los demás usan la convención
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "developer"),
            ".pi/skills/developer/SKILL.md"
        );
    }

    #[test]
    fn parse_mixed_providers_with_explicit_skills() {
        let toml = r#"
[agents]
provider = "pi"

[agents.product_owner]
provider = "claude"
skill = ".claude/agents/po-custom.md"

[agents.developer]
provider = "codex"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();

        assert_eq!(
            providers::provider_for_role(&cfg.agents, "product_owner"),
            "claude"
        );
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "product_owner"),
            ".claude/agents/po-custom.md"
        );

        assert_eq!(
            providers::provider_for_role(&cfg.agents, "developer"),
            "codex"
        );
        assert_eq!(
            providers::skill_for_role(&cfg.agents, "developer"),
            ".agents/skills/developer/SKILL.md"
        );

        // QA y Reviewer heredan pi
        assert_eq!(
            providers::provider_for_role(&cfg.agents, "qa_engineer"),
            "pi"
        );
        assert_eq!(providers::provider_for_role(&cfg.agents, "reviewer"), "pi");
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
[project]
stories_dir = "docs/stories"
story_pattern = "*.md"

[agents]
provider = "claude"

[limits]
max_iterations = 5
max_retries_per_step = 3

[hooks]
post_dev = "cargo test"

[git]
enabled = false
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.project.stories_dir, "docs/stories");
        assert_eq!(cfg.agents.provider, "claude");
        assert_eq!(cfg.limits.max_iterations, 5);
        assert_eq!(cfg.hooks.post_dev.as_deref(), Some("cargo test"));
        assert!(!cfg.git.enabled);
    }

    #[test]
    fn parse_stack_config_all_fields() {
        let toml = r#"
[stack]
build_command = "npm run build"
test_command = "npm test"
lint_command = "eslint ."
fmt_command = "prettier --check ."
src_dir = "src/"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.stack.build_command.as_deref(), Some("npm run build"));
        assert_eq!(cfg.stack.test_command.as_deref(), Some("npm test"));
        assert_eq!(cfg.stack.lint_command.as_deref(), Some("eslint ."));
        assert_eq!(cfg.stack.fmt_command.as_deref(), Some("prettier --check ."));
        assert_eq!(cfg.stack.src_dir.as_deref(), Some("src/"));
    }

    #[test]
    fn parse_stack_config_partial() {
        let toml = r#"
[stack]
test_command = "pytest"
src_dir = "src/"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.stack.test_command.as_deref(), Some("pytest"));
        assert!(cfg.stack.build_command.is_none());
        assert!(cfg.stack.lint_command.is_none());
    }

    #[test]
    fn default_stack_is_all_none() {
        let stack = StackConfig::default();
        assert!(stack.build_command.is_none());
        assert!(stack.test_command.is_none());
        assert!(stack.lint_command.is_none());
        assert!(stack.fmt_command.is_none());
        assert!(stack.src_dir.is_none());
    }
}
