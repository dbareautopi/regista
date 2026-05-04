//! Sistema de providers de agentes de codificación.
//!
//! Cada provider encapsula cómo invocar a un agente concreto (pi, Claude Code,
//! Codex, OpenCode) en modo no-interactivo. El trait devuelve `Vec<String>`
//! (args de CLI) en lugar de un `Command`, para ser compatible tanto con
//! ejecución síncrona como asíncrona (paralelismo #01).

use std::path::Path;

/// Un provider sabe cómo invocar a un agente de codificación concreto.
///
/// Devuelve `Vec<String>` (args de CLI), no un `Command`, para que el
/// invocador decida si usar `std::process::Command` (sync) o
/// `tokio::process::Command` (async, paralelismo).
pub trait AgentProvider {
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
        format!(".opencode/agents/{role}.md")
    }

    fn build_args(&self, instruction: &Path, prompt: &str) -> Vec<String> {
        // opencode usa subcomando "run" con mensaje posicional.
        // El nombre del agente se deriva del nombre del archivo de
        // instrucción (sin extensión): product_owner.md → product_owner.
        // --dangerously-skip-permissions: modo no-interactivo.
        let agent_name = instruction
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("build");

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
            let ps_cmd = format!(
                "opencode run --agent {agent_name} --dangerously-skip-permissions \"{escaped_prompt}\""
            );
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                ps_cmd,
            ]
        } else {
            vec![
                "run".to_string(),
                "--agent".to_string(),
                agent_name.to_string(),
                "--dangerously-skip-permissions".to_string(),
                prompt.to_string(),
            ]
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
        assert_eq!(
            p.instruction_dir("reviewer"),
            ".opencode/agents/reviewer.md"
        );
    }

    #[test]
    fn opencode_build_args_uses_run_with_agent() {
        let p = OpenCodeProvider;
        let args = p.build_args(
            Path::new(".opencode/agents/product_owner.md"),
            "refina esta historia",
        );
        assert_eq!(args[0], "run");
        assert!(args.contains(&"--agent".to_string()));
        assert!(args.contains(&"product_owner".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"refina esta historia".to_string()));
        // No debe contener -p ni -q ni -f (eran del API anterior)
        assert!(!args.contains(&"-p".to_string()));
        assert!(!args.contains(&"-q".to_string()));
        assert!(!args.contains(&"-f".to_string()));
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
