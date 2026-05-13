# AGENTS.md — regista

Guía para agentes de codificación que trabajen en este proyecto.  
Incluye arquitectura, convenciones, comandos, y decisiones de diseño.

> ⚠️ **Rama actual**: `rework` (refactor v0.10.0). La versión estable está en `main` (v0.9.5).

---

## 📌 ¿Qué es esto?

`regista` es un **orquestador genérico de agentes** para [`pi`](https://github.com/mariozechner/pi-coding-agent), [Claude Code](https://github.com/anthropics/claude-code), [Codex CLI](https://github.com/openai/codex), y [OpenCode](https://github.com/anomalyco/opencode).  
Automatiza un pipeline de desarrollo de software con 4 roles (PO, QA, Dev, Reviewer)  
gobernado por una **máquina de estados** formal con detección de deadlocks,
checkpoint/resume, y salida JSON para CI/CD.

**Filosofía clave**: regista **no sabe nada del proyecto** que orquesta.  
No importa si el proyecto usa Rust, Python, o cualquier cosa. Solo necesita:
1. Dónde están las historias de usuario (archivos `.md`)
2. Qué provider y qué instrucciones de rol usar para cada rol
3. La máquina de estados fija que gobierna las transiciones

**Rama `rework` (v0.10.0)**: El proyecto está en pleno refactor hacia una versión
más genérica. Los cambios principales son:
- Unificación de `plan`/`auto`/`run` en `run --plan-only`
- `Task` genérico con `task_format` configurable (reemplaza al `Story` hardcodeado)
- Templates de prompts con `{{variables}}` (reemplaza los 7 prompts fijos)
- Cliente LLM nativo en `infra/llm/` (OpenAI, Anthropic)
- Épicas como campo de task, no como archivos separados
- Eliminación de la dependencia conceptual con `spartito`

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
| Async runtime | **tokio** (rt-multi-thread, process, time, fs) |
| Regex | **regex** (con `LazyLock`) |
| Fechas | **chrono** |
| Glob | **glob** 0.3 |
| Tests | `#[cfg(test)]` + `tempfile` (dev-dependency) |
| HTTP client | **ureq** 2 (json feature) |
| Build | `cargo` |

---

## 📁 Estructura del proyecto

La arquitectura sigue un diseño por **capas** con dependencias unidireccionales
verificadas automáticamente por `tests/architecture.rs` (18 tests):

```
regista/
├── AGENTS.md                  ← este archivo
├── README.md                  ← descripción general para usuarios
├── Cargo.toml                 ← dependencias y metadata del crate
├── CHANGELOG.md               ← historial de versiones (Keep a Changelog)
├── .gitignore
├── src/
│   ├── main.rs                ← entry point: mod app, cli, config, domain, infra
│   ├── config.rs              ← Config, AgentsConfig, StackConfig, ModelConfig, carga TOML
│   │
│   ├── cli/                   ← 🟢 Capa CLI: args + handlers (puede importar cualquier capa)
│   │   ├── mod.rs
│   │   ├── args.rs            ← Cli, Commands (Plan/Auto/Run/Logs/Status/Kill/Validate/Init/Update/Board)
│   │   └── handlers.rs        ← dispatch(), handlers, daemon, exit codes
│   │
│   ├── app/                   ← 🟡 Capa Aplicación: casos de uso (importa domain + infra + config)
│   │   ├── mod.rs
│   │   ├── board.rs           ← Dashboard Kanban: conteo por estado, bloqueadas/fallidas, --json
│   │   ├── health.rs          ← HealthReport: métricas (iteraciones/hora, coste, tasa rechazo)
│   │   ├── init.rs            ← Scaffolding multi-provider + instrucciones de rol
│   │   ├── pipeline.rs        ← Loop principal: run(), run_real() (async), run_dry()
│   │   ├── plan.rs            ← Generación de backlog desde spec + bucle plan→validate
│   │   ├── update.rs          ← Auto-update desde crates.io
│   │   └── validate.rs        ← Chequeo pre-vuelo: config, skills, providers, historias, git
│   │
│   ├── domain/                ← 🔴 Capa Dominio: lógica pura (NO importa otras capas del crate)
│   │   ├── mod.rs
│   │   ├── state.rs           ← Status (enum con 9 variantes), Actor (enum), Transition, SharedState
│   │   ├── story.rs           ← Story, parseo .md, set_status() atómico, advance_status_in_memory()
│   │   ├── graph.rs           ← DependencyGraph, DFS para ciclos, blocks_count()
│   │   ├── deadlock.rs        ← analyze() (v0.x con Story) + analyze_deadlock() (v0.10.0 con Task)
│   │   ├── prompts.rs         ← PromptContext, DomainStackConfig, 7 prompts stack-agnósticos (v0.x)
│   │   ├── workflow.rs        ← Trait Workflow + CanonicalWorkflow + ConfigurableWorkflow (v0.10.0)
│   │   ├── task.rs            ← [NUEVO v0.10.0] Task, TaskFormatConfig, parseo configurable de .md
│   │   └── templates.rs       ← [NUEVO v0.10.0] render_template() con {{variables}} para prompts
│   │
│   └── infra/                 ← 🔵 Capa Infraestructura: I/O, procesos, git (importa solo config)
│       ├── mod.rs
│       ├── providers.rs       ← trait AgentProvider + Pi/ClaudeCode/Codex/OpenCode + factory
│       ├── agent.rs           ← invoke_with_retry() async, backoff, feedback rico, tokio runtime
│       ├── checkpoint.rs      ← OrchestratorState: save/load/remove (.regista/state.toml)
│       ├── daemon.rs          ← detach(), status(), kill(), follow(), PidCleanup
│       ├── git.rs             ← snapshot(), rollback() con spawn_blocking
│       ├── hooks.rs           ← run_hook(): comandos shell post-fase con tokio::process::Command
│       └── llm/               ← [NUEVO v0.10.0] Cliente LLM nativo (sin tools CLI externas)
│           ├── mod.rs         ← Trait LlmProvider + factory from_config()
│           ├── types.rs       ← Message, ChatResponse, TokenUsage
│           ├── openai.rs      ← OpenAiProvider (API Chat Completions)
│           ├── anthropic.rs   ← AnthropicProvider (Messages API)
│           ├── rate_limiter.rs
│           └── retry.rs
│
├── tests/
│   ├── architecture.rs        ← 18 tests: verifica que las capas respetan R1-R5
│   └── fixtures/
│       ├── story_draft.md
│       ├── story_blocked.md
│       └── story_business_review.md
│
├── docs/
│   ├── refactor-plan.md
│   └── architecture.md
│
├── specs/                     ← especificaciones usadas para desarrollar regista
│   ├── 01-cli-subcomandos-daemon.md
│   └── spec-logs-transparentes.md
│
└── roadmap/                   ← plan de refactor v0.10.0
    ├── epics/                 ← 6 epics (EPIC-V10-01 a EPIC-V10-06)
    ├── stories/               ← 24 stories (STORY-V10-001 a STORY-V10-024)
    └── features/              ← 14+ archivos Gherkin .feature
```

---

## ⚙️ Comandos esenciales

```bash
# Compilar (debug)
cargo build

# Compilar (release)
cargo build --release

# Ejecutar todos los tests (unitarios + arquitectura)
cargo test

# Ejecutar tests de un módulo específico
cargo test --lib domain::state
cargo test --lib domain::workflow
cargo test --lib domain::task
cargo test --lib domain::templates
cargo test --lib infra::providers
cargo test --lib infra::llm
cargo test --lib app::pipeline

# Ejecutar test ignorado (requiere pi instalado)
cargo test -- --ignored

# Ver warnings
cargo check

# Formatear
cargo fmt

# Linting (20 warnings actualmente — código v0.10.0 no cableado)
cargo clippy -- -D warnings

# Tests de arquitectura (verifica R1-R5)
cargo test --test architecture
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

### Transiciones canónicas (inmutables, definidas en `domain/workflow.rs`)

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

### `Status` como enum (v0.x) y `Workflow` trait

`Status` está definido como un enum en `domain/state.rs` con 9 variantes
(`Draft`, `Ready`, `TestsReady`, `InProgress`, `InReview`, `BusinessReview`,
`Done`, `Blocked`, `Failed`). El trait `Workflow` en `domain/workflow.rs`
abstrae la lógica de transiciones:

```rust
pub trait Workflow: Sync {
    fn next_status(&self, current: Status) -> Status;
    fn map_status_to_role(&self, status: Status) -> &'static str;
    fn canonical_column_order(&self) -> &[&'static str];
}
```

`CanonicalWorkflow` implementa las 14 transiciones fijas. En v0.10.0,
`ConfigurableWorkflow` permite definir workflows arbitrarios desde
`.regista/config.toml`.

---

## 📝 Contrato de historia (.md) — v0.x

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

### Reglas de parseo (`domain/story.rs`)

| Campo | Cómo se extrae |
|-------|---------------|
| **Status** | Busca `## Status` (case-insensitive), lee la línea siguiente, limpia `**...**` |
| **Bloqueadores** | Busca `Bloqueado por:` (case-insensitive) dentro de `## Dependencias`, extrae `STORY-\d+` |
| **Epic** | Busca `## Epic`, lee la línea siguiente, extrae `EPIC-\d+` |
| **Last rejection** | Busca `## Activity Log`, última línea que contiene "rechaz" (case-insensitive) |
| **Last actor** | Busca `## Activity Log`, última línea, extrae actor entre `|` |

### Contrato de tarea (.md) — v0.10.0 (nuevo, no cableado aún)

El formato v0.10.0 es configurable vía `TaskFormatConfig` (`domain/task.rs`).
Cada campo y su marcador de sección se define en TOML:

```toml
[workflow.task_format]
id_pattern = "TASK-\\d+"
section_markers = { status = "## Status", description = "## Descripción" }
dependency_marker = "Bloqueado por:"
```

`Task::load()` extrae el ID del nombre de archivo usando `id_pattern`, parsea
campos según `section_markers`, dependencias según `dependency_marker`, y el
Activity Log de forma estándar.

---

## 🧩 Descripción de módulos

### Capa `cli/` — Interfaz de usuario (puede importar cualquier capa)

#### `args.rs` — Definición de CLI
- `Cli` con `#[derive(Parser)]`, 10 subcomandos vía `Commands` enum
- `PlanArgs`, `AutoArgs`, `RunArgs`, `ValidateArgs`, `InitArgs`, `UpdateArgs`, `BoardArgs`
- `--version` y `--help` nativos de clap
- **En v0.10.0**: `plan` y `auto` se unifican en `run --plan-only`

#### `handlers.rs` — Dispatch y handlers
- `dispatch(cli)`: enruta cada subcomando a su handler
- `handle_plan()`, `handle_auto()`, `handle_run()`: daemon / dry-run / sync dispatch
- `handle_logs()`, `handle_status()`, `handle_kill()`: gestión del daemon
- `handle_validate()`, `handle_init()`, `handle_update()`, `handle_board()`
- Exit codes: 0=OK, 1=error plan, 2=pipeline con Failed, 3=parada temprana

### Capa `app/` — Casos de uso (importa domain + infra + config)

#### `pipeline.rs` — Loop principal del orquestador
- `run()`: dispatch a `run_real()` (async) o `run_dry()` (sync) según `options.dry_run`
- `run_real()`: async, loop con carga de historias, transiciones automáticas, deadlock, `process_story().await`
  - Acepta `resume_state: Option<OrchestratorState>` para `--resume`
  - Guarda checkpoint tras cada `process_story()` exitoso
  - Limpia checkpoint en `PipelineComplete`
  - `effective_max_iterations()`: auto-escala con `nº historias × 6`
- `run_dry()`: simulación en memoria sin agentes ni escritura a disco
- `process_story()`: async, determina rol → resuelve provider + instrucciones → invoca agente
- `RunReport`: serializable a JSON con `StoryRecord` por historia

#### `plan.rs` — Generación de backlog
- `run()`: invoca al PO para descomponer spec en historias y épicas
- Bucle de validación: plan → validate dependencias → feedback al PO → corregir
- `--max-stories` (0 = sin límite), `--replace`
- **En v0.10.0**: absorbido por `run --plan-only`

#### `board.rs` — Dashboard Kanban
- `BoardData`: conteo por estado, `BlockedStory`, `FailedStory`
- `render_board()`: acepta `&dyn Workflow` para orden dinámico de columnas
- `--json` para CI/CD, `--epic` para filtrar

#### `health.rs` — Health & Metrics
- `HealthReport`: métricas agregadas (iteraciones/hora, tiempo medio, tasa rechazo, coste)
- `is_health_checkpoint()`: cada N iteraciones (default: 10)
- `write_health_json()`: escritura atómica a `.regista/health.json`

#### `validate.rs` — Chequeo pre-vuelo
- Valida: config, skills multi-provider, providers en PATH, historias, dependencias, git
- `ValidationResult` con `Vec<Finding>` (severity: Error/Warning)
- `--json` para CI, exit codes: 0=OK, 1=errores, 2=warnings

#### `init.rs` — Scaffolding
- `init(project_dir, light, with_example, provider_name)`: genera `.regista/config.toml` + instrucciones
- Directorios por provider: `pi`→`.pi/skills/`, `claude`→`.claude/agents/`, `codex`→`.agents/skills/`, `opencode`→`.opencode/agents/`
- No pisa archivos existentes

#### `update.rs` — Auto-update
- `check()`: consulta crates.io vía `ureq`, compara versiones semánticas
- `run(auto_yes)`: instala con `cargo install regista --version <latest>`

### Capa `domain/` — Lógica pura (NO importa otras capas del crate)

#### `state.rs` — Tipos de la máquina de estados
- `Status`: enum con 9 variantes (`Draft`, `Ready`, `TestsReady`, `InProgress`, `InReview`, `BusinessReview`, `Done`, `Blocked`, `Failed`)
- `Actor`: enum con 5 variantes (`ProductOwner`, `QaEngineer`, `Developer`, `Reviewer`, `Orchestrator`)
- `Transition`: struct con `from`, `to`, `actor`, y guard opcional
- `SharedState`: `Arc<RwLock<HashMap<>>>` para estado compartido entre tareas
  - `reject_cycles`, `story_iterations`, `story_errors`
  - `Clone` comparte el mismo `Arc`; `read()`/`write()` con `RwLock`

#### `story.rs` — Parseo de historias (v0.x, hardcodeado)
- `Story` struct: `id`, `path`, `status: Status`, `epic`, `blockers`, `last_rejection`, `raw_content`
- `load()`: lee archivo .md y parsea todos los campos
- `set_status()`: escribe a disco con backup atómico (.bak)
- `advance_status_in_memory()`: muta estado sin tocar disco (dry-run)
- `last_actor()`: extrae último actor del Activity Log
- El parser de IDs (STORY-NNN, EPIC-NNN) usa regex compilado con `LazyLock`

#### `task.rs` — [NUEVO v0.10.0, no cableado] Parseo genérico de tareas
- `Task` struct: `id`, `path`, `fields: HashMap<String, String>`, `blockers`, `activity_log: Vec<ActivityLogEntry>`, `raw_content`
- `TaskFormatConfig`: `id_pattern`, `section_markers`, `dependency_marker` — todo configurable
- `load()`: parsea cualquier formato .md definido en `TaskFormatConfig`
- `set_status()`: escribe campo genérico (no solo "status") con backup atómico
- `last_rejection()`, `last_actor()`: equivalentes a `Story`
- Reemplazará a `Story` cuando `pipeline.rs` se migre a `Task`

#### `templates.rs` — [NUEVO v0.10.0, no cableado] Templates de prompts
- `render_template(template, task, context)`: sustituye `{{variables}}`
- Variables soportadas: `{{task_id}}`, `{{task_status}}`, `{{task_fields.<campo>}}`, `{{task_fields.*}}`, `{{last_rejection}}`, `{{blockers}}`, `{{context.<clave>}}`, `{{role_name}}`
- Variables no encontradas → `"(no definido)"`
- Reemplazará los 7 prompts hardcodeados de `prompts.rs`

#### `graph.rs` — Grafo de dependencias
- `DependencyGraph`: `forward` (bloqueador→bloqueados), `reverse`, DFS con colores
- `blocks_count()`, `has_cycle_from()`, `has_any_cycle()`, `find_cycle_members()`

#### `deadlock.rs` — Detección de bloqueos
- `DeadlockResolution` enum: `NoDeadlock`, `InvokePoFor`, `PipelineComplete`
- `analyze()`: algoritmo para `Story` (v0.x) con priorización por desbloqueo
- `DeadlockResolutionV10` + `analyze_deadlock()`: versión genérica para `Task` (v0.10.0, no cableada)
- Prioriza por: mayor `unblocks`, luego menor ID numérico

#### `prompts.rs` — Generación de prompts (v0.x)
- `PromptContext`: `story_id`, `stories_dir`, `decisions_dir`, `last_rejection`, `from`, `to`, `stack: DomainStackConfig`
- `DomainStackConfig::render()`: bloque de comandos o instrucción genérica
- 7 prompts: `po_plan()`, `po_validate()`, `qa_tests()`, `qa_fix_tests()`, `dev_implement()`, `dev_fix()`, `reviewer()`
- **En v0.10.0**: reemplazado por `templates.rs` + templates en `.regista/prompts/`

#### `workflow.rs` — Trait Workflow + implementaciones
- `Workflow` trait: `next_status()`, `map_status_to_role()`, `canonical_column_order()`
- `CanonicalWorkflow`: implementa las 14 transiciones fijas
- `ConfigurableWorkflow`: [v0.10.0] workflow definido en `.regista/config.toml` con fases, roles, bifurcaciones y guards configurables
- **Nota**: A diferencia de lo que indicaban versiones anteriores de AGENTS.md, `workflow.rs` NO está eliminado. Todo el código de workflow vive aquí (no hay dependencia externa `spartito`).

### Capa `infra/` — Infraestructura (importa solo `config`)

#### `providers.rs` — Sistema de providers (tools CLI externas)
- `AgentProvider` trait: `binary()`, `build_args()`, `display_name()`, `instruction_name()`, `instruction_dir()`
- El trait devuelve `Vec<String>` (no `Command`) — compatible con sync y async
- **PiProvider**: `pi -p "..." --skill <path> --no-session`
- **ClaudeCodeProvider**: `claude -p "..." --append-system-prompt-file <path> --permission-mode bypassPermissions`
- **CodexProvider**: `codex exec --sandbox workspace-write "..."` (auto-descubre `.agents/skills/`)
- **OpenCodeProvider**: `opencode run --agent <name> --dangerously-skip-permissions "..."`
- Factory `from_name(name)`: resuelve alias, case-insensitive, retorna `Result`

#### `agent.rs` — Invocación de agentes (async)
- `invoke_with_retry()`: async, loop con backoff exponencial (`delay *= 2`), timeout real con `tokio::time::timeout`
- `invoke_once()`: async, usa `tokio::process::Command`, mata proceso por PID en timeout
- `invoke_with_retry_blocking()`: wrapper síncrono con `RUNTIME.block_on()`
- `build_feedback_prompt()`: inyecta stderr truncado (2000 bytes) en reintentos
- `save_agent_decision()`: async, guarda trazas en `decisions/`
- `RUNTIME`: `LazyLock<tokio::runtime::Runtime>` global para callers síncronos

#### `checkpoint.rs` — Persistencia del estado
- `OrchestratorState`: `iteration`, `reject_cycles`, `story_iterations`, `story_errors`
- `save()` / `load()` / `remove()` sobre `.regista/state.toml`
- `load()` maneja archivos corruptos (los borra)

#### `daemon.rs` — Modo daemon
- `detach()`: spawnea proceso hijo con `--daemon` interno, guarda PID en `.regista/daemon.pid`
- `status()`, `kill()`, `follow()`: gestión del proceso
- `PidCleanup`: guard RAII que limpia el archivo PID al salir
- `get_all_child_pids()`: recursivo vía `/proc` (Linux) o `wmic` (Windows)
- ⚠️ 3 tests fallan en macOS (API `/proc/pid/task/*/children` es solo Linux)

#### `git.rs` — Snapshots y rollback
- `snapshot()`: `git add -A && git commit -q -m "snapshot: {label}"`, auto-inicializa repo
- `rollback()`: `git reset --hard <hash>`
- `check_git_changes()`: detecta cambios unstaged, staged, y untracked
- Usa `spawn_blocking` para seguridad async

#### `hooks.rs` — Hooks post-fase
- `run_hook()`: ejecuta `sh -c "<comando>"` con `tokio::process::Command`

#### `llm/` — [NUEVO v0.10.0, no cableado] Cliente LLM nativo
- `LlmProvider` trait: `chat(messages, model, timeout)` + `provider_name()`
- `from_config()`: factory que instancia el provider correcto desde `ModelConfig`
- `OpenAiProvider`: llama a la API Chat Completions (soporta proxies y Ollama)
- `AnthropicProvider`: llama a la Messages API (conversión de formato system/user/assistant)
- `types.rs`: `Message` (system/user/assistant), `ChatResponse`, `TokenUsage`
- `rate_limiter.rs`, `retry.rs`: control de concurrencia y reintentos HTTP
- Reemplazará a `providers.rs` + `agent.rs` cuando se complete la migración

---

## 💡 Decisiones de diseño importantes

1. **Arquitectura en capas**: `cli → app → domain/infra → config`.  
   Dependencias unidireccionales verificadas por `tests/architecture.rs` (18 tests, reglas R1-R5).  
   `domain/` no puede importar `infra/`, `app/`, ni `cli/`. `infra/` solo importa `config`.

2. **Agnóstico al proyecto anfitrión**: regista no sabe de Rust, cargo, ni nada.  
   Solo invoca el provider configurado con prompts genéricos.

3. **Async runtime con tokio**: `agent.rs` y `pipeline.rs` migrados a async/await.  
   `tokio::process::Command` + `tokio::time::timeout` reemplazan busy-polling.  
   Timeout real mata el proceso por PID (sin zombies). Operaciones de bloqueo (git, hooks) usan `spawn_blocking`.

4. **Workflow auto-contenido en `domain/workflow.rs`**: el trait `Workflow`, `CanonicalWorkflow`
   (14 transiciones fijas) y `ConfigurableWorkflow` (workflow desde TOML) viven en el crate.
   No hay dependencia externa `spartito`. El concepto de "partitura compartida" con `mezzala`
   está en estudio para una fase posterior; actualmente regista es autosuficiente.

5. **`SharedState` con `Arc<RwLock<>>`**: reemplaza `&mut HashMap` pasado por la pila.  
   Clonable, compartible entre tareas, preparado para `tokio::spawn` en paralelismo.

6. **Trait `AgentProvider` devuelve `Vec<String>`**: no `Command`, para ser  
   compatible con ejecución síncrona y asíncrona.

7. **CLI con clap Subcommand**: la CLI usa `#[derive(Subcommand)]` de clap 4.  
   Subcomandos actuales: `plan`, `auto`, `run`, `logs`, `status`, `kill`, `validate`, `init`, `update`, `board`.  
   **En v0.10.0**: `plan` y `auto` se unifican en `run --plan-only`.

8. **Dry-run en memoria**: `advance_status_in_memory()` muta `Story` sin tocar el filesystem.  
   Permite simular pipelines completos sin gastar créditos de LLM.

9. **Checkpoint TOML**: el estado del orquestador se guarda en `.regista/state.toml`.  
   Si el pipeline se interrumpe, `--resume` lo reanuda. Se limpia en `PipelineComplete`.

10. **Feedback rico en reintentos**: cuando un agente falla, su stderr se guarda en  
    `decisions/` y se inyecta en el prompt del reintento. Truncado a 2000 bytes.

11. **Plan con bucle validate**: generar historias no basta — hay que validar que las  
    dependencias son correctas. El PO recibe feedback concreto y corrige en bucle.

12. **Salida JSON en validate y board**: `--json` emite JSON a stdout para CI/CD.  
    El pipeline daemon escribe resultados en `.regista/daemon.log`.

13. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos (`delay *= 2`).

14. **`set_status()` con backup atómico**: escribe → re-parsea → si falla, restaura `.bak`.

15. **Provider por defecto `"pi"`**: retrocompatibilidad total. Si no se especifica  
    provider en config ni CLI, se usa pi.

16. **Provider por rol**: cada rol (PO, QA, Dev, Reviewer) puede usar un provider distinto.  
    Configurable en `.regista/config.toml`.

17. **Codex auto-descubre skills**: `CodexProvider` ignora el path de instrucciones —  
    Codex lee automáticamente `.agents/skills/` y `AGENTS.md` del proyecto.

18. **Skills inline en `init.rs`**: las instrucciones de rol están como constantes  
    con YAML frontmatter completo (`name`, `description`, `model`).  
    OpenCode usa el campo `model` para pasar `-m <model>` automáticamente.

19. **Health metrics**: `health.rs` calcula métricas del pipeline y las escribe  
    atómicamente a `.regista/health.json`.

20. **`max_reject_cycles = 8`**: por defecto, 8 ciclos de rechazo antes de `Failed`.  
    `max_iterations = 0`: auto-escala a `max(10, historias × 6)`.

21. **`Status` como enum, no como newtype**: el enum con 9 variantes es el tipo canónico
    en v0.x. `ConfigurableWorkflow` (v0.10.0) trabaja con strings para soportar estados
    arbitrarios definidos en TOML, sin necesidad de un newtype intermedio.

22. **Bifurcaciones sin sintaxis especial**: `transitions_from()` en `ConfigurableWorkflow`
    devuelve múltiples destinos desde un mismo estado. El prompt presenta las opciones;
    el agente decide; el orquestador solo valida.

23. **Dos sistemas de parseo coexistiendo**: `story.rs` (v0.x, formato hardcodeado
    STORY-NNN) y `task.rs` (v0.10.0, formato configurable) conviven durante la transición.
    `pipeline.rs` aún usa `Story`. La migración a `Task` es progresiva.

24. **Dos sistemas de prompts coexistiendo**: `prompts.rs` (v0.x, 7 funciones hardcodeadas)
    y `templates.rs` (v0.10.0, sistema de `{{variables}}`) conviven durante la transición.
    La migración es progresiva.

25. **Dos sistemas de invocación de agentes**: `providers.rs` + `agent.rs` (v0.x,
    tools CLI externas) y `infra/llm/` (v0.10.0, APIs HTTP nativas) coexisten.
    `infra/llm/` reemplazará a los providers CLI cuando esté completo.

26. **ModelConfig con soporte `${ENV_VAR}`**: las API keys en `[models]` usan
    la sintaxis `${NOMBRE_VAR}` para no escribir secretos en claro en TOML.

---

## 🔮 Decisiones de diseño — rework v0.10.0

Estas decisiones aplican durante el refactor en la rama `rework` y reemplazan
o amplían las decisiones de v0.x anteriores.

### 27. Unificación de `plan`/`auto`/`run` en `run --plan-only`

v0.x tiene tres comandos de pipeline. En v0.10.0 se unifican:

- **`regista run`**: único comando de ejecución. Si no hay tareas en `tasks_dir`,
  ejecuta primero la fase de descomposición del workflow y genera las tareas iniciales,
  luego ejecuta el pipeline completo sobre ellas.
- **`regista run --plan-only`**: ejecuta solo la fase de descomposición. Reemplaza a `plan`.
- `plan` y `auto` desaparecen como subcomandos.

**Motivo**: `plan` y `auto` asumían el dominio software-dev (spec → historias).
Con presets como `research` o `single-agent`, el concepto de "descomposición" no
siempre aplica. Unificar en `run` con una fase inicial opcional lo hace genérico.

### 28. `task_format` en config.toml define el formato interno de los .md

Cada archivo de tarea sigue un formato configurable definido en `[workflow.task_format]`:

```toml
[workflow.task_format]
id_pattern = "TASK-\\d+"
section_markers = {
    status      = "## Status",
    description = "## Descripción",
    priority    = "## Priority"
}
dependency_marker = "Bloqueado por:"
```

| Define `task_format` | Define `[project]` |
|----------------------|---------------------|
| Qué campos tiene cada .md y cómo se llaman | `tasks_dir` — dónde están los archivos |
| Patrón de ID (STORY-NNN, TASK-NNN, etc.) | `task_pattern` — glob para encontrarlos |
| Cómo se marcan las dependencias entre tareas | Resto de directorios (decisions, logs, etc.) |

La ubicación física de los archivos sigue siendo `[project].tasks_dir` (default `.regista/tasks/`).

### 29. Épicas como campo de task, no como archivos separados

v0.x tenía un directorio `epics/` con archivos `EPIC-NNN.md` independientes.
En v0.10.0, "épica" es un campo más del `task_format`:

```toml
section_markers = { epic = "## Epic" }
```

Cada tarea puede referenciar una épica directamente (`EPIC-001`). El board puede
filtrar por épica igual que antes. No hay archivos de épica separados.

### 30. Cliente LLM nativo (`infra/llm/`)

En lugar de invocar tools CLI externas (`pi`, `claude`, `codex`, `opencode`),
v0.10.0 se comunica directamente con las APIs HTTP de los modelos. El trait
`LlmProvider` abstrae OpenAI y Anthropic, con `ModelConfig` por modelo.

### 31. Templates de prompts con `{{variables}}`

Los 7 prompts hardcodeados de `prompts.rs` se reemplazan por archivos de
template en `.regista/prompts/` que usan sustitución de variables. Cada
fase del workflow referencia su template; el sistema `templates.rs` sustituye
`{{task_id}}`, `{{task_fields.*}}`, `{{context.<clave>}}`, etc.

---

## 🧪 Estrategia de testing

- **Tests unitarios**: cada módulo tiene `#[cfg(test)] mod tests` con fixtures inline
- **Tests de arquitectura**: `tests/architecture.rs` verifica R1-R5 (18 tests)
- **Fixtures**: `tests/fixtures/` contiene archivos .md de ejemplo
- **Test ignorado**: `agent::tests::invoke_with_retry_fails_when_agent_not_installed` (requiere `pi` en PATH)
- **Total actual**: 723 tests pasando, 3 fallos (macOS daemon), 1 ignorado
- **20 warnings de clippy**: código muerto en módulos v0.10.0 no cableados

Para añadir tests:
- Usa `make_story()` o `story_fixture()` helpers para crear Stories sintéticas
- Para tests de `Task`, usa `TaskFormatConfig` con `section_markers` inline
- No dependas de archivos reales salvo en tests de `story.rs` (que usan fixtures)
- Para tests de providers, usa `from_name()` y verifica `build_args()`
- Para tests de `LlmProvider`, implementa el trait con un struct dummy
- Para tests de async, usa `#[tokio::test]` y `tokio::task::spawn_blocking`
- Para tests de workflow, implementa `Workflow` con un struct ad-hoc
- Usa `tempfile::tempdir()` para aislar el filesystem

---

## ⚠️ Errores comunes y anti-patrones

- ❌ **Romper la arquitectura de capas**: `domain/` NO puede importar `infra/`, `app/`, `cli/`, ni `config`.  
  `infra/` solo puede importar `config`. `app/` no puede importar `cli/`.  
  `tests/architecture.rs` (18 tests) detecta estas violaciones.

- ❌ **Añadir transiciones a la máquina de estados**: las 14 transiciones canónicas
  son el default en `CanonicalWorkflow`. Para workflows custom, se define
  `[workflow]` en `.regista/config.toml` vía `ConfigurableWorkflow`.
  No se añaden transiciones en código.

- ❌ **Asumir que `Status` es un newtype**: es un enum con 9 variantes.
  `ConfigurableWorkflow` (v0.10.0) trabaja con strings para soportar estados arbitrarios.

- ❌ **Parsear historias sin usar `extract_section()`**: usa las funciones existentes
  en `domain/story.rs` o `domain/task.rs` según corresponda.

- ❌ **Modificar `raw_content` sin actualizar el campo correspondiente**: deben estar siempre sincronizados.

- ❌ **Ejecutar hooks sin `sh -c`**: los hooks son comandos shell.

- ❌ **Asumir que todos los bloqueadores existen**: filtrar siempre con `status_map.get()`.

- ❌ **Usar `..ctx` (struct update) sin clonar**: `PromptContext` contiene Strings, el update  
  syntax los mueve. Si necesitas reutilizar `ctx`, clona los campos explícitamente.

- ❌ **Llamar a `invoke_with_retry` sin provider**: la firma requiere `&dyn AgentProvider` como  
  primer argumento. Usa `providers::from_name("pi")?` si no necesitas un provider concreto.

- ❌ **Asumir que el provider es `pi`**: usa `AgentsConfig::provider_for_role(role)` para  
  resolver el provider correcto según la configuración del proyecto.

- ❌ **Hardcodear flags de provider**: usa `AgentProvider::build_args()` para construir los  
  argumentos de CLI. Cada provider tiene sus propios flags y subcomandos.

- ❌ **Usar `.regista.toml` como path de config**: el path correcto es `.regista/config.toml`  
  (dentro del directorio `.regista/`, no en la raíz).

- ❌ **Generar solo skills para pi en `init`**: el generador usa `AgentProvider::instruction_dir(role)`  
  para colocar las instrucciones en el directorio correcto según el provider.

- ❌ **Llamar a `RUNTIME.block_on()` desde dentro del runtime de tokio**: paniquea con  
  "Cannot start a runtime from within a runtime". Usa `spawn_blocking` en su lugar.

- ❌ **Bloquear el runtime async con operaciones síncronas**: usa `spawn_blocking` para  
  git, hooks, y cualquier I/O de bloqueo cuando estés en contexto async.

- ❌ **Referenciar `spartito` en código o documentación nueva**: `spartito` no es una
  dependencia del proyecto. No existe en `Cargo.toml` ni en ningún archivo `.rs`.
  Las referencias en `AGENTS.md`, `README.md` y roadmap son documentación histórica
  a eliminar (STORY-V10-020).

- ❌ **Trabajar en `main` para cambios v0.10.0**: la rama activa de desarrollo es `rework`.
  `main` contiene la versión estable v0.9.5.

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
- **Async tests**: `#[tokio::test]` para tests async, `#[test]` para sync
- **Nuevos módulos**: siguen el patrón `pub fn run(...) -> anyhow::Result<...>` para su entry point
- **Respetar capas**: antes de añadir un import `use crate::X`, verifica que `tests/architecture.rs` no lo rechace
- **Módulos v0.10.0 no cableados**: llevan `#![allow(dead_code)]` al inicio del archivo.
  No elimines este atributo hasta que el módulo esté integrado en el pipeline.
