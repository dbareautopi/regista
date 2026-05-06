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

    /// Modelo LLM para este rol (ej: "gpt-5", "claude-sonnet-4").
    /// Si es `None`, hereda del `model` global de `AgentsConfig`,
    /// luego del YAML frontmatter del skill, y finalmente `"desconocido"`.
    #[serde(default)]
    #[allow(dead_code)]
    pub model: Option<String>,
}

/// Configuración de agentes: providers y skills para cada rol.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Provider por defecto para todos los roles.
    /// Si no se especifica, se usa "pi".
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Modelo LLM global para todos los roles (ej: "gpt-5", "claude-sonnet-4").
    /// Cada rol puede sobrescribirlo con `AgentRoleConfig.model`.
    /// Si es `None`, `model_for_role` intenta leerlo del YAML frontmatter
    /// del archivo de skill y usa `"desconocido"` como último fallback.
    #[serde(default)]
    #[allow(dead_code)]
    pub model: Option<String>,

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

    /// Resuelve el nombre del provider para un rol dado.
    ///
    /// Si el rol tiene `provider` explícito, lo usa.
    /// Si no, hereda del provider global.
    /// Si el rol no es reconocido, devuelve el provider global.
    pub fn provider_for_role(&self, role: &str) -> String {
        let config = match role {
            "product_owner" => &self.product_owner,
            "qa_engineer" => &self.qa_engineer,
            "developer" => &self.developer,
            "reviewer" => &self.reviewer,
            _ => return self.provider.clone(),
        };
        config
            .provider
            .clone()
            .unwrap_or_else(|| self.provider.clone())
    }

    /// Resuelve la ruta al archivo de instrucciones (skill) para un rol.
    ///
    /// Si el rol tiene `skill` explícito, lo usa.
    /// Si no, usa la convención de directorio del provider.
    /// Si el rol no es reconocido, devuelve String vacía.
    pub fn skill_for_role(&self, role: &str) -> String {
        let config = match role {
            "product_owner" => &self.product_owner,
            "qa_engineer" => &self.qa_engineer,
            "developer" => &self.developer,
            "reviewer" => &self.reviewer,
            _ => return String::new(),
        };

        if let Some(ref skill) = config.skill {
            return skill.clone();
        }

        let provider_name = self.provider_for_role(role);
        let provider = crate::infra::providers::from_name(&provider_name).expect(
            "provider inválido en configuración — ejecuta 'regista validate' para diagnosticar",
        );
        provider.instruction_dir(role)
    }

    /// Resuelve el modelo LLM para un rol con la prioridad:
    /// 1. `AgentRoleConfig.model` del rol
    /// 2. `AgentsConfig.model` (global)
    /// 3. Campo `model` del YAML frontmatter del skill
    /// 4. `"desconocido"`
    ///
    /// No paniquea si `skill_path` no existe — trata el error como fallback al paso 3.
    #[allow(dead_code)]
    pub fn model_for_role(&self, role: &str, skill_path: &Path) -> String {
        // 1. Modelo específico del rol
        let role_config = match role {
            "product_owner" => Some(&self.product_owner),
            "qa_engineer" => Some(&self.qa_engineer),
            "developer" => Some(&self.developer),
            "reviewer" => Some(&self.reviewer),
            _ => None,
        };

        if let Some(config) = role_config {
            if let Some(ref model) = config.model {
                return model.clone();
            }
        }

        // 2. Modelo global
        if let Some(ref model) = self.model {
            return model.clone();
        }

        // 3. YAML frontmatter del skill
        if let Some(model) = crate::infra::providers::read_yaml_field(skill_path, "model") {
            return model;
        }

        // 4. Fallback
        "desconocido".to_string()
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
            model: None,
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
            cfg.agents.skill_for_role("product_owner"),
            ".pi/skills/product-owner/SKILL.md"
        );
        assert_eq!(
            cfg.agents.skill_for_role("qa_engineer"),
            ".pi/skills/qa-engineer/SKILL.md"
        );
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".pi/skills/developer/SKILL.md"
        );
    }

    #[test]
    fn default_provider_for_role_is_pi() {
        let cfg = Config::default();
        for role in AgentsConfig::all_roles() {
            assert_eq!(cfg.agents.provider_for_role(role), "pi");
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
        assert_eq!(cfg.agents.provider_for_role("product_owner"), "claude");
        assert_eq!(cfg.agents.provider_for_role("developer"), "claude");
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
        assert_eq!(cfg.agents.provider_for_role("product_owner"), "claude");
        // Dev tiene su propio provider
        assert_eq!(cfg.agents.provider_for_role("developer"), "pi");
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
            cfg.agents.skill_for_role("reviewer"),
            ".pi/skills/senior-reviewer/SKILL.md"
        );
        // Los demás usan la convención
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
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

        assert_eq!(cfg.agents.provider_for_role("product_owner"), "claude");
        assert_eq!(
            cfg.agents.skill_for_role("product_owner"),
            ".claude/agents/po-custom.md"
        );

        assert_eq!(cfg.agents.provider_for_role("developer"), "codex");
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".agents/skills/developer/SKILL.md"
        );

        // QA y Reviewer heredan pi
        assert_eq!(cfg.agents.provider_for_role("qa_engineer"), "pi");
        assert_eq!(cfg.agents.provider_for_role("reviewer"), "pi");
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-002: provider_for_role / skill_for_role en AgentsConfig
    // ═══════════════════════════════════════════════════════════════

    // ── CA1: provider_for_role como método de AgentsConfig ────────

    /// CA1: provider_for_role existe como método público en AgentsConfig.
    #[test]
    fn story002_ca1_method_exists_provider_for_role() {
        let cfg = Config::default();
        let result = cfg.agents.provider_for_role("developer");
        assert_eq!(result, "pi");
    }

    /// CA1: provider_for_role hereda del global cuando el rol no tiene provider explícito.
    #[test]
    fn story002_ca1_inherits_global_provider() {
        let toml = r#"
[agents]
provider = "claude"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider_for_role("product_owner"), "claude");
        assert_eq!(cfg.agents.provider_for_role("qa_engineer"), "claude");
        assert_eq!(cfg.agents.provider_for_role("developer"), "claude");
        assert_eq!(cfg.agents.provider_for_role("reviewer"), "claude");
    }

    /// CA1: provider_for_role usa el provider específico si el rol lo define.
    #[test]
    fn story002_ca1_role_specific_provider_overrides_global() {
        let toml = r#"
[agents]
provider = "pi"

[agents.developer]
provider = "claude"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider_for_role("developer"), "claude");
        assert_eq!(cfg.agents.provider_for_role("product_owner"), "pi");
    }

    /// CA1: provider_for_role con rol desconocido devuelve el provider global.
    #[test]
    fn story002_ca1_unknown_role_returns_global() {
        let cfg = Config::default();
        assert_eq!(cfg.agents.provider_for_role("unknown_role"), "pi");
    }

    /// CA1: provider_for_role para los 4 roles canónicos.
    #[test]
    fn story002_ca1_all_canonical_roles_return_pi_by_default() {
        let cfg = Config::default();
        for role in AgentsConfig::all_roles() {
            assert_eq!(
                cfg.agents.provider_for_role(role),
                "pi",
                "Rol '{role}' debería usar provider 'pi' por defecto"
            );
        }
    }

    /// CA1: provider_for_role con todos los providers canónicos.
    #[test]
    fn story002_ca1_each_canonical_provider_per_role() {
        let toml = r#"
[agents]
provider = "pi"

[agents.product_owner]
provider = "claude"

[agents.qa_engineer]
provider = "codex"

[agents.developer]
provider = "pi"

[agents.reviewer]
provider = "opencode"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider_for_role("product_owner"), "claude");
        assert_eq!(cfg.agents.provider_for_role("qa_engineer"), "codex");
        assert_eq!(cfg.agents.provider_for_role("developer"), "pi");
        assert_eq!(cfg.agents.provider_for_role("reviewer"), "opencode");
    }

    // ── CA2: skill_for_role como método de AgentsConfig ───────────

    /// CA2: skill_for_role existe como método público en AgentsConfig.
    #[test]
    fn story002_ca2_method_exists_skill_for_role() {
        let cfg = Config::default();
        let result = cfg.agents.skill_for_role("developer");
        assert_eq!(result, ".pi/skills/developer/SKILL.md");
    }

    /// CA2: skill_for_role con provider pi usa la convención .pi/skills/.
    #[test]
    fn story002_ca2_pi_convention_skill_paths() {
        let cfg = Config::default();
        assert_eq!(
            cfg.agents.skill_for_role("product_owner"),
            ".pi/skills/product-owner/SKILL.md"
        );
        assert_eq!(
            cfg.agents.skill_for_role("qa_engineer"),
            ".pi/skills/qa-engineer/SKILL.md"
        );
        assert_eq!(
            cfg.agents.skill_for_role("reviewer"),
            ".pi/skills/reviewer/SKILL.md"
        );
    }

    /// CA2: skill_for_role con provider claude usa la convención .claude/agents/.
    #[test]
    fn story002_ca2_claude_convention_skill_paths() {
        let toml = r#"
[agents]
provider = "claude"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".claude/agents/developer.md"
        );
    }

    /// CA2: skill_for_role con provider codex usa .agents/skills/.
    #[test]
    fn story002_ca2_codex_convention_skill_paths() {
        let toml = r#"
[agents]
provider = "codex"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".agents/skills/developer/SKILL.md"
        );
    }

    /// CA2: skill_for_role con provider opencode usa .opencode/agents/.
    #[test]
    fn story002_ca2_opencode_convention_skill_paths() {
        let toml = r#"
[agents]
provider = "opencode"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".opencode/agents/developer.md"
        );
    }

    /// CA2: skill_for_role respeta un skill path explícito.
    #[test]
    fn story002_ca2_explicit_skill_path_overrides_convention() {
        let toml = r#"
[agents]
provider = "pi"

[agents.reviewer]
skill = ".pi/skills/senior-reviewer/SKILL.md"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.agents.skill_for_role("reviewer"),
            ".pi/skills/senior-reviewer/SKILL.md"
        );
    }

    /// CA2: skill_for_role con rol desconocido devuelve String vacía.
    #[test]
    fn story002_ca2_unknown_role_returns_empty_string() {
        let cfg = Config::default();
        assert_eq!(cfg.agents.skill_for_role("unknown_role"), "");
    }

    /// CA2: skill_for_role con provider explícito por rol y skill explícito.
    #[test]
    fn story002_ca2_mixed_provider_and_explicit_skill() {
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
            cfg.agents.skill_for_role("product_owner"),
            ".claude/agents/po-custom.md"
        );
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".agents/skills/developer/SKILL.md"
        );
        // QA y Reviewer heredan pi
        assert_eq!(
            cfg.agents.skill_for_role("qa_engineer"),
            ".pi/skills/qa-engineer/SKILL.md"
        );
    }

    // ── CA3: all_roles en AgentsConfig ─────────────────────────────

    /// CA3: all_roles() devuelve los 4 roles canónicos.
    #[test]
    fn story002_ca3_all_roles_returns_four_canonical_roles() {
        let roles = AgentsConfig::all_roles();
        assert_eq!(roles.len(), 4);
        assert!(roles.contains(&"product_owner"));
        assert!(roles.contains(&"qa_engineer"));
        assert!(roles.contains(&"developer"));
        assert!(roles.contains(&"reviewer"));
    }

    /// CA3: all_roles() es iterable y sus elementos son &str.
    #[test]
    fn story002_ca3_all_roles_is_iterable() {
        let count = AgentsConfig::all_roles()
            .iter()
            .filter(|r| r.contains('_'))
            .count();
        assert_eq!(count, 2, "product_owner y qa_engineer contienen underscore");
    }

    // ── CA5: Callers usan cfg.agents.provider_for_role() ───────────

    /// CA5: Simula el patrón de uso exacto desde un caller (pipeline/plan/validate).
    #[test]
    fn story002_ca5_caller_pattern_provider_and_skill_for_role() {
        let cfg = Config::default();
        let role = "developer";
        let provider_name = cfg.agents.provider_for_role(role);
        let skill_path = cfg.agents.skill_for_role(role);

        assert_eq!(provider_name, "pi");
        assert_eq!(skill_path, ".pi/skills/developer/SKILL.md");
    }

    /// CA5: Simula el patrón con provider mixto (global claude, dev codex).
    #[test]
    fn story002_ca5_caller_pattern_mixed_providers() {
        let toml = r#"
[agents]
provider = "claude"

[agents.developer]
provider = "codex"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();

        // PO usa claude (global)
        assert_eq!(cfg.agents.provider_for_role("product_owner"), "claude");
        assert_eq!(
            cfg.agents.skill_for_role("product_owner"),
            ".claude/agents/product_owner.md"
        );

        // Dev usa codex (específico)
        assert_eq!(cfg.agents.provider_for_role("developer"), "codex");
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".agents/skills/developer/SKILL.md"
        );
    }

    /// CA5: Simula el patrón de uso desde plan.rs (PO con posible provider no-pi).
    #[test]
    fn story002_ca5_caller_pattern_plan_po_role() {
        let toml = r#"
[agents]
provider = "opencode"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();

        // plan.rs usa "product_owner" como rol fijo
        let provider_name = cfg.agents.provider_for_role("product_owner");
        let skill_path_str = cfg.agents.skill_for_role("product_owner");

        assert_eq!(provider_name, "opencode");
        assert_eq!(skill_path_str, ".opencode/agents/product-owner.md");
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-019: campo model en configuración + model_for_role()
    // ═══════════════════════════════════════════════════════════════

    // ── CA1: AgentsConfig.model ────────────────────────────────────

    /// CA1: AgentsConfig tiene campo `pub model: Option<String>`
    /// con `#[serde(default)]` y aparece en `Default` como `None`.
    #[test]
    fn story019_ca1_agents_config_model_field_exists_and_defaults_none() {
        let cfg = AgentsConfig::default();
        assert!(
            cfg.model.is_none(),
            "AgentsConfig.model debe ser None por defecto"
        );
    }

    /// CA1: Verifica que el campo model es `pub` (accesible desde fuera).
    #[test]
    fn story019_ca1_default_config_reports_model_as_none() {
        let cfg = Config::default();
        assert!(cfg.agents.model.is_none());
    }

    // ── CA2: AgentRoleConfig.model ──────────────────────────────────

    /// CA2: AgentRoleConfig tiene campo `pub model: Option<String>`
    /// con `#[serde(default)]` y valor por defecto `None`.
    #[test]
    fn story019_ca2_agent_role_config_model_field_exists_and_defaults_none() {
        let role_cfg = AgentRoleConfig::default();
        assert!(
            role_cfg.model.is_none(),
            "AgentRoleConfig.model debe ser None por defecto"
        );
    }

    /// CA2: Cada rol dentro de AgentsConfig hereda model=None por defecto.
    #[test]
    fn story019_ca2_all_roles_model_default_none() {
        let cfg = Config::default();
        assert!(cfg.agents.product_owner.model.is_none());
        assert!(cfg.agents.qa_engineer.model.is_none());
        assert!(cfg.agents.developer.model.is_none());
        assert!(cfg.agents.reviewer.model.is_none());
    }

    // ── CA3: model_for_role existe ──────────────────────────────────

    /// CA3: AgentsConfig::model_for_role(role, skill_path) -> String existe y compila.
    #[test]
    fn story019_ca3_model_for_role_exists_and_compiles() {
        let cfg = AgentsConfig::default();
        // Solo verificamos que compila y devuelve un String
        let result = cfg.model_for_role("developer", Path::new("nonexistent.md"));
        let _s: String = result;
    }

    /// CA3: model_for_role acepta los 4 roles canónicos.
    #[test]
    fn story019_ca3_model_for_role_accepts_all_canonical_roles() {
        let cfg = AgentsConfig::default();
        for role in AgentsConfig::all_roles() {
            let result = cfg.model_for_role(role, Path::new("nonexistent.md"));
            let _s: String = result; // compila para los 4 roles
        }
    }

    /// CA3: model_for_role acepta un rol desconocido sin paniquear.
    #[test]
    fn story019_ca3_model_for_role_accepts_unknown_role() {
        let cfg = AgentsConfig::default();
        let result = cfg.model_for_role("unknown_role", Path::new("nonexistent.md"));
        let _s: String = result;
    }

    // ── CA4: Role model > global ─────────────────────────────────────

    /// CA4: model_for_role devuelve AgentRoleConfig.model si está definido.
    #[test]
    fn story019_ca4_returns_role_model_when_defined() {
        let toml = r#"
[agents]
provider = "pi"

[agents.developer]
model = "gpt-5"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let result = cfg
            .agents
            .model_for_role("developer", Path::new("nonexistent.md"));
        assert_eq!(result, "gpt-5");
    }

    /// CA4: El modelo de rol prevalece sobre el global.
    #[test]
    fn story019_ca4_role_model_overrides_global() {
        let toml = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"

[agents.developer]
model = "gpt-5"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let result = cfg
            .agents
            .model_for_role("developer", Path::new("nonexistent.md"));
        assert_eq!(
            result, "gpt-5",
            "El modelo de rol (gpt-5) debe prevalecer sobre el global (claude-sonnet-4)"
        );
    }

    // ── CA5: Global model > YAML ─────────────────────────────────────

    /// CA5: model_for_role devuelve AgentsConfig.model (global) si no hay por rol.
    #[test]
    fn story019_ca5_returns_global_model_when_no_role_model() {
        let toml = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let result = cfg
            .agents
            .model_for_role("developer", Path::new("nonexistent.md"));
        assert_eq!(result, "claude-sonnet-4");
    }

    /// CA5: El global prevalece sobre el YAML del skill.
    #[test]
    fn story019_ca5_global_overrides_yaml_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("developer.md");
        std::fs::write(
            &skill,
            "---\nname: developer\nmodel: gpt-5-nano\n---\n# Skill\n",
        )
        .unwrap();

        let toml = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let result = cfg.agents.model_for_role("developer", &skill);
        assert_eq!(
            result, "claude-sonnet-4",
            "El modelo global debe prevalecer sobre el YAML del skill"
        );
    }

    // ── CA6: YAML frontmatter ────────────────────────────────────────

    /// CA6: model_for_role lee el campo model del YAML frontmatter del skill
    /// cuando no hay modelo en la config.
    #[test]
    fn story019_ca6_reads_yaml_frontmatter_when_no_config_model() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("developer.md");
        std::fs::write(
            &skill,
            "---\nname: developer\nmodel: opencode/gpt-5-nano\n---\n# Developer skill\n",
        )
        .unwrap();

        let cfg = AgentsConfig::default(); // sin model global ni por rol
        let result = cfg.model_for_role("developer", &skill);
        assert_eq!(result, "opencode/gpt-5-nano");
    }

    /// CA6: model_for_role lee del YAML incluso con rol desconocido
    /// (sin config de rol, usa YAML).
    #[test]
    fn story019_ca6_reads_yaml_for_role_without_config() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("custom-role.md");
        std::fs::write(
            &skill,
            "---\nname: custom-role\nmodel: mistral-large\n---\n# Body\n",
        )
        .unwrap();

        let cfg = AgentsConfig::default();
        let result = cfg.model_for_role("custom_role", &skill);
        assert_eq!(result, "mistral-large");
    }

    // ── CA7: Fallback "desconocido" ──────────────────────────────────

    /// CA7: model_for_role devuelve "desconocido" cuando no hay modelo
    /// en ningún lado (ni rol, ni global, ni YAML).
    #[test]
    fn story019_ca7_returns_desconocido_when_no_model_anywhere() {
        let cfg = AgentsConfig::default();
        let result = cfg.model_for_role("developer", Path::new("/nonexistent/skill.md"));
        assert_eq!(result, "desconocido");
    }

    /// CA7: "desconocido" con skill existente pero sin campo model en YAML.
    #[test]
    fn story019_ca7_returns_desconocido_when_yaml_has_no_model() {
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("developer.md");
        std::fs::write(
            &skill,
            "---\nname: developer\ndescription: A developer skill\n---\n# Body\n",
        )
        .unwrap();

        let cfg = AgentsConfig::default();
        let result = cfg.model_for_role("developer", &skill);
        assert_eq!(result, "desconocido");
    }

    // ── CA8: skill_path no existe → no paniquea ─────────────────────

    /// CA8: model_for_role no paniquea si skill_path no existe.
    #[test]
    fn story019_ca8_no_panic_on_missing_skill_path() {
        let cfg = AgentsConfig::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cfg.model_for_role("developer", Path::new("/definitivamente/no/existe.md"))
        }));
        assert!(
            result.is_ok(),
            "model_for_role NO debe paniquear si el skill_path no existe"
        );
    }

    /// CA8: model_for_role con skill_path inexistente devuelve "desconocido"
    /// (fallback último, no error ni panic).
    #[test]
    fn story019_ca8_missing_skill_returns_desconocido() {
        let cfg = AgentsConfig::default();
        let result = cfg.model_for_role("qa_engineer", Path::new("/tmp/no-existe.md"));
        assert_eq!(
            result, "desconocido",
            "skill_path inexistente debe devolver 'desconocido', no paniquear"
        );
    }

    /// CA8: model_for_role con skill_path vacío (Path::new("")) no paniquea.
    #[test]
    fn story019_ca8_empty_skill_path_does_not_panic() {
        let cfg = AgentsConfig::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cfg.model_for_role("developer", Path::new(""))
        }));
        assert!(
            result.is_ok(),
            "model_for_role no debe paniquear con skill_path vacío"
        );
    }

    // ── CA9: Backward compatibility ──────────────────────────────────

    /// CA9: Un .regista/config.toml existente sin campo `model`
    /// sigue parseando sin errores.
    #[test]
    fn story019_ca9_existing_config_without_model_parses_fine() {
        let toml = r#"
[agents]
provider = "pi"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider, "pi");
        assert!(cfg.agents.model.is_none());
    }

    /// CA9: Config con todos los campos excepto model funciona igual.
    #[test]
    fn story019_ca9_full_config_without_model_fields_still_parses() {
        let toml = r#"
[project]
stories_dir = "docs/stories"

[agents]
provider = "claude"

[agents.developer]
provider = "pi"
skill = ".pi/skills/senior-dev/SKILL.md"

[limits]
max_iterations = 10

[git]
enabled = false
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.project.stories_dir, "docs/stories");
        assert_eq!(cfg.agents.provider, "claude");
        assert_eq!(cfg.agents.provider_for_role("developer"), "pi");
        assert_eq!(
            cfg.agents.skill_for_role("developer"),
            ".pi/skills/senior-dev/SKILL.md"
        );
        assert_eq!(cfg.limits.max_iterations, 10);
        assert!(!cfg.git.enabled);
        // Los campos model son None (no definidos en TOML)
        assert!(cfg.agents.model.is_none());
        assert!(cfg.agents.developer.model.is_none());
    }

    /// CA9: Config con campo model global funciona.
    #[test]
    fn story019_ca9_config_with_global_model_parses() {
        let toml = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider, "pi");
        assert_eq!(cfg.agents.model.as_deref(), Some("claude-sonnet-4"));
    }

    /// CA9: Config con campo model por rol funciona.
    #[test]
    fn story019_ca9_config_with_role_model_parses() {
        let toml = r#"
[agents]
provider = "pi"

[agents.reviewer]
model = "gpt-5"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.agents.provider, "pi");
        assert!(cfg.agents.model.is_none());
        assert_eq!(cfg.agents.reviewer.model.as_deref(), Some("gpt-5"));
        assert!(cfg.agents.developer.model.is_none());
    }

    // ── CA10: Cobertura de los 4 casos ────────────────────────────────

    /// CA10: Los 4 casos de resolución están cubiertos:
    /// 1. Modelo de rol
    /// 2. Modelo global
    /// 3. YAML frontmatter del skill
    /// 4. "desconocido"
    ///
    /// Este test agrupa la verificación de los 4 casos en uno solo.
    #[test]
    fn story019_ca10_four_resolution_cases_covered() {
        // Preparamos un skill con modelo YAML
        let tmp = tempfile::tempdir().unwrap();
        let skill = tmp.path().join("product-owner.md");
        std::fs::write(
            &skill,
            "---\nname: product-owner\nmodel: gpt-5-nano\n---\n# PO skill\n",
        )
        .unwrap();

        // Caso 1: modelo de rol
        let toml1 = r#"
[agents]
provider = "pi"

[agents.product_owner]
model = "gpt-5"
"#;
        let cfg1: Config = toml::from_str(toml1).unwrap();
        assert_eq!(
            cfg1.agents.model_for_role("product_owner", &skill),
            "gpt-5",
            "Caso 1: modelo de rol debe usarse"
        );

        // Caso 2: modelo global (sin modelo de rol)
        let toml2 = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"
"#;
        let cfg2: Config = toml::from_str(toml2).unwrap();
        assert_eq!(
            cfg2.agents.model_for_role("product_owner", &skill),
            "claude-sonnet-4",
            "Caso 2: modelo global debe usarse cuando no hay de rol"
        );

        // Caso 3: YAML frontmatter (sin config)
        let cfg3 = AgentsConfig::default();
        assert_eq!(
            cfg3.model_for_role("product_owner", &skill),
            "gpt-5-nano",
            "Caso 3: YAML frontmatter debe usarse cuando no hay config"
        );

        // Caso 4: "desconocido" (nada definido)
        assert_eq!(
            cfg3.model_for_role("product_owner", Path::new("/no/existe.md")),
            "desconocido",
            "Caso 4: 'desconocido' cuando no hay modelo en ningún lado"
        );
    }
}
