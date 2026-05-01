# 09 — Prompts agnósticos al stack

## 🎯 Objetivo

Eliminar las referencias hardcodeadas a herramientas específicas de un stack
(`cargo`, `npm`, `src/`) de los prompts, permitiendo que `regista` orqueste
proyectos en cualquier lenguaje sin modificar el código fuente.

## 📍 Posición en el roadmap

**Fase 3** — justo después del paralelismo (#01). En este punto providers (#20)
y ejecución async (#01) ya están estables. Los templates de prompt pueden
adaptarse sabiendo que se ejecutan sobre providers heterogéneos y en oleadas
paralelas.

## ❓ Problema actual

Los prompts en `prompts.rs` contienen referencias concretas a herramientas:

| Prompt | Referencia hardcodeada | Problema |
|--------|----------------------|----------|
| `reviewer()` | `cargo test, clippy, fmt` | Solo funciona en proyectos Rust |
| `qa_tests()` | `src/` para placeholders | Asume estructura de Rust/Python |
| `dev_implement()` | `build + tests` | Genérico pero vago |
| `dev_fix()` | `build + tests` | Ídem |

Si el proyecto anfitrión es Node.js, los agentes deberían ejecutar `npm test`,
`eslint`, etc. Si es Python: `pytest`, `ruff`. Hoy eso depende de que el skill
del agente *adivine* el stack, lo cual es frágil.

Además, la feature #04 (workflow configurable) requiere que los prompts sean
genéricos por rol, no por transición específica. Este es el primer paso para
desacoplar prompts de la máquina de estados fija.

## ✅ Solución propuesta

### Variables de stack en `.regista/config.toml`

```toml
[stack]
# Comandos que los agentes deben ejecutar para verificar su trabajo.
# Si no se definen, el agente usa su criterio (skill).
build_command = "npm run build"
test_command = "npm test"
lint_command = "eslint ."
fmt_command = "prettier --check ."
src_dir = "src/"
```

### Plantillas de prompt con placeholders

Los prompts pasan de tener herramientas hardcodeadas a usar placeholders
que se resuelven desde `StackConfig`:

```
Antes (Rust):
  "Ejecuta cargo test, clippy, fmt."

Después (agnóstico):
  "Ejecuta {test_command}, {lint_command}, {fmt_command}."
```

Si un comando no está definido, el placeholder se omite o se sustituye por
una instrucción genérica: `"verifica que el código compile y los tests pasen"`.

### Prompt genérico por rol (preparando #04)

Cada rol tiene una plantilla base que recibe:

| Variable | Fuente |
|----------|--------|
| `{role}` | Rol canónico (`product_owner`, `qa_engineer`, `developer`, `reviewer`) |
| `{story_id}` | ID de la historia |
| `{from}` → `{to}` | Transición esperada |
| `{action}` | Verbo: "refina", "testea", "implementa", "revisa", "valida" |
| `{stack_context}` | Bloque con los comandos del stack (build/test/lint/fmt) |
| `{rejection_context}` | Motivo de rechazo anterior, si existe |
| `{cross_story_context}` | Contexto de dependencias (#10), si está habilitado |

La función `build_prompt(role, ctx, stack)` sustituye las variables y devuelve
el prompt final. Los 7 métodos actuales (`po_groom()`, `qa_tests()`, etc.)
pasan a ser wrappers que llaman a `build_prompt` con los parámetros adecuados.

### Estructura propuesta de `prompts.rs` tras #09

```rust
pub struct StackConfig {
    pub build_command: Option<String>,
    pub test_command: Option<String>,
    pub lint_command: Option<String>,
    pub fmt_command: Option<String>,
    pub src_dir: Option<String>,
}

impl StackConfig {
    /// Renderiza el bloque de stack para inyectar en el prompt.
    /// Si no hay comandos definidos, devuelve instrucción genérica.
    pub fn render(&self) -> String { ... }
}

pub fn build_prompt(role: &str, ctx: &PromptContext, stack: &StackConfig) -> String {
    // Plantilla base con {placeholders}
    // Sustituye stack.render(), ctx.story_id, ctx.from, ctx.to, ctx.last_rejection
    // Añade siempre "NO preguntes. 100% autónomo."
}
```

### Comportamiento sin configuración

Si `[stack]` no existe en el TOML (retrocompatibilidad):

- `build_prompt` usa instrucciones genéricas: `"compila/construye el proyecto"`.
- El skill del agente es responsable de interpretar qué significa eso en el
  contexto del proyecto.
- Cero breaking change para proyectos existentes.

## 📝 Notas de implementación

### Archivos modificados

| Archivo | Cambio | Líneas |
|---------|--------|--------|
| `src/config.rs` | Nuevo struct `StackConfig` con `#[serde(default)]`; campo `stack` en `Config` | +30 |
| `src/prompts.rs` | `build_prompt()` genérico; `StackConfig::render()`; wrappers mantienen API pública | +80 |
| `src/orchestrator.rs` | Construir `StackConfig` desde `Config`; pasar a `process_story()` | +10 |
| Tests | Tests de render con/sin comandos; tests de placeholders | +40 |
| **Total** | | **~160 líneas** |

### Riesgos

- **Prompts demasiado genéricos**: si no hay `[stack]` definido, el prompt es
  vago (`"compila el proyecto"`). El skill del agente DEBE ser bueno. Si el
  agente no tiene contexto del stack, puede fallar. Solución: documentar que
  `[stack]` es recomendado, no obligatorio.
- **Regresiones en proyectos Rust**: los prompts actuales mencionan `cargo`
  explícitamente y funcionan bien. Al migrar a plantillas, hay que asegurar que
  el comportamiento por defecto sea al menos igual de bueno. Estrategia: el
  `StackConfig::default()` puede tener defaults razonables detectados del
  proyecto (`Cargo.toml` → Rust, `package.json` → Node).

### Auto-detección de stack (opcional, v2)

Como mejora futura, `regista` podría detectar el stack automáticamente:

```rust
fn detect_stack(project_root: &Path) -> StackConfig {
    if project_root.join("Cargo.toml").exists() {
        StackConfig {
            build_command: Some("cargo build".into()),
            test_command: Some("cargo test".into()),
            lint_command: Some("cargo clippy -- -D warnings".into()),
            fmt_command: Some("cargo fmt -- --check".into()),
            src_dir: Some("src/".into()),
        }
    } else if project_root.join("package.json").exists() {
        // ...
    } else {
        StackConfig::default()
    }
}
```

Esto reduce la fricción de adopción sin requerir configuración manual. Se
implementaría después de #09 como un quick win adicional.

## 🔗 Relacionado con

- [`04-workflow-configurable.md`](./04-workflow-configurable.md) — **cliente**.
  #04 necesita prompts genéricos por rol para funcionar con workflows arbitrarios.
- [`10-cross-story-context.md`](./10-cross-story-context.md) — el contexto
  cross-story se inyecta como una sección más del prompt (`{cross_story_context}`).
- [`01-paralelismo.md`](./01-paralelismo.md) — con paralelismo, el stack config
  es el mismo para todas las historias de una oleada (el proyecto no cambia de
  stack a mitad del pipeline).
- [`20-multi-provider.md`](./20-multi-provider.md) — cada provider recibe el
  mismo prompt. Si un provider necesita formato especial (system prompt vs
  user prompt), el trait `AgentProvider` puede extenderse con `build_system_prompt()`.
