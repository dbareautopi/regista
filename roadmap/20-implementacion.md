# 🧬 Diseño de implementación — Multi-provider (#20)

> **Fase 1 del roadmap** · Rama: `develop` · Fecha: mayo 2026

---

## 1. Arquitectura general

```
┌─────────────────────────────────────────────────────────┐
│                    orchestrator.rs                       │
│  process_story(story, cfg)                               │
│    │                                                     │
│    ├─ resolve_provider(cfg, story.status) → dyn Provider │
│    ├─ resolve_instruction_path(cfg, provider, rol)       │
│    └─ agent::invoke_with_retry(provider, instruction, …) │
│                                                          │
│                    agent.rs                              │
│  invoke_with_retry(provider, instruction, prompt, …)     │
│    └─ invoke_once(provider, instruction, prompt)         │
│         └─ let args = provider.build_args(…)             │
│            Command::new(provider.binary())               │
│              .args(args).output()                        │
│                                                          │
│                    providers.rs (NUEVO)                   │
│  trait AgentProvider                                     │
│  ├─ PiProvider                                           │
│  ├─ ClaudeCodeProvider                                   │
│  ├─ CodexProvider                                        │
│  └─ OpenCodeProvider                                     │
│  + fn from_name(name: &str) -> Box<dyn AgentProvider>    │
└─────────────────────────────────────────────────────────┘
```

---

## 2. El trait `AgentProvider`

```rust
/// Un provider encapsula cómo invocar a un agente de codificación concreto.
///
/// Devuelve `Vec<String>` (args de CLI), no un `Command`, para ser compatible
/// tanto con ejecución síncrona (`std::process::Command`) como asíncrona
/// (`tokio::process::Command` en #01 — paralelismo).
pub trait AgentProvider {
    /// Binario a ejecutar: "pi", "claude", "codex", "opencode".
    fn binary(&self) -> &str;

    /// Argumentos completos de CLI para una invocación no-interactiva.
    ///
    /// El provider decide si usar subcomando (codex exec) o flag (-p).
    /// `instruction_path` es la ruta al archivo de instrucciones de rol
    /// (skill, agent, etc.) ya resuelta a una ruta absoluta o relativa al
    /// directorio de trabajo.
    fn build_args(&self, instruction_path: &Path, prompt: &str) -> Vec<String>;

    /// Nombre legible para logs: "pi", "Claude Code", "Codex", "OpenCode".
    fn display_name(&self) -> &str;

    /// Cómo se llama el concepto de "rol/personalidad" en este provider:
    /// pi → "skill", claude → "agent", codex → "skill", opencode → "instruction".
    fn instruction_name(&self) -> &str;

    /// Directorio por convención donde se guardan las instrucciones de un rol.
    /// Ejemplo: `.pi/skills/product-owner/SKILL.md`
    fn instruction_dir(&self, role: &str) -> String;

    /// Extensión de los archivos de instrucción (default: "md").
    fn instruction_extension(&self) -> &str {
        "md"
    }
}
```

### ¿Por qué `Vec<String>` y no `Command`?

| Si el trait devuelve… | Problema |
|---|---|
| `std::process::Command` | Incompatible con `tokio::process::Command` (paralelismo #01) |
| `tokio::process::Command` | Obliga a tokio incluso en modo secuencial |
| **`Vec<String>`** ✅ | El invocador construye el `Command` que necesite. Una sola implementación para sync y async. |

---

## 3. Implementaciones concretas

### 3.1 PiProvider

```rust
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
        format!(".pi/skills/{role}/SKILL.md")
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
```

### 3.2 ClaudeCodeProvider

```rust
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
```

**Nota**: `--append-system-prompt-file` añade las instrucciones de rol **sin** reemplazar el system prompt por defecto de Claude Code. Esto conserva sus capacidades de coding mientras le damos el rol de regista. `--permission-mode bypassPermissions` evita prompts interactivos en modo no-interactivo (necesario para CI/CD).

### 3.3 CodexProvider

```rust
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
        format!(".codex/skills/{role}.md")
    }

    fn build_args(&self, _instruction: &Path, prompt: &str) -> Vec<String> {
        // Codex usa subcomando "exec", no flag -p.
        // El prompt va como argumento posicional después de las flags.
        // Para v1, las instrucciones de rol no se pasan explícitamente;
        // Codex leerá AGENTS.md del proyecto automáticamente.
        vec![
            "exec".to_string(),
            "--sandbox".to_string(),
            "workspace-write".to_string(),
            prompt.to_string(),
        ]
    }
}
```

**Nota**: Codex CLI usa `codex exec "<tarea>"` (subcomando, no flag). Las instrucciones de rol para v1 se gestionan vía `AGENTS.md` en la raíz del proyecto, que Codex lee automáticamente. En v2 se puede explorar `--settings` o `--config` para pasar config adicional.

### 3.4 OpenCodeProvider

```rust
pub struct OpenCodeProvider;

impl AgentProvider for OpenCodeProvider {
    fn binary(&self) -> &str {
        "opencode"
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn instruction_name(&self) -> &str {
        "instruction"
    }

    fn instruction_dir(&self, role: &str) -> String {
        format!(".opencode/instructions/{role}.md")
    }

    fn build_args(&self, _instruction: &Path, prompt: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "-q".to_string(), // sin spinner, más limpio en logs
        ]
    }
}
```

**Nota**: OpenCode no tiene un mecanismo nativo de "system prompt file" como Claude o "skill" como pi. Para v1, las instrucciones de rol se pueden precargar en la config de OpenCode (custom commands) o simplemente documentarse como referencia. La flag `-q` suprime el spinner, ideal para ejecución automatizada.

### 3.5 Factory

```rust
/// Construye un provider a partir de su nombre.
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
```

---

## 4. Configuración (`.regista/config.toml`)

### 4.1 Formato limpio (sin retrocompatibilidad)

```toml
[agents]
# Provider por defecto para todos los roles.
# Si no se especifica, se usa "pi".
provider = "claude"

# Opcional: sobreescribir provider y/o skill por rol específico.
[agents.product_owner]
provider = "claude"              # hereda del default, se puede omitir
# skill = ".claude/agents/po-custom.md"  # opcional: path explícito

[agents.developer]
provider = "pi"                  # dev usa pi aunque el default sea claude
# skill = ".pi/skills/developer/SKILL.md"  # opcional
```

### 4.2 Structs en Rust

```rust
/// Configuración de un rol específico.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentRoleConfig {
    /// Nombre del provider ("pi", "claude", "codex", "opencode").
    /// Si no se especifica, hereda de `AgentsConfig::provider`.
    pub provider: Option<String>,

    /// Ruta explícita al archivo de instrucciones.
    /// Si no se especifica, se usa `provider.instruction_dir(role)`.
    pub skill: Option<String>,
}

impl Default for AgentRoleConfig {
    fn default() -> Self {
        Self {
            provider: None,
            skill: None,
        }
    }
}

/// Configuración global de agentes.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Provider por defecto para todos los roles.
    #[serde(default = "default_provider")]
    pub provider: String,

    pub product_owner: AgentRoleConfig,
    pub qa_engineer: AgentRoleConfig,
    pub developer: AgentRoleConfig,
    pub reviewer: AgentRoleConfig,
}

fn default_provider() -> String {
    "pi".to_string()
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

impl AgentsConfig {
    /// Resuelve el nombre del provider para un rol dado.
    /// Si el rol tiene provider explícito, lo usa.
    /// Si no, hereda del provider global.
    pub fn provider_for_role(&self, role: &str) -> String {
        let role_config = match role {
            "product_owner" => &self.product_owner,
            "qa_engineer" => &self.qa_engineer,
            "developer" => &self.developer,
            "reviewer" => &self.reviewer,
            _ => return self.provider.clone(),
        };
        role_config
            .provider
            .clone()
            .unwrap_or_else(|| self.provider.clone())
    }

    /// Resuelve la ruta al archivo de instrucciones para un rol dado.
    /// Si el rol tiene skill explícito, lo usa.
    /// Si no, usa la convención del provider.
    pub fn skill_for_role(&self, role: &str) -> String {
        let role_config = match role {
            "product_owner" => &self.product_owner,
            "qa_engineer" => &self.qa_engineer,
            "developer" => &self.developer,
            "reviewer" => &self.reviewer,
            _ => return String::new(),
        };
        role_config.skill.clone().unwrap_or_else(|| {
            let provider_name = self.provider_for_role(role);
            let provider = providers::from_name(&provider_name);
            provider.instruction_dir(role)
        })
    }
}
```

---

## 5. Cambios en `agent.rs`

```rust
use crate::providers::AgentProvider;

fn invoke_once(
    provider: &dyn AgentProvider,
    instruction: &Path,
    prompt: &str,
    _timeout: Duration,
) -> anyhow::Result<Output> {
    let args = provider.build_args(instruction, prompt);
    let result = std::process::Command::new(provider.binary())
        .args(&args)
        .output();

    match result {
        Ok(output) => Ok(output),
        Err(e) => {
            anyhow::bail!(
                "no se pudo ejecutar '{}': {e}. ¿Está instalado?",
                provider.binary()
            );
        }
    }
}
```

`invoke_with_retry` también recibe `&dyn AgentProvider` como primer argumento.

`save_agent_decision` usa `provider.display_name()` en vez de derivarlo del path del skill.

---

## 6. Cambios en `orchestrator.rs`

### 6.1 Función `map_status_to_role`

```rust
fn map_status_to_role(status: Status) -> &'static str {
    match status {
        Status::Draft | Status::BusinessReview => "product_owner",
        Status::Ready => "qa_engineer",
        Status::TestsReady => "developer",    // o "qa_engineer" si el último actor fue Dev
        Status::InProgress => "developer",
        Status::InReview => "reviewer",
        _ => "product_owner", // fallback seguro
    }
}
```

### 6.2 Cambios en `process_story`

```rust
fn process_story(
    story: &Story,
    project_root: &Path,
    cfg: &Config,
    reject_cycles: &mut HashMap<String, u32>,
    agent_opts: &AgentOptions,
) -> anyhow::Result<()> {
    // ... ctx, prompt (sin cambios) ...

    let role = map_status_to_role(story.status);
    let provider = providers::from_name(&cfg.agents.provider_for_role(role));
    let skill_path_str = cfg.agents.skill_for_role(role);
    let skill_path = project_root.join(&skill_path_str);

    tracing::info!(
        "  🎯 {} ({}) | {} ({} → {})",
        provider.display_name(),
        provider.instruction_name(),
        story.id,
        story.status,
        ctx.to
    );

    let result = agent::invoke_with_retry(
        provider.as_ref(),
        &skill_path,
        &prompt,
        &cfg.limits,
        agent_opts,
    );
    // ... resto igual ...
}
```

---

## 7. Cambios en `init.rs`

`regista init --provider <name>` genera los archivos en el directorio del provider:

```rust
pub fn init(project_root: &Path, light: bool, with_example: bool, provider_name: &str) -> Result {
    let provider = providers::from_name(provider_name);

    // Crear config con el provider
    let config_content = format!(
        r#"[agents]
provider = "{provider_name}"
"#
    );

    // Crear archivos de instrucciones para cada rol
    let roles = ["product_owner", "qa_engineer", "developer", "reviewer"];
    for role in &roles {
        let instruction_path = project_root.join(provider.instruction_dir(role));
        // Crear directorio y archivo .md con template según el rol
    }
}
```

---

## 8. Flag `--provider` en CLI

```bash
regista --provider claude                     # pipeline con Claude Code
regista --provider codex --dry-run            # simular con Codex
regista init --provider opencode              # init para OpenCode
regista groom spec.md --provider pi           # groom con pi
regista validate --provider claude            # validar proyecto con config de Claude
```

En `main.rs`:

```rust
#[arg(long, default_value = "pi")]
pub provider: String,
```

El valor se inyecta en `Config` o se usa como override del `agents.provider` del TOML.

**Comportamiento**: si se pasa `--provider`, **sobreescribe** el `provider` global del TOML. Los roles que tengan `provider` explícito en el TOML no se ven afectados.

---

## 9. Estructura de directorios generada

```
mi-proyecto/
├── .regista/
│   ├── config.toml           ← agents.provider = "claude"
│   ├── stories/              ← historias (vacío al init)
│   ├── epics/
│   ├── decisions/
│   └── logs/
│
├── .pi/skills/               ← si se usa provider=pi
│   ├── product-owner/SKILL.md
│   ├── qa-engineer/SKILL.md
│   ├── developer/SKILL.md
│   └── reviewer/SKILL.md
│
├── .claude/agents/           ← si se usa provider=claude
│   ├── product_owner.md
│   ├── qa_engineer.md
│   ├── developer.md
│   └── reviewer.md
│
├── .codex/skills/            ← si se usa provider=codex
│   ├── product_owner.md
│   ├── qa_engineer.md
│   ├── developer.md
│   └── reviewer.md
│
└── .opencode/instructions/   ← si se usa provider=opencode
    ├── product_owner.md
    ├── qa_engineer.md
    ├── developer.md
    └── reviewer.md
```

---

## 10. Orden de implementación

| Paso | Archivo | Acción | Líneas ~ |
|------|---------|--------|----------|
| 1 | `src/providers.rs` | **NUEVO**: trait `AgentProvider` + 4 implementaciones + factory | 130 |
| 2 | `src/config.rs` | Rediseñar `AgentsConfig` sin retrocompatibilidad | 60 |
| 3 | `src/agent.rs` | `invoke_once` + `invoke_with_retry` aceptan `&dyn AgentProvider` | 30 |
| 4 | `src/orchestrator.rs` | `process_story` resuelve provider por rol | 25 |
| 5 | `src/init.rs` | Aceptar `provider_name`, generar instrucciones en dir correcto | 40 |
| 6 | `src/main.rs` | Flag `--provider` + pasarlo a `Config` e `init` | 15 |
| 7 | Tests | Tests unitarios de providers + parseo config + integración | 80 |
| **Total** | | | **~380 líneas** |

---

## 11. Plan de testing

1. **Unitarios de providers**: verificar que `build_args` produce los vectores esperados para cada provider
2. **Parseo de config**: TOML mínimo (solo `provider`), TOML completo (con `skill` explícito por rol), TOML mixto
3. **Resolución de provider**: `provider_for_role` hereda correctamente, `skill_for_role` usa path explícito o convención
4. **Factory**: `from_name` con nombres canónicos y alias
5. **Integración**: test ignorado que invoca `pi --version` real (requiere pi instalado)

---

## 12. Riesgos y mitigaciones

| Riesgo | Mitigación |
|--------|------------|
| Claude Code CLI cambia sus flags | Cambio de 5 líneas en `ClaudeCodeProvider::build_args` |
| Codex CLI añade flag para system prompt | Añadir `--system-prompt-file` al `build_args` de Codex |
| OpenCode añade flag para custom instructions | Añadir soporte en `build_args` |
| Skills no portables entre providers | Documentar que cada provider necesita sus propias instrucciones |
| `regista init` muy acoplado a pi | Refactorizado: genera según provider |
