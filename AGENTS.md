# AGENTS.md — regista

Guía para agentes de codificación que trabajen en este proyecto.  
Incluye arquitectura, convenciones, comandos, y decisiones de diseño.

---

## 📌 ¿Qué es esto?

`regista` es un **orquestador genérico de agentes** para [`pi`](https://github.com/mariozechner/pi-coding-agent).  
Automatiza un pipeline de desarrollo de software con 4 roles (PO, QA, Dev, Reviewer)  
gobernado por una **máquina de estados** formal con detección de deadlocks,
checkpoint/resume, y salida JSON para CI/CD.

**Filosofía clave**: regista **no sabe nada del proyecto** que orquesta.  
No importa si el proyecto usa Rust, Python, o cualquier cosa. Solo necesita:
1. Dónde están las historias de usuario (archivos `.md`)
2. Qué provider y qué instrucciones de rol usar para cada rol
3. La máquina de estados fija que gobierna las transiciones

---

## 🧱 Stack técnico

| Componente | Tecnología |
|------------|------------|
| Lenguaje | **Rust** (edition 2021) |
| CLI | **clap** 4 (derive) |
| Configuración | **TOML** (`serde` + `toml` 0.8) |
| JSON output | **serde_json** 1 |
| Logging | **tracing** + `tracing-subscriber` (env-filter) |
| Error handling | **anyhow** |
| Regex | **regex** (con `LazyLock`) |
| Fechas | **chrono** |
| Glob | **glob** 0.3 |
| Tests | `#[cfg(test)]` + `tempfile` (dev-dependency) |
| HTTP client | **ureq** 2 (json feature) |
| Build | `cargo` |

---

## 📁 Estructura del proyecto

```
regista/
├── AGENTS.md                  ← este archivo
├── README.md                  ← descripción general para usuarios
├── DESIGN.md                  ← diseño completo (máquina de estados, arquitectura, multi-provider)
├── HANDOFF.md                 ← handoff de la última sesión (lo implementado y pendiente)
├── Cargo.toml                 ← dependencias y metadata del crate
├── .gitignore
├── src/
│   ├── main.rs                ← CLI (clap Subcommand), 9 subcomandos, --version
│   ├── config.rs              ← Config, AgentsConfig + AgentRoleConfig, carga TOML
│   ├── state.rs               ← Status, Actor, Transition, can_transition_to(), ALL
│   ├── story.rs               ← Story, parseo de .md, set_status(), advance_status_in_memory()
│   ├── dependency_graph.rs    ← DependencyGraph, DFS para ciclos, blocks_count()
│   ├── deadlock.rs            ← analyze(), DeadlockResolution, priorización
│   ├── providers.rs           ← trait AgentProvider + Pi/ClaudeCode/Codex/OpenCode + factory
│   ├── agent.rs               ← invoke_with_retry(provider: &dyn AgentProvider, …), AgentOptions, feedback rico
│   ├── prompts.rs             ← PromptContext, 7 prompts stack-agnósticos, StackConfig::render()
│   ├── orchestrator.rs        ← run(), run_real(), run_dry(), process_story() con resolución de provider por rol
│   ├── checkpoint.rs          ← OrchestratorState: save/load/remove (.regista/state.toml)
│   ├── validator.rs           ← validate(): chequeo pre-vuelo multi-provider
│   ├── init.rs                ← init(): scaffolding multi-provider (pi, claude, codex, opencode)
│   ├── plan.rs               ← run(): generación de backlog (comando `plan`)
│   ├── hooks.rs               ← run_hook(), ejecución de comandos shell post-fase
│   ├── git.rs                 ← snapshot(), rollback(), init_git()
│   ├── daemon.rs              ← detach(), status(), kill(), follow(), DaemonState
│   └── update.rs              ← check() + run(): auto-update desde crates.io
├── roadmap/                   ← Documentos de diseño de features futuras
│   ├── ROADMAP.md             ← Índice con estado de cada feature y orden de implementación
│   ├── 01-paralelismo.md      ← Diseño detallado (Fase 7, último)
│   ├── 02-salida-json-ci-cd.md        ← ✅ IMPLEMENTADO
│   ├── 03-dry-run.md                  ← ✅ IMPLEMENTADO
│   ├── 04-workflow-configurable.md    ← Diseño detallado (Fase 5)
│   ├── 05-validate.md                 ← ✅ IMPLEMENTADO
│   ├── 06-init-scaffold.md            ← ✅ IMPLEMENTADO
│   ├── 07-checkpoint-resume.md        ← ✅ IMPLEMENTADO
│   ├── 08-feedback-agentes.md         ← ✅ IMPLEMENTADO
│   ├── 09-prompts-agnosticos.md       ← ✍️  Diseño definido (Fase 2)
│   ├── 10-cross-story-context.md      ← ✍️  Diseño definido (Fase 4)
│   ├── 13-groom-generacion-historias.md ← ✅ IMPLEMENTADO (comando `plan`)
│   ├── 14-groom-from-dir.md           ← Pendiente (`plan --from-dir`, Fase 3)
│   ├── 15-groom-interactive.md        ← Pendiente (`plan --interactive`, Fase 6)
│   ├── 20-multi-provider.md           ← ✅ IMPLEMENTADO
│   └── 20-implementacion.md           ← Detalle técnico de la implementación
└── tests/
    └── fixtures/
        ├── story_draft.md
        ├── story_blocked.md
        └── story_business_review.md
```

---

## ⚙️ Comandos esenciales

```bash
# Compilar (debug)
cargo build

# Compilar (release)
cargo build --release

# Ejecutar todos los tests
cargo test

# Ejecutar tests de un módulo específico
cargo test --lib state
cargo test --lib checkpoint
cargo test --lib plan
cargo test --lib providers

# Ejecutar test ignorado (requiere pi instalado)
cargo test -- --ignored

# Ver warnings
cargo check

# Formatear
cargo fmt

# Linting
cargo clippy -- -D warnings
```

---

## 🔄 Máquina de estados

### Diagrama del flujo feliz

```
Draft ──PO(plan)──→ Ready ──QA(tests)──→ Tests Ready ──Dev(implement)──→ In Review
                                                                                │
                                                                         Reviewer │
                                                                                ▼
                                Done ←──PO(validate)── Business Review
```

### Transiciones canónicas (inmutables, definidas en `state.rs`)

| # | De | A | Actor | Condición |
|---|---|---|---|---|
| 1 | `Draft` | `Ready` | **PO** | Historia cumple DoR |
| 2 | `Ready` | `Tests Ready` | **QA** | Tests escritos para todos los CAs |
| 3 | `Ready` | `Draft` | **QA** (rollback) | Historia no es testeable |
| 4 | `Tests Ready` | `In Review` | **Dev** | Implementación completa |
| 5 | `Tests Ready` | `Tests Ready` | **QA** (corregir) | Dev reporta tests rotos |
| 6 | `In Progress` | `In Review` | **Dev** (fix) | Corrección aplicada |
| 7 | `In Review` | `Business Review` | **Reviewer** | DoD técnico OK |
| 8 | `In Review` | `In Progress` | **Reviewer** | Rechazo técnico |
| 9 | `Business Review` | `Done` | **PO** (validate) | Validación de negocio OK |
| 10 | `Business Review` | `In Review` | **PO** | Rechazo leve |
| 11 | `Business Review` | `In Progress` | **PO** | Rechazo grave |
| 12 | `*` | `Blocked` | **Orchestrator** | Dependencias ≠ Done |
| 13 | `Blocked` | `Ready` | **Orchestrator** | Dependencias pasan a Done |
| 14 | `*` | `Failed` | **Orchestrator** | `max_reject_cycles` agotado |

> ⚠️ Las transiciones 12, 13, 14 son automáticas (sin agente).  
> Las transiciones son **inmutables** — no se añaden en runtime.

### Estados terminales

- `Done` — historia completada exitosamente
- `Failed` — superó `max_reject_cycles`

---

## 📝 Contrato de historia (.md)

Los archivos de historia deben seguir este formato **exacto**:

```markdown
# STORY-NNN: Título

## Status
**<Draft|Ready|Tests Ready|In Progress|In Review|Business Review|Done|Blocked|Failed>**

## Epic
EPIC-XXX

## Descripción
...

## Criterios de aceptación
- [ ] CA1: descripción
- [ ] CA2: ...

## Dependencias       ← opcional
- Bloqueado por: STORY-XXX, STORY-YYY

## Activity Log       ← obligatorio
- YYYY-MM-DD | PO | descripción
```

### Reglas de parseo (`story.rs`)

| Campo | Cómo se extrae |
|-------|---------------|
| **Status** | Busca `## Status` (case-insensitive), lee la línea siguiente, limpia `**...**` |
| **Bloqueadores** | Busca `Bloqueado por:` (case-insensitive) dentro de `## Dependencias`, extrae `STORY-\d+` |
| **Epic** | Busca `## Epic`, lee la línea siguiente, extrae `EPIC-\d+` |
| **Last rejection** | Busca `## Activity Log`, última línea que contiene "rechaz" (case-insensitive) |
| **Last actor** | Busca `## Activity Log`, última línea, extrae actor entre `|` |

---

## 🧩 Descripción de módulos

### `main.rs` — Entry point
- Define `Cli` con `#[derive(Parser)]` y `#[derive(Subcommand)]` de clap 4
- 9 subcomandos: `plan`, `auto`, `run`, `logs`, `status`, `kill`, `validate`, `init`, `update`
- `plan`/`auto`/`run` spawnean un daemon (proceso hijo); `logs`/`status`/`kill` lo gestionan
- `--version` y `--help` son nativos de clap
- Configura tracing (stderr para usuario, archivo de log para daemon)
- Exit codes: 0=OK, 1=error plan, 2=pipeline con Failed, 3=parada temprana
- Flag `--provider` para sobreescribir provider global desde CLI
- Flags `--dry-run`, `--quiet`, `--resume`, `--clean-state`, `--logs`, `--config`

### `config.rs` — Configuración
- `Config` con `#[serde(default)]`: todos los campos tienen defaults razonables
- Carga desde `.regista/config.toml`; si no existe, usa defaults
- `AgentsConfig`: `provider` global + `AgentRoleConfig` por rol (product_owner, qa_engineer, developer, reviewer)
- `AgentRoleConfig`: `provider` opcional (hereda del global) y `skill` opcional (usa convención del provider)
- `provider_for_role(role)`: resuelve el nombre del provider para un rol
- `skill_for_role(role)`: resuelve la ruta al archivo de instrucciones
- `all_roles()`: iterador de los 4 roles canónicos
- `LimitsConfig`: incluye `plan_max_iterations` (default 5), `inject_feedback_on_retry` (default true)
- `validate()`: verifica que `stories_dir` existe, crea `decisions_dir` y `log_dir`

### `state.rs` — Máquina de estados
- `Status` enum: 9 variantes
- `Actor` enum: `ProductOwner`, `QaEngineer`, `Developer`, `Reviewer`, `Orchestrator`
- `Transition` struct con `Status::ALL` — 14 transiciones canónicas
- Tests: 23 tests

### `story.rs` — Parseo de historias
- `Story` struct: `id`, `path`, `status`, `epic`, `blockers`, `last_rejection`, `raw_content`
- `load()`: lee archivo .md y parsea todos los campos
- `set_status()`: escribe a disco con backup atómico + verificación
- `advance_status_in_memory()`: muta estado sin tocar disco (dry-run)
- `last_actor()`: extrae último actor del Activity Log
- Tests: 12 tests

### `dependency_graph.rs` — Grafo de dependencias
- `DependencyGraph`: `forward`, `reverse`, DFS con colores
- `blocks_count()`, `has_cycle_from()`, `has_any_cycle()`, `find_cycle_members()`
- Tests: 4 tests

### `deadlock.rs` — Detección de bloqueos
- `DeadlockResolution` enum: `NoDeadlock`, `InvokePoFor`, `PipelineComplete`
- `analyze()`: algoritmo de 4 pasos con priorización
- Prioriza por: mayor `unblocks`, luego menor ID numérico
- Tests: 7 tests

### `providers.rs` — Sistema de providers
- `AgentProvider` trait: `binary()`, `build_args()`, `display_name()`, `instruction_name()`, `instruction_dir()`
- El trait devuelve `Vec<String>` (args de CLI), no `Command` — compatible con sync y async (paralelismo #01)
- **PiProvider**: `pi -p "..." --skill <path> --no-session`
- **ClaudeCodeProvider**: `claude -p "..." --append-system-prompt-file <path> --permission-mode bypassPermissions`
- **CodexProvider**: `codex exec --sandbox workspace-write "..."` (auto-descubre `.agents/skills/`)
- **OpenCodeProvider**: `opencode -p "..." -q`
- Factory `from_name(name)`: resuelve alias (claude-code, open-code, etc.), case-insensitive
- `supported_providers()`: lista `["pi", "claude", "codex", "opencode"]`
- Tests: 17 tests (factory, alias, case insensitivity, args verification para cada provider)

### `agent.rs` — Invocación de agentes con feedback
- `invoke_with_retry(provider: &dyn AgentProvider, …)`: loop con backoff exponencial, acepta `AgentOptions`
- Recibe el provider como trait object — sin hardcodear `pi`
- `AgentOptions`: `story_id`, `decisions_dir`, `inject_feedback`
- Feedback rico: guarda stdout/stderr en `decisions/`, inyecta errores en reintentos
- `AgentResult` incluye `attempt` y `attempts: Vec<AttemptTrace>`
- Tests: 3 tests + 1 ignorado

### `prompts.rs` — Generación de prompts
- `PromptContext`: `story_id`, `stories_dir`, `decisions_dir`, `last_rejection`, `from`, `to`, `stack: StackConfig`
- `StackConfig::render()`: renderiza bloque de comandos (build/test/lint/fmt) o instrucción genérica
- `PromptContext::header()` / `suffix()`: helpers privados para componer prompts
- 7 funciones de prompt (una por transición accionable por agentes), todas stack-agnósticas
- Los prompts de Dev (`dev_implement`, `dev_fix`) y Reviewer inyectan `stack.render()`
- `qa_tests()` usa `stack.src_dir` para placeholders (si está definido)
- Todos los prompts terminan con `"NO preguntes. 100% autónomo."`
- Tests: 15 tests

### `orchestrator.rs` — Loop principal
- `run()`: dispatch a `run_real()` o `run_dry()` según `options.dry_run`
- `run_real()`: loop con carga de historias, transiciones automáticas, deadlock, process_story
  - Acepta `resume_state: Option<OrchestratorState>` para `--resume`
  - Guarda checkpoint tras cada `process_story()` exitoso
  - Limpia checkpoint en `PipelineComplete`
- `run_dry()`: simulación en memoria sin agentes ni escritura a disco
  - Usa `advance_status_in_memory()` para mutar estados
  - Muestra desbloqueos y estima tiempo
- `process_story()`: determina rol → resuelve provider + instrucciones vía `AgentsConfig` → invoca agente con AgentOptions
- `map_status_to_role()`: mapea Status → rol canónico ("product_owner", "qa_engineer", "developer", "reviewer")
- `filter_stories()`: aplica filtros `--story`, `--epic`, `--epics`
- Tests: 18 tests

### `checkpoint.rs` — Persistencia del estado
- `OrchestratorState`: `iteration`, `reject_cycles`, `story_iterations`, `story_errors`
- `save()` / `load()` / `remove()` sobre `.regista/state.toml`
- `load()` maneja archivos corruptos (los borra)
- Tests: 5 tests

### `validator.rs` — Comando `validate`
- Valida: config, instrucciones de rol (multi-provider), historias (parseo, Activity Log), dependencias (refs rotas, ciclos), git
- Reconoce `AgentsConfig` con per-role providers — valida cada path de instrucción según el provider del rol
- `ValidationResult` con `Vec<Finding>` (severity: Error/Warning, category, message)
- `--json` para CI, exit codes: 0=OK, 1=errores, 2=warnings
- Tests: 3 tests

### `init.rs` — Comando `init`
- Genera `.regista/config.toml` con el provider especificado y estructura de directorios
- `init(project_dir, light, with_example, provider_name)` — provider_name determina dónde guardar instrucciones
- Directorios por provider:
  - `pi` → `.pi/skills/<rol>/SKILL.md`
  - `claude` → `.claude/agents/<rol>.md`
  - `codex` → `.agents/skills/<rol>/SKILL.md`
  - `opencode` → `.opencode/commands/<rol>.md`
- `--light`: solo config, sin instrucciones de rol
- `--with-example`: incluye historia y épica de ejemplo
- `--provider <NAME>`: elegir provider (default: pi)
- No pisa archivos existentes
- Tests: 7 tests

### `plan.rs` — Comando `plan`
- `run()`: invoca al PO para generar historias desde spec
- **Bucle de validación**: plan → validate dependencias → si errores → feedback al PO → repetir
- Máx iteraciones: `plan_max_iterations` (default 5)
- `--max-stories` (0 = sin límite), `--replace`
- Prompts: `plan_prompt_initial()`, `plan_prompt_fix()` con `PlanCtx`
- El módulo mantiene el nombre `plan.rs` por compatibilidad interna; el comando CLI es `plan`
- Tests: 6 tests

### `hooks.rs` — Hooks post-fase
- `run_hook()`: ejecuta `sh -c "<comando>"`, retorna error si exit code ≠ 0

### `git.rs` — Snapshots y rollback
- `snapshot()`: `git add -A && git commit -q -m "snapshot: {label}"`
- `rollback()`: `git reset --hard <hash>`
- Auto-inicializa repo si `git.enabled = true` y no existe

### `daemon.rs` — Modo daemon
- `detach()`: spawnea proceso hijo con `--daemon` interno
- `status()`, `kill()`, `follow()`: gestión del proceso
- Estado guardado en `.regista/daemon.pid` (TOML)
- Tests: 6 tests

### `update.rs` — Auto-update
- `check()`: consulta `https://crates.io/api/v1/crates/regista` vía `ureq`
- `run(auto_yes)`: compara versiones, pregunta confirmación, ejecuta `cargo install regista --version <latest>`
- `current_version()`: devuelve `env!("CARGO_PKG_VERSION")`
- Flag `--yes` para omitir prompt interactivo
- Tests: 2 tests

---

## 💡 Decisiones de diseño importantes

1. **Agnóstico al proyecto anfitrión**: regista no sabe de Rust, cargo, ni nada.  
   Solo invoca el provider configurado con prompts genéricos.

2. **Workflow fijo e inmutable**: las 14 transiciones son canónicas.  
   No se añaden transiciones en runtime **por diseño**.

3. **Trait `AgentProvider` devuelve `Vec<String>`**: no `Command`, para ser  
   compatible con ejecución síncrona y asíncrona (paralelismo #01).  
   El invocador decide si usar `std::process::Command` o `tokio::process::Command`.

4. **CLI con clap Subcommand**: la CLI usa `#[derive(Subcommand)]` de clap 4.
   Los subcomandos son `plan`, `auto`, `run`, `logs`, `status`, `kill`,
   `validate`, `init`, `update`. `--version` y `--help` son nativos de clap.
   Ya no se usa detección manual de args.

5. **Dry-run en memoria**: `advance_status_in_memory()` muta `Story` sin tocar el filesystem.  
   Permite simular pipelines completos sin gastar créditos de LLM.

6. **Checkpoint TOML**: el estado del orquestador se guarda en `.regista/state.toml`.  
   Si el pipeline se interrumpe, `--resume` lo reanuda. Se limpia en `PipelineComplete`.

7. **Feedback rico en reintentos**: cuando un agente falla, su stderr se guarda en  
   `decisions/` y se inyecta en el prompt del reintento. Truncado a 2000 bytes.

8. **Plan con bucle validate**: generar historias no basta — hay que validar que las  
   dependencias son correctas. El PO recibe feedback concreto y corrige en bucle.

9. **Salida JSON en validate**: `regista validate --json` emite JSON a stdout.
   El pipeline daemon escribe resultados en `.regista/daemon.log`.

10. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos (`delay *= 2`).

11. **`set_status()` con backup atómico**: escribe → re-parsea → si falla, restaura `.bak`.

12. **Provider por defecto `"pi"`**: retrocompatibilidad total. Si no se especifica  
    provider en config ni CLI, se usa pi. Projects existentes siguen funcionando sin cambios.

13. **Provider por rol**: cada rol (PO, QA, Dev, Reviewer) puede usar un provider distinto.  
    Ejemplo: PO con Claude Code, Dev con pi. Configurable en `.regista/config.toml`.

14. **Codex auto-descubre skills**: `CodexProvider` ignora el path de instrucciones —  
    Codex lee automáticamente `.agents/skills/` y `AGENTS.md` del proyecto.

---

## 🚧 Pendiente (roadmap)

### Features no implementadas

| # | Feature | Esfuerzo | Fase |
|---|---------|----------|------|
| 01 | Paralelismo | Alto | 7 (último) |
| 04 | Workflow configurable | Medio | 5 |
| 09 | Prompts agnósticos al stack | Bajo | 2 |
| 10 | Cross-story context | Medio | 4 |
| 11 | TUI / dashboard | Medio | 6 |
| 12 | Cost tracking | Medio | 6 |
| 14 | Plan `--from-dir` | Bajo | 3 |
| 15 | Plan `--interactive` | Medio | 6 |

---

## 🧪 Estrategia de testing

- **Tests unitarios**: cada módulo tiene `#[cfg(test)] mod tests` con fixtures inline
- **Fixtures**: `tests/fixtures/` contiene archivos .md de ejemplo para pruebas de parseo
- **Test ignorado**: `agent::tests::invoke_with_retry_fails_when_pi_not_installed` (requiere `pi` en PATH)
- **Total**: 145 tests pasando, 0 fallos, 1 ignorado

Para añadir tests:
- Usa `make_story()` o `story_fixture()` helpers para crear Stories sintéticas
- No dependas de archivos reales salvo en tests de `story.rs` (que usan fixtures)
- Para tests de providers, usa `from_name()` para obtener una instancia y verificar `build_args()`
- Para tests de nuevos módulos, usa `tempfile::tempdir()` para aislar el filesystem

---

## ⚠️ Errores comunes y anti-patrones

- ❌ **Añadir transiciones a `Status::ALL`**: rompe la inmutabilidad del workflow.  
  Las 14 transiciones son el contrato fijo.

- ❌ **Parsear historias sin usar `extract_section()`**: usa las funciones existentes en `story.rs`.

- ❌ **Modificar `raw_content` sin actualizar `status`**: deben estar siempre sincronizados.

- ❌ **Ejecutar hooks sin `sh -c`**: los hooks son comandos shell.

- ❌ **Asumir que todos los bloqueadores existen**: filtrar siempre con `status_map.get()`.

- ❌ **Usar `..ctx` (struct update) sin clonar**: `PromptContext` contiene Strings, el update  
  syntax los mueve. Si necesitas reutilizar `ctx`, clona los campos explícitamente.

- ❌ **Llamar a `invoke_with_retry` sin provider**: la firma requiere `&dyn AgentProvider` como  
  primer argumento. Usa `providers::from_name("pi")` si no necesitas un provider concreto.

- ❌ **Asumir que el provider es `pi`**: usa `AgentsConfig::provider_for_role(role)` para  
  resolver el provider correcto según la configuración del proyecto.

- ❌ **Hardcodear flags de provider**: usa `AgentProvider::build_args()` para construir los  
  argumentos de CLI. Cada provider tiene sus propios flags y subcomandos.

- ❌ **Usar `.regista.toml` como path de config**: el path correcto es `.regista/config.toml`  
  (dentro del directorio `.regista/`, no en la raíz).

- ❌ **Generar solo skills para pi en `init`**: el generador usa `AgentProvider::instruction_dir(role)`  
  para colocar las instrucciones en el directorio correcto según el provider.

---

## 🔑 Convenciones de código

- **Idioma**: código y comentarios en español, nombres de variables/funciones en inglés
- **Formato**: `cargo fmt` (rustfmt estándar)
- **Documentación**: `//!` para módulos, `///` para items públicos
- **Errores**: `anyhow::Result<T>` y `anyhow::bail!()` (nunca `unwrap()` en lógica de negocio)
- **Logging**: `tracing::info!()` / `warn!()` / `error!()` / `debug!()` (nunca `println!`)
- **Regex estáticos**: usa `LazyLock<Regex>` para compilar una sola vez
- **Defaults de serde**: `#[serde(default)]` + funciones `default_xxx()`
- **Tests**: usa `assert!()` / `assert_eq!()` con mensajes descriptivos
- **Nuevos módulos**: siguen el patrón `pub fn run(...) -> anyhow::Result<...>` para su entry point
