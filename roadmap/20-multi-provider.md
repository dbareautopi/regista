# 20 — Multi-provider: Claude Code, Aider, y otros

## 🎯 Objetivo

Eliminar la dependencia dura de `pi` como único agente de codificación.
Permitir que regista invoque **cualquier** CLI de agente (Claude Code, Aider,
Codex, etc.) mediante un sistema de providers configurable en `.regista/config.toml`.

## 📍 Posición en el roadmap

**Fase 1** — la primera feature a implementar. Es la **abstracción fundacional**
sobre la que se construye todo lo demás (paralelismo, workflow configurable, etc.).

## ❓ Problema actual

`agent.rs:invoke_once()` hardcodea:

```rust
std::process::Command::new("pi")
    .arg("-p").arg(prompt)
    .arg("--skill").arg(skill_path)
    .arg("--no-session")
```

Esto crea **vendor lock-in**:
- Si `pi` deja de mantenerse o no funciona en el SO del equipo, regista muere con él
- Claude Code está ganando tracción masiva y muchos equipos ya lo usan
- Cada agente tiene su propia CLI con flags distintos
- No hay razón técnica para atarse a un solo runtime de agente

## ✅ Solución propuesta

### Abstracción de providers

El trait `AgentProvider` es **agnóstico al modelo de concurrencia**:
devuelve `Vec<String>` (argumentos de CLI), no un `Command`. Esto permite
que el invocador decida si usar `std::process::Command` (sync) o
`tokio::process::Command` (async, para paralelismo en #01).

```rust
/// Un provider sabe cómo invocar a un agente de codificación concreto.
///
/// El trait devuelve los argumentos de CLI, no el Command directamente,
/// para ser compatible tanto con ejecución síncrona como asíncrona.
pub trait AgentProvider {
    /// Binario a ejecutar ("pi", "claude", "aider").
    fn binary(&self) -> &str;

    /// Construye los argumentos de CLI para una invocación.
    /// Ejemplo para pi: ["-p", "<prompt>", "--skill", "<path>", "--no-session"]
    fn build_args(&self, skill: &Path, prompt: &str) -> Vec<String>;

    /// Nombre legible para logs ("pi", "Claude Code", "Aider").
    fn display_name(&self) -> &str;

    /// Extensión de archivos de skill que usa este provider.
    /// Por defecto "md", pero algunos providers pueden usar otros formatos.
    fn skill_extension(&self) -> &str {
        "md"
    }
}
```

### ¿Por qué `Vec<String>` y no `Command`?

| Si el trait devuelve... | Problema |
|---|---|
| `std::process::Command` | No funciona con paralelismo async (#01). Habría que duplicar métodos. |
| `tokio::process::Command` | Obliga a usar tokio incluso en modo secuencial. |
| **`Vec<String>`** ✅ | El invocador construye el `Command` que necesite (sync o async). Una sola implementación. |

### Providers built-in

| Provider | Binario | Args de prompt | Args de skill | Notas |
|----------|---------|----------------|---------------|-------|
| `pi` | `pi` | `-p "<prompt>"` | `--skill <file> --no-session` | Default, compatibilidad total |
| `claude` | `claude` | `-p "<prompt>"` | `--system-prompt-file <file>` | Claude Code CLI |
| `aider` | `aider` | `--message "<prompt>"` | `--read <file>` | Modo no-interactivo |

### Configuración

```toml
# .regista/config.toml

[agents]
provider = "claude"                    # default global (si no se especifica, "pi")

[agents.product_owner]
provider = "claude"
skill = ".claude/skills/po.md"

[agents.qa_engineer]
provider = "claude"
skill = ".claude/skills/qa.md"

[agents.developer]
provider = "pi"                       # puedes mezclar providers por rol
skill = ".pi/skills/dev/SKILL.md"

[agents.reviewer]
provider = "pi"
skill = ".pi/skills/rev/SKILL.md"
```

O en formato simplificado cuando todos usan el mismo:

```toml
[agents]
provider = "claude"                   # todos los roles usan Claude Code
```

### Resolución de paths de skill por provider

Cada provider busca skills en su propio directorio por convención:

| Provider | Directorio de skills |
|----------|---------------------|
| `pi` | `.pi/skills/<rol>/SKILL.md` |
| `claude` | `.claude/skills/<rol>/SKILL.md` |
| `aider` | `.aider/skills/<rol>/SKILL.md` |

Si el usuario especifica `skill` explícitamente, se usa esa ruta (relativa al
proyecto). Si no, se usa el path por convención del provider.

### Flujo de invocación con providers

```
orchestrator::process_story(story, cfg)
  │
  ├─ 1. Determinar el rol necesario (PO, QA, Dev, Reviewer) según story.status
  │
  ├─ 2. Resolver provider desde cfg.agents para ese rol
  │      let provider = providers::from_name("claude");
  │
  ├─ 3. Obtener el skill path (explícito o por convención)
  │      let skill_path = provider.resolve_skill(cfg, role);
  │
  ├─ 4. Construir prompt (igual que ahora, prompt según story.status)
  │
  ├─ 5. Construir args desde el provider
  │      let args = provider.build_args(&skill_path, &prompt);
  │
  └─ 6. Ejecutar (sync ahora, async en #01)
         std::process::Command::new(provider.binary())
             .args(args)
             .output()
```

## 📝 Notas de implementación

### Archivos modificados

| Archivo | Cambio | Líneas |
|---------|--------|--------|
| `src/providers.rs` | **NUEVO**: trait `AgentProvider`, `PiProvider`, `ClaudeCodeProvider`, `AiderProvider`, factory `from_name()` | ~110 |
| `src/agent.rs` | `invoke_once` usa `dyn AgentProvider` en vez de hardcodear `pi` | ~25 |
| `src/config.rs` | Nuevo struct `AgentRoleConfig` { provider, skill }; `AgentsConfig` acepta formato plano (legado) y nuevo | ~50 |
| `src/orchestrator.rs` | `process_story` resuelve provider por rol; `skill_path` desde config o convención | ~20 |
| `src/main.rs` | Sin cambios | 0 |
| Tests | Parseo de config legacy + nuevo, tests de cada provider | ~50 |
| **Total** | | **~255 líneas** |

### Breaking changes: NINGUNO

- Si no se especifica `provider`, se usa `"pi"` (default actual)
- Si solo se especifica `product_owner = ".pi/skills/po/SKILL.md"` (formato plano), se interpreta como `{ provider = "pi", skill = "..." }`
- La CLI no cambia
- Los projects existentes siguen funcionando sin modificar su `.regista/config.toml`

### Estructura de `AgentRoleConfig`

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AgentRoleConfig {
    /// Formato legacy: solo path del skill (implícitamente provider = "pi")
    Legacy(String),
    /// Formato nuevo: provider + skill opcional
    Full {
        #[serde(default = "default_provider")]
        provider: String,
        #[serde(default)]
        skill: Option<String>,
    },
}
```

Con `#[serde(untagged)]`, serde intenta primero `Full` y luego `Legacy`.
Esto permite que el TOML soporte ambos formatos simultáneamente.

## 🔗 Relacionado con

- [`01-paralelismo.md`](./01-paralelismo.md) — **siguiente feature**. El
  paralelismo se construye sobre el trait `AgentProvider`. Como el trait
  devuelve `Vec<String>`, la transición a async es trivial.
- [`04-workflow-configurable.md`](./04-workflow-configurable.md) — el provider
  es parte de la config de cada rol en el workflow custom.
- [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) — necesario para
  que los prompts funcionen con cualquier provider/stack.

## ⚠️ Riesgos

- **Skills no portables**: un SKILL.md escrito para pi no funcionará en Claude Code.
  El comando `regista init` deberá generar skills para el provider elegido.
  Para v1, el usuario es responsable de escribir skills compatibles con su provider.
- **Diferencias de comportamiento**: cada agente tiene su propio "estilo".
  El mismo prompt puede dar resultados distintos en pi vs Claude Code.
  Esto es inherente a tener múltiples providers y es aceptable.
- **Claude Code CLI puede cambiar**: la CLI de Claude Code está en desarrollo activo.
  Los flags (`--system-prompt-file`, `-p`) pueden cambiar. El provider se actualiza
  en consecuencia (es un cambio de ~5 líneas).
