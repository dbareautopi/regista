//! Sistema de providers de agentes de codificación.
//!
//! Cada provider encapsula cómo invocar a un agente concreto (pi, Claude Code,
//! Codex, OpenCode) en modo no-interactivo. El trait devuelve `Vec<String>`
//! (args de CLI) en lugar de un `Command`, para ser compatible tanto con
//! ejecución síncrona como asíncrona (paralelismo #01).

use std::path::Path;

/// Lee un campo simple del YAML frontmatter de un archivo markdown.
///
/// Busca líneas como `campo: valor` en el bloque delimitado por `---`.
/// Devuelve `None` si no hay frontmatter o el campo no existe.
fn read_yaml_field(path: &Path, field: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_frontmatter = false;
    let mut count = 0u32;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            count += 1;
            if count == 1 {
                in_frontmatter = true;
                continue;
            } else if in_frontmatter {
                // Fin del frontmatter
                break;
            }
        }
        if in_frontmatter {
            if let Some(rest) = trimmed.strip_prefix(&format!("{field}:")) {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

/// Un provider sabe cómo invocar a un agente de codificación concreto.
///
/// Devuelve `Vec<String>` (args de CLI), no un `Command`, para que el
/// invocador decida si usar `std::process::Command` (sync) o
/// `tokio::process::Command` (async, paralelismo).
/// `Send + Sync` is required so that `&dyn AgentProvider` and
/// `Box<dyn AgentProvider>` can be held across `.await` points in
/// async functions (the resulting future must be `Send` for
/// multi-threaded tokio runtimes and potential `tokio::spawn` usage
/// in #01).
pub trait AgentProvider: Send + Sync {
    /// Binario a ejecutar: "pi", "claude", "codex", "opencode".
    fn binary(&self) -> &str;

    /// Argumentos completos de CLI para una invocación no-interactiva.
    ///
    /// `instruction_path` es la ruta al archivo de instrucciones de rol
    /// (skill, agent, command) ya resuelta. Algunos providers lo usan
    /// como flag (`--skill`), otros lo ignoran porque auto-descubren
    /// las instrucciones (Codex lee `.agents/skills/` automáticamente).
    fn build_args(&self, instruction_path: &Path, prompt: &str) -> Vec<String>;

    /// Nombre legible para logs: "pi", "Claude Code", "Codex", "OpenCode".
    fn display_name(&self) -> &str;

    /// Cómo se llama el concepto de "rol/personalidad" en este provider.
    /// pi → "skill", claude → "agent", codex → "skill", opencode → "command".
    #[allow(dead_code)]
    fn instruction_name(&self) -> &str;

    /// Directorio por convención donde se guardan las instrucciones de un rol.
    fn instruction_dir(&self, role: &str) -> String;

    /// Extensión de los archivos de instrucción.
    #[allow(dead_code)]
    fn instruction_extension(&self) -> &str {
        "md"
    }
}

// ── pi ────────────────────────────────────────────────────────────────

pub struct PiProvider;

impl AgentProvider for PiProvider {
    fn binary(&self) -> &str {
        "pi"
    }

    fn display_name(&self) -> &str {
        "pi"
    }

    fn instruction_name(&self) -> &str {
        "skill"
    }

    fn instruction_dir(&self, role: &str) -> String {
        // pi requires skill names to be lowercase a-z, 0-9, hyphens only.
        let dir_name = role.replace('_', "-");
        format!(".pi/skills/{dir_name}/SKILL.md")
    }

    fn build_args(&self, instruction: &Path, prompt: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "--skill".to_string(),
            instruction.to_string_lossy().to_string(),
            "--no-session".to_string(),
        ]
    }
}

// ── Claude Code ────────────────────────────────────────────────────────

pub struct ClaudeCodeProvider;

impl AgentProvider for ClaudeCodeProvider {
    fn binary(&self) -> &str {
        "claude"
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn instruction_name(&self) -> &str {
        "agent"
    }

    fn instruction_dir(&self, role: &str) -> String {
        format!(".claude/agents/{role}.md")
    }

    fn build_args(&self, instruction: &Path, prompt: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "--append-system-prompt-file".to_string(),
            instruction.to_string_lossy().to_string(),
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
        ]
    }
}

// ── Codex (OpenAI) ─────────────────────────────────────────────────────

pub struct CodexProvider;

impl AgentProvider for CodexProvider {
    fn binary(&self) -> &str {
        "codex"
    }

    fn display_name(&self) -> &str {
        "Codex"
    }

    fn instruction_name(&self) -> &str {
        "skill"
    }

    fn instruction_dir(&self, role: &str) -> String {
        // Codex usa el open agent skills standard.
        // Las skills viven en .agents/skills/ y se auto-descubren.
        format!(".agents/skills/{role}/SKILL.md")
    }

    fn build_args(&self, _instruction: &Path, prompt: &str) -> Vec<String> {
        // codex exec usa subcomando, no flag -p.
        // El prompt va como argumento posicional.
        // Las skills se auto-descubren de .agents/skills/.
        // AGENTS.md del proyecto también se lee automáticamente.
        vec![
            "exec".to_string(),
            "--sandbox".to_string(),
            "workspace-write".to_string(),
            prompt.to_string(),
        ]
    }
}

// ── OpenCode ───────────────────────────────────────────────────────────

pub struct OpenCodeProvider;

impl AgentProvider for OpenCodeProvider {
    /// En Windows, opencode se distribuye como un script PowerShell (.ps1)
    /// que no se puede invocar directamente con CreateProcess.
    /// Usamos powershell.exe como wrapper con -Command.
    fn binary(&self) -> &str {
        if cfg!(windows) {
            "powershell"
        } else {
            "opencode"
        }
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn instruction_name(&self) -> &str {
        "agent"
    }

    fn instruction_dir(&self, role: &str) -> String {
        // OpenCode lee agentes desde .opencode/agents/*.md.
        // El contenido del .md se usa como system prompt del agente.
        // Los guiones bajos se convierten a guiones para que coincidan
        // con el campo `name` del YAML frontmatter (opencode identifica
        // agentes por el YAML name, no por el nombre del archivo).
        let dir_name = role.replace('_', "-");
        format!(".opencode/agents/{dir_name}.md")
    }

    fn build_args(&self, instruction: &Path, prompt: &str) -> Vec<String> {
        // opencode usa subcomando "run" con mensaje posicional.
        // El nombre del agente se deriva del nombre del archivo de
        // instrucción (sin extensión): product-owner.md → product-owner.
        // --dangerously-skip-permissions: modo no-interactivo.
        let agent_name = instruction
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("build");

        // Leer el modelo del archivo de instrucción (campo `model:` en YAML frontmatter)
        let model = read_yaml_field(instruction, "model");

        if cfg!(windows) {
            // Windows: opencode es un .ps1 → necesita powershell wrapper.
            // El prompt va dentro de comillas dobles de PowerShell → escapar:
            //   " → ""  (convención PowerShell)
            //   ` → ``   (backtick: carácter de escape)
            //   $ → `$   (evita expansión de variables)
            let escaped_prompt = prompt
                .replace('`', "``")
                .replace('$', "`$")
                .replace('"', "\"\"");
            let model_flag = model
                .as_ref()
                .map(|m| format!(" -m {m}"))
                .unwrap_or_default();
            let ps_cmd = format!(
                "opencode run --agent {agent_name}{model_flag} --dangerously-skip-permissions \"{escaped_prompt}\""
            );
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                ps_cmd,
            ]
        } else {
            let mut args = vec![
                "run".to_string(),
                "--agent".to_string(),
                agent_name.to_string(),
            ];
            if let Some(ref m) = model {
                args.push("-m".to_string());
                args.push(m.clone());
            }
            args.push("--dangerously-skip-permissions".to_string());
            args.push(prompt.to_string());
            args
        }
    }
}

// ── Factory ────────────────────────────────────────────────────────────

/// Construye un provider a partir de su nombre.
///
/// Acepta nombres canónicos y alias comunes.
/// Lanza panic si el nombre no corresponde a ningún provider conocido.
pub fn from_name(name: &str) -> Box<dyn AgentProvider> {
    match name.to_lowercase().as_str() {
        "pi" => Box::new(PiProvider),
        "claude" | "claude-code" | "claude_code" => Box::new(ClaudeCodeProvider),
        "codex" => Box::new(CodexProvider),
        "opencode" | "open-code" | "open_code" => Box::new(OpenCodeProvider),
        other => panic!(
            "provider desconocido: '{other}'. Providers válidos: pi, claude, codex, opencode"
        ),
    }
}

/// Lista de nombres canónicos de providers soportados.
#[allow(dead_code)]
pub fn supported_providers() -> Vec<&'static str> {
    vec!["pi", "claude", "codex", "opencode"]
}

// ═══════════════════════════════════════════════════════════════════════════
// Resolver provider e instrucciones desde AgentsConfig
// ═══════════════════════════════════════════════════════════════════════════

/// Resuelve el nombre del provider para un rol dado desde la configuración.
///
/// Si el rol tiene `provider` explícito, lo usa.
/// Si no, hereda del provider global.
pub fn provider_for_role(agents: &crate::config::AgentsConfig, role: &str) -> String {
    let config = match role {
        "product_owner" => &agents.product_owner,
        "qa_engineer" => &agents.qa_engineer,
        "developer" => &agents.developer,
        "reviewer" => &agents.reviewer,
        _ => return agents.provider.clone(),
    };
    config
        .provider
        .clone()
        .unwrap_or_else(|| agents.provider.clone())
}

/// Resuelve la ruta al archivo de instrucciones para un rol dado.
///
/// Si el rol tiene `skill` explícito, lo usa.
/// Si no, usa la convención de directorio del provider.
pub fn skill_for_role(agents: &crate::config::AgentsConfig, role: &str) -> String {
    let config = match role {
        "product_owner" => &agents.product_owner,
        "qa_engineer" => &agents.qa_engineer,
        "developer" => &agents.developer,
        "reviewer" => &agents.reviewer,
        _ => return String::new(),
    };

    if let Some(ref skill) = config.skill {
        return skill.clone();
    }

    let provider_name = provider_for_role(agents, role);
    let provider = from_name(&provider_name);
    provider.instruction_dir(role)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Factory ──────────────────────────────────────────────────────

    #[test]
    fn from_name_returns_pi() {
        let p = from_name("pi");
        assert_eq!(p.binary(), "pi");
        assert_eq!(p.display_name(), "pi");
        assert_eq!(p.instruction_name(), "skill");
    }

    #[test]
    fn from_name_returns_claude() {
        let p = from_name("claude");
        assert_eq!(p.binary(), "claude");
        assert_eq!(p.display_name(), "Claude Code");
        assert_eq!(p.instruction_name(), "agent");
    }

    #[test]
    fn from_name_aliases_claude() {
        for alias in &["claude-code", "claude_code"] {
            let p = from_name(alias);
            assert_eq!(
                p.binary(),
                "claude",
                "alias '{alias}' debería resolver a claude"
            );
        }
    }

    #[test]
    fn from_name_returns_codex() {
        let p = from_name("codex");
        assert_eq!(p.binary(), "codex");
        assert_eq!(p.display_name(), "Codex");
    }

    #[test]
    fn from_name_returns_opencode() {
        let p = from_name("opencode");
        assert_eq!(p.binary(), "opencode");
        assert_eq!(p.display_name(), "OpenCode");
    }

    #[test]
    fn from_name_aliases_opencode() {
        for alias in &["open-code", "open_code"] {
            let p = from_name(alias);
            assert_eq!(
                p.binary(),
                "opencode",
                "alias '{alias}' debería resolver a opencode"
            );
        }
    }

    #[test]
    #[should_panic(expected = "provider desconocido")]
    fn from_name_panics_on_unknown() {
        from_name("chatgpt");
    }

    #[test]
    fn from_name_is_case_insensitive() {
        let p = from_name("CLAUDE");
        assert_eq!(p.binary(), "claude");
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-001: from_name() devuelve Result
    // ═══════════════════════════════════════════════════════════════

    /// CA1: from_name("pi") devuelve Ok(Box<dyn AgentProvider>)
    /// (mismo comportamiento, distinto tipo de retorno).
    #[test]
    fn from_name_returns_ok_for_known_provider() {
        let result = from_name("pi");
        assert!(
            result.is_ok(),
            "from_name(\"pi\") debería devolver Ok, no Err ni paniquear"
        );
        let provider = result.unwrap();
        assert_eq!(provider.binary(), "pi");
        assert_eq!(provider.display_name(), "pi");
        assert_eq!(provider.instruction_name(), "skill");
    }

    /// CA1: from_name para cada provider canónico devuelve Ok.
    #[test]
    fn from_name_returns_ok_for_all_canonical_providers() {
        for name in &["pi", "claude", "codex", "opencode"] {
            let result = from_name(name);
            assert!(
                result.is_ok(),
                "from_name(\"{name}\") debería devolver Ok"
            );
        }
    }

    /// CA2: from_name("inventado") devuelve Err con mensaje descriptivo,
    /// sin hacer panic.
    #[test]
    fn from_name_returns_err_for_unknown_provider() {
        let result = from_name("inventado");
        assert!(
            result.is_err(),
            "from_name(\"inventado\") debería devolver Err, NO paniquear"
        );
    }

    /// CA2: el mensaje de error debe ser descriptivo:
    /// mencionar el nombre del provider y sugerir alternativas.
    #[test]
    fn from_name_err_message_is_descriptive() {
        let result = from_name("inventado");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("inventado"),
            "El mensaje de error debe mencionar el provider desconocido: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("provider"),
            "El mensaje debe ser descriptivo e indicar que es un error de provider: {msg}"
        );
    }

    /// CA2: Varios nombres inválidos deben devolver Err, no panic.
    #[test]
    fn from_name_returns_err_for_various_unknown_names() {
        for invalid in &["chatgpt", "copilot", "cursor", "", "unknown-agent"] {
            let result = from_name(invalid);
            assert!(
                result.is_err(),
                "from_name(\"{invalid}\") debería devolver Err"
            );
        }
    }

    /// CA3: Aliases de Claude ("claude-code", "claude_code", "claude")
    /// siguen funcionando con el nuevo Result.
    #[test]
    fn from_name_claude_aliases_return_ok() {
        for alias in &["claude", "claude-code", "claude_code"] {
            let result = from_name(alias);
            assert!(
                result.is_ok(),
                "Alias '{alias}' debería devolver Ok"
            );
            let provider = result.unwrap();
            assert_eq!(
                provider.binary(),
                "claude",
                "Alias '{alias}' debería resolver a claude"
            );
            assert_eq!(provider.display_name(), "Claude Code");
        }
    }

    /// CA4: Aliases de OpenCode ("opencode", "open-code", "open_code")
    /// siguen funcionando con el nuevo Result.
    #[test]
    fn from_name_opencode_aliases_return_ok() {
        for alias in &["opencode", "open-code", "open_code"] {
            let result = from_name(alias);
            assert!(
                result.is_ok(),
                "Alias '{alias}' debería devolver Ok"
            );
            let provider = result.unwrap();
            assert_eq!(
                provider.binary(),
                "opencode",
                "Alias '{alias}' debería resolver a opencode"
            );
            assert_eq!(provider.display_name(), "OpenCode");
        }
    }

    /// CA5: El Result de from_name se puede propagar con el operador `?`.
    /// Esto verifica que el tipo de retorno es compatible con anyhow.
    #[test]
    fn from_name_result_works_with_question_mark_operator() {
        fn try_get_provider(name: &str) -> anyhow::Result<Box<dyn AgentProvider>> {
            Ok(from_name(name)?)
        }

        assert!(try_get_provider("pi").is_ok());
        assert!(try_get_provider("claude").is_ok());
        assert!(try_get_provider("inventado").is_err());
    }

    /// CA5: El Result de from_name se puede manejar con match exhaustivo.
    #[test]
    fn from_name_result_handled_with_match() {
        let binary = match from_name("codex") {
            Ok(p) => p.binary().to_string(),
            Err(e) => {
                panic!("No debería fallar con 'codex': {e}");
            }
        };
        assert_eq!(binary, "codex");

        let desc = match from_name("inventado") {
            Ok(_) => "ok".to_string(),
            Err(e) => e.to_string(),
        };
        assert!(desc.contains("inventado"));
    }

    /// CA5: skill_for_role usa internamente from_name y debe manejar el Result.
    /// Con un provider válido, skill_for_role no debe paniquear.
    #[test]
    fn skill_for_role_uses_result_from_from_name() {
        let cfg = crate::config::Config::default(); // provider = "pi"
        let path = skill_for_role(&cfg.agents, "developer");
        assert_eq!(path, ".pi/skills/developer/SKILL.md");

        let path_po = skill_for_role(&cfg.agents, "product_owner");
        assert_eq!(path_po, ".pi/skills/product-owner/SKILL.md");
    }

    /// CA5: skill_for_role con provider no-pi también funciona.
    #[test]
    fn skill_for_role_works_with_claude_provider() {
        let toml = r#"
[agents]
provider = "claude"
"#;
        let cfg: crate::config::Config = toml::from_str(toml).unwrap();
        let path = skill_for_role(&cfg.agents, "developer");
        assert_eq!(path, ".claude/agents/developer.md");
    }

    // ── pi ───────────────────────────────────────────────────────────

    #[test]
    fn pi_instruction_dir() {
        let p = PiProvider;
        // Underscores are converted to hyphens for pi compatibility
        assert_eq!(
            p.instruction_dir("product_owner"),
            ".pi/skills/product-owner/SKILL.md"
        );
        assert_eq!(
            p.instruction_dir("qa_engineer"),
            ".pi/skills/qa-engineer/SKILL.md"
        );
        // developer and reviewer have no underscores, unchanged
        assert_eq!(
            p.instruction_dir("developer"),
            ".pi/skills/developer/SKILL.md"
        );
    }

    #[test]
    fn pi_build_args() {
        let p = PiProvider;
        let args = p.build_args(Path::new("skills/po/SKILL.md"), "haz esto");
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"haz esto".to_string()));
        assert!(args.contains(&"--skill".to_string()));
        assert!(args.contains(&"skills/po/SKILL.md".to_string()));
        assert!(args.contains(&"--no-session".to_string()));
    }

    // ── Claude Code ──────────────────────────────────────────────────

    #[test]
    fn claude_instruction_dir() {
        let p = ClaudeCodeProvider;
        assert_eq!(
            p.instruction_dir("developer"),
            ".claude/agents/developer.md"
        );
    }

    #[test]
    fn claude_build_args_includes_bypass_permissions() {
        let p = ClaudeCodeProvider;
        let args = p.build_args(Path::new(".claude/agents/po.md"), "revisa esto");
        assert!(args.contains(&"bypassPermissions".to_string()));
        assert!(args.contains(&"--append-system-prompt-file".to_string()));
        assert!(args.contains(&"-p".to_string()));
    }

    // ── Codex ────────────────────────────────────────────────────────

    #[test]
    fn codex_instruction_dir() {
        let p = CodexProvider;
        assert_eq!(
            p.instruction_dir("product_owner"),
            ".agents/skills/product_owner/SKILL.md"
        );
    }

    #[test]
    fn codex_build_args_uses_exec_subcommand() {
        let p = CodexProvider;
        let args = p.build_args(Path::new("ignored"), "mi tarea");
        assert_eq!(args[0], "exec");
        assert!(args.contains(&"--sandbox".to_string()));
        assert!(args.contains(&"workspace-write".to_string()));
        assert!(args.contains(&"mi tarea".to_string()));
        // No debe contener -p (Codex usa subcomando)
        assert!(!args.contains(&"-p".to_string()));
    }

    // ── OpenCode ─────────────────────────────────────────────────────

    #[test]
    fn opencode_instruction_dir() {
        let p = OpenCodeProvider;
        // reviewer has no underscores, unchanged
        assert_eq!(
            p.instruction_dir("reviewer"),
            ".opencode/agents/reviewer.md"
        );
        // product_owner: underscores → hyphens (must match YAML name)
        assert_eq!(
            p.instruction_dir("product_owner"),
            ".opencode/agents/product-owner.md"
        );
        // qa_engineer: underscores → hyphens
        assert_eq!(
            p.instruction_dir("qa_engineer"),
            ".opencode/agents/qa-engineer.md"
        );
    }

    #[test]
    fn opencode_build_args_uses_run_with_agent() {
        let p = OpenCodeProvider;
        // Path with hyphens (matching instruction_dir output)
        let args = p.build_args(
            Path::new(".opencode/agents/product-owner.md"),
            "refina esta historia",
        );
        assert_eq!(args[0], "run");
        assert!(args.contains(&"--agent".to_string()));
        assert!(args.contains(&"product-owner".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"refina esta historia".to_string()));
        // No debe contener -p ni -q ni -f (eran del API anterior)
        assert!(!args.contains(&"-p".to_string()));
        assert!(!args.contains(&"-q".to_string()));
        assert!(!args.contains(&"-f".to_string()));
        // Sin modelo en el archivo → no se pasa -m
        assert!(!args.contains(&"-m".to_string()));
    }

    #[test]
    fn opencode_build_args_includes_model_when_present_in_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let agent_file = tmp.path().join("product-owner.md");
        std::fs::write(
            &agent_file,
            "---\nname: product-owner\nmodel: opencode/minimax-m2.5-free\n---\n# test",
        )
        .unwrap();

        let p = OpenCodeProvider;
        let args = p.build_args(&agent_file, "test prompt");
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"opencode/minimax-m2.5-free".to_string()));
        assert!(args.contains(&"test prompt".to_string()));
    }

    #[test]
    fn read_yaml_field_extracts_value() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.md");
        std::fs::write(
            &file,
            "---\nname: my-agent\nmodel: opencode/gpt-5-nano\ndescription: test agent\n---\n# Body",
        )
        .unwrap();

        assert_eq!(
            read_yaml_field(&file, "model"),
            Some("opencode/gpt-5-nano".into())
        );
        assert_eq!(read_yaml_field(&file, "name"), Some("my-agent".into()));
        assert_eq!(
            read_yaml_field(&file, "description"),
            Some("test agent".into())
        );
        assert_eq!(read_yaml_field(&file, "nonexistent"), None);
    }

    #[test]
    fn read_yaml_field_returns_none_without_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("no-frontmatter.md");
        std::fs::write(&file, "# Just a markdown file\n\nNo YAML here.").unwrap();

        assert_eq!(read_yaml_field(&file, "model"), None);
    }

    #[test]
    fn read_yaml_field_returns_none_for_missing_file() {
        assert_eq!(
            read_yaml_field(Path::new("/nonexistent/file.md"), "model"),
            None
        );
    }

    // ── supported_providers ──────────────────────────────────────────

    #[test]
    fn supported_providers_includes_all_four() {
        let names = supported_providers();
        assert_eq!(names.len(), 4);
        assert!(names.contains(&"pi"));
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"codex"));
        assert!(names.contains(&"opencode"));
    }
}
