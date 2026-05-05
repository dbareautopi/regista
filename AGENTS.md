# AGENTS.md — regista

Guía para agentes de codificación que trabajen en este proyecto.  
Incluye arquitectura, convenciones, comandos, y decisiones de diseño.

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
verificadas automáticamente por `tests/architecture.rs`:

```
regista/
├── AGENTS.md                  ← este archivo
├── README.md                  ← descripción general para usuarios
├── DESIGN.md                  ← diseño completo (máquina de estados, arquitectura, multi-provider)
├── HANDOFF.md                 ← handoff de la última sesión (lo implementado y pendiente)
├── Cargo.toml                 ← dependencias y metadata del crate
├── .gitignore
├── src/
│   ├── main.rs                ← entry point: mod app, cli, config, domain, infra
│   ├── config.rs              ← Config, AgentsConfig + AgentRoleConfig, StackConfig, carga TOML
│   │
│   ├── cli/                   ← 🟢 Capa CLI: args + handlers (puede importar cualquier capa)
│   │   ├── mod.rs
│   │   ├── args.rs            ← Cli, Commands (Plan/Auto/Run/Logs/Status/Kill/Validate/Init/Update/Board)
│   │   └── handlers.rs        ← dispatch(), handle_plan(), handle_run(), daemon, exit codes
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
│   │   ├── state.rs           ← Status (9), Actor (5), Transition (14), SharedState (Arc<RwLock<>>)
│   │   ├── story.rs           ← Story, parseo .md, set_status() atómico, advance_status_in_memory()
│   │   ├── graph.rs           ← DependencyGraph, DFS para ciclos, blocks_count()
│   │   ├── deadlock.rs        ← analyze(), DeadlockResolution, priorización por desbloqueo
│   │   ├── prompts.rs         ← PromptContext, DomainStackConfig, 7 prompts stack-agnósticos
│   │   └── workflow.rs        ← Trait Workflow + CanonicalWorkflow (extensible para #04)
│   │
│   └── infra/                 ← 🔵 Capa Infraestructura: I/O, procesos, git (importa solo config)
│       ├── mod.rs
│       ├── providers.rs       ← trait AgentProvider + Pi/ClaudeCode/Codex/OpenCode + factory
│       ├── agent.rs           ← invoke_with_retry() async, backoff, feedback rico, tokio runtime
│       ├── checkpoint.rs      ← OrchestratorState: save/load/remove (.regista/state.toml)
│       ├── daemon.rs          ← detach(), status(), kill(), follow(), PidCleanup
│       ├── git.rs             ← snapshot(), rollback() con spawn_blocking
│       └── hooks.rs           ← run_hook(): comandos shell post-fase con tokio::process::Command
│
├── tests/
│   ├── architecture.rs        ← 11 tests: verifica que las capas respetan R1-R5
│   └── fixtures/
│       ├── story_draft.md
│       ├── story_blocked.md
│       └── story_business_review.md
│
└── roadmap/                   ← Documentos de diseño de features futuras
    ├── ROADMAP.md             ← Índice con estado de cada feature y orden de implementación
    ├── 01-paralelismo.md      ← Diseño detallado (Fase 7, último)
    ├── 04-workflow-configurable.md ← Diseño detallado (Fase 5)
    ├── 10-cross-story-context.md   ← Diseño definido (Fase 4)
    └── ...
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
cargo test --lib infra::providers
cargo test --lib app::pipeline

# Ejecutar test ignorado (requiere pi instalado)
cargo test -- --ignored

# Ver warnings
cargo check

# Formatear
cargo fmt

# Linting
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

### Transiciones canónicas (inmutables, definidas en `domain/state.rs`)

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

### Trait `Workflow` (extensibilidad)

El trait `Workflow` en `domain/workflow.rs` abstrae la lógica de transiciones:

```rust
pub trait Workflow: Sync {
    fn next_status(&self, current: Status) -> Status;
    fn map_status_to_role(&self, status: Status) -> &'static str;
    fn canonical_column_order(&self) -> &[&'static str];
}
```

`CanonicalWorkflow` implementa las 14 transiciones fijas. El trait permite
que futuros workflows configurables (#04) reemplacen esta lógica sin tocar
el código del pipeline ni del board.

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

### Reglas de parseo (`domain/story.rs`)

| Campo | Cómo se extrae |
|-------|---------------|
| **Status** | Busca `## Status` (case-insensitive), lee la línea siguiente, limpia `**...**` |
| **Bloqueadores** | Busca `Bloqueado por:` (case-insensitive) dentro de `## Dependencias`, extrae `STORY-\d+` |
| **Epic** | Busca `## Epic`, lee la línea siguiente, extrae `EPIC-\d+` |
| **Last rejection** | Busca `## Activity Log`, última línea que contiene "rechaz" (case-insensitive) |
| **Last actor** | Busca `## Activity Log`, última línea, extrae actor entre `|` |

---

## 🧩 Descripción de módulos

### Capa `cli/` — Interfaz de usuario (puede importar cualquier capa)

#### `args.rs` — Definición de CLI
- `Cli` con `#[derive(Parser)]`, 10 subcomandos vía `Commands` enum
- Estructuras compuestas con `#[command(flatten)]`: `RepoArgs`, `PlanModeArgs`, `CommonArgs`, `PipelineArgs`, `DaemonArgs`
- `PlanArgs`, `AutoArgs`, `RunArgs`, `ValidateArgs`, `InitArgs`, `UpdateArgs`, `BoardArgs`, `RepoArgs`

#### `handlers.rs` — Dispatch y handlers
- `dispatch(cli)`: enruta cada subcomando a su handler
- `handle_plan()`, `handle_auto()`, `handle_run()`: daemon / dry-run / sync dispatch
- `handle_logs()`, `handle_status()`, `handle_kill()`: gestión del daemon
- `handle_validate()`, `handle_init()`, `handle_update()`, `handle_board()`
- `build_daemon_args()`: construye argumentos para el proceso hijo
- `setup_daemon_tracing()` / `setup_user_tracing()`: configuración de logs
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
  - Snapshot git con `spawn_blocking`, hooks con `spawn_blocking`
  - Detecta si el agente rechazó e incrementa `reject_cycles`
- `RunReport`: serializable a JSON con `StoryRecord` por historia
- Tests: 18 tests

#### `plan.rs` — Generación de backlog
- `run()`: invoca al PO para descomponer spec en historias y épicas
- **Bucle de validación**: plan → validate dependencias → feedback al PO → corregir (máx `plan_max_iterations`)
- `--max-stories` (0 = sin límite), `--replace`
- Prompts: `plan_prompt_initial()`, `plan_prompt_fix()` con `PlanCtx`
- Tests: 6 tests

#### `board.rs` — Dashboard Kanban
- `BoardData`: conteo por estado, `BlockedStory`, `FailedStory`
- `render_board()`: acepta `&dyn Workflow` para orden dinámico de columnas
- Columnas vacías (count=0) se omiten; orden canónico según workflow
- `--json` para CI/CD, `--epic` para filtrar
- Tests: 9 tests

#### `health.rs` — Health & Metrics
- `HealthReport`: métricas agregadas (iteraciones/hora, tiempo medio, tasa rechazo, coste)
- `generate_report()`: cálculo puro desde datos crudos
- `is_health_checkpoint()`: cada N iteraciones (default: 10)
- `write_health_json()`: escritura atómica a `.regista/health.json` (tmp → rename)
- `write_final_health_report()`: reporte final en PipelineComplete
- Tests: 19 tests

#### `validate.rs` — Chequeo pre-vuelo
- Valida: config, skills multi-provider, providers en PATH, historias (parseo, Activity Log), dependencias (refs rotas, ciclos), git
- `ValidationResult` con `Vec<Finding>` (severity: Error/Warning, category, message)
- `validate_providers()`: verifica binarios en PATH (Error para codex → Warning)
- `--json` para CI, exit codes: 0=OK, 1=errores, 2=warnings
- Tests: 3 tests

#### `init.rs` — Scaffolding
- `init(project_dir, light, with_example, provider_name)`: genera `.regista/config.toml` + instrucciones
- Directorios por provider: `pi`→`.pi/skills/`, `claude`→`.claude/agents/`, `codex`→`.agents/skills/`, `opencode`→`.opencode/agents/`
- `--light`: solo config, `--with-example`: historia + épica de ejemplo
- Skills inline como constantes con YAML frontmatter (`name`, `model`, `description`)
- No pisa archivos existentes
- Tests: 7 tests

#### `update.rs` — Auto-update
- `check()`: consulta crates.io vía `ureq`, compara versiones semánticas
- `run(auto_yes)`: instala con `cargo install regista --version <latest>`
- Flag `--yes` para omitir prompt interactivo
- Tests: 2 tests

### Capa `domain/` — Lógica pura (NO importa otras capas del crate)

#### `state.rs` — Máquina de estados
- `Status` enum: 9 variantes con `Display`, `is_terminal()`, `is_actionable()`, `is_stuck()`
- `Actor` enum: `ProductOwner`, `QaEngineer`, `Developer`, `Reviewer`, `Orchestrator`
- `Transition` struct con `Status::ALL` — 14 transiciones canónicas
- `SharedState`: `Arc<RwLock<HashMap<>>>` para estado compartido entre tareas (paralelismo #01)
  - `reject_cycles`, `story_iterations`, `story_errors`
  - `Clone` comparte el mismo `Arc`; `read()`/`write()` con `RwLock`
- Tests: 23 tests + tests de `SharedState` (STORY-011)

#### `story.rs` — Parseo de historias
- `Story` struct: `id`, `path`, `status`, `epic`, `blockers`, `last_rejection`, `raw_content`
- `load()`: lee archivo .md y parsea todos los campos
- `set_status()`: escribe a disco con backup atómico + verificación
- `advance_status_in_memory()`: muta estado sin tocar disco (dry-run)
- `last_actor()`: extrae último actor del Activity Log
- Tests: 12 tests

#### `graph.rs` — Grafo de dependencias
- `DependencyGraph`: `forward` (bloqueador→bloqueados), `reverse`, DFS con colores
- `blocks_count()`, `has_cycle_from()`, `has_any_cycle()`, `find_cycle_members()`
- Tests: 4 tests

#### `deadlock.rs` — Detección de bloqueos
- `DeadlockResolution` enum: `NoDeadlock`, `InvokePoFor`, `PipelineComplete`
- `analyze()`: algoritmo de 4 pasos con priorización
  - Identifica Draft, ciclos, bloqueadores Draft
  - Prioriza por: mayor `unblocks`, luego menor ID numérico
- Tests: 7 tests

#### `prompts.rs` — Generación de prompts
- `PromptContext`: `story_id`, `stories_dir`, `decisions_dir`, `last_rejection`, `from`, `to`, `stack: DomainStackConfig`
- `DomainStackConfig::render()`: bloque de comandos o instrucción genérica
- `PromptContext::header()` / `suffix()`: helpers privados
- 7 prompts: `po_plan()`, `po_validate()`, `qa_tests()`, `qa_fix_tests()`, `dev_implement()`, `dev_fix()`, `reviewer()`
- `qa_tests()` incluye reglas estrictas (NO crear módulos, NO implementar, solo tests)
- Tests: 15 tests

#### `workflow.rs` — Trait de workflow extensible
- `Workflow` trait: `next_status()`, `map_status_to_role()`, `canonical_column_order()`
- `CanonicalWorkflow`: implementación con las 14 transiciones fijas
- El trait usa `&self` (no `&mut self`) + `Sync` — compatible con paralelismo
- Tests: 35 tests (happy path, fix path, determinismo, trait object safety)

### Capa `infra/` — Infraestructura (importa solo `config`)

#### `providers.rs` — Sistema de providers
- `AgentProvider` trait: `binary()`, `build_args()`, `display_name()`, `instruction_name()`, `instruction_dir()`
- El trait devuelve `Vec<String>` (no `Command`) — compatible con sync y async
- **PiProvider**: `pi -p "..." --skill <path> --no-session`
- **ClaudeCodeProvider**: `claude -p "..." --append-system-prompt-file <path> --permission-mode bypassPermissions`
- **CodexProvider**: `codex exec --sandbox workspace-write "..."` (auto-descubre `.agents/skills/`)
- **OpenCodeProvider**: `opencode run --agent <name> --dangerously-skip-permissions "..."` (soporta `-m <model>` desde YAML)
- Factory `from_name(name)`: resuelve alias (claude-code, open-code, etc.), case-insensitive, retorna `Result`
- `read_yaml_field()`: extrae campos del frontmatter YAML de archivos .md
- `find_in_path()`: verifica disponibilidad de binarios
- Tests: 17 tests

#### `agent.rs` — Invocación de agentes (async)
- `invoke_with_retry()`: async, loop con backoff exponencial (`delay *= 2`), timeout real con `tokio::time::timeout`
- `invoke_once()`: async, usa `tokio::process::Command`, mata proceso por PID en timeout
- `invoke_with_retry_blocking()`: wrapper síncrono con `RUNTIME.block_on()`
- `AgentOptions`: `story_id`, `decisions_dir`, `inject_feedback`
- `build_feedback_prompt()`: inyecta stderr truncado (2000 bytes) en reintentos
- `save_agent_decision()`: async, guarda trazas en `decisions/` con `tokio::fs`
- `RUNTIME`: `LazyLock<tokio::runtime::Runtime>` global para callers síncronos
- Tests: 3 tests + 1 ignorado

#### `checkpoint.rs` — Persistencia del estado
- `OrchestratorState`: `iteration`, `reject_cycles`, `story_iterations`, `story_errors`
- `save()` / `load()` / `remove()` sobre `.regista/state.toml`
- `load()` maneja archivos corruptos (los borra)
- Test de integración con `SharedState` (clonación bajo read lock)
- Tests: 5 tests

#### `daemon.rs` — Modo daemon
- `detach()`: spawnea proceso hijo con `--daemon` interno, guarda PID en `.regista/daemon.pid`
- `status()`, `kill()`, `follow()`: gestión del proceso
- `PidCleanup`: guard RAII que limpia el archivo PID al salir
- `get_all_child_pids()`: recursivo vía `/proc/<pid>/task/*/children` (Linux) o `wmic` (Windows)
- Soporte multiplataforma: `#[cfg(windows)]` / `#[cfg(not(windows))]`
- Tests: 6 tests

#### `git.rs` — Snapshots y rollback
- `snapshot()`: `git add -A && git commit -q -m "snapshot: {label}"`, auto-inicializa repo
- `rollback()`: `git reset --hard <hash>`
- `check_git_changes()`: detecta cambios unstaged, staged, y untracked
- Tests con `spawn_blocking` para seguridad async (STORY-012)
- Tests: 5 tests

#### `hooks.rs` — Hooks post-fase
- `run_hook()`: ejecuta `sh -c "<comando>"` con `tokio::process::Command` vía `RUNTIME`
- Invocable desde sync y async (usa `spawn_blocking` en tests async)
- Tests: 4 tests

---

## 💡 Decisiones de diseño importantes

1. **Arquitectura en capas**: `cli → app → domain/infra → config`.  
   Dependencias unidireccionales verificadas por `tests/architecture.rs` (11 tests, reglas R1-R5).  
   `domain/` no puede importar `infra/`, `app/`, ni `cli/`. `infra/` solo importa `config`.

2. **Agnóstico al proyecto anfitrión**: regista no sabe de Rust, cargo, ni nada.  
   Solo invoca el provider configurado con prompts genéricos.

3. **Async runtime con tokio**: `agent.rs` y `pipeline.rs` migrados a async/await.  
   `tokio::process::Command` + `tokio::time::timeout` reemplazan busy-polling.  
   Timeout real mata el proceso por PID (sin zombies). Operaciones de bloqueo (git, hooks) usan `spawn_blocking`.

4. **Workflow fijo e inmutable**: las 14 transiciones son canónicas.  
   No se añaden transiciones en runtime **por diseño**.  
   El trait `Workflow` abstrae la lógica para que #04 pueda reemplazarla sin tocar el pipeline.

5. **`SharedState` con `Arc<RwLock<>>`**: reemplaza `&mut HashMap` pasado por la pila.  
   Clonable, compartible entre tareas, preparado para `tokio::spawn` en paralelismo (#01).

6. **Trait `AgentProvider` devuelve `Vec<String>`**: no `Command`, para ser  
   compatible con ejecución síncrona y asíncrona (paralelismo #01).  
   El invocador decide si usar `std::process::Command` o `tokio::process::Command`.

7. **CLI con clap Subcommand**: la CLI usa `#[derive(Subcommand)]` de clap 4.  
   Los subcomandos son `plan`, `auto`, `run`, `logs`, `status`, `kill`, `validate`, `init`, `update`, `board`.  
   `--version` y `--help` son nativos de clap.

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
    provider en config ni CLI, se usa pi. Projects existentes siguen funcionando sin cambios.

16. **Provider por rol**: cada rol (PO, QA, Dev, Reviewer) puede usar un provider distinto.  
    Ejemplo: PO con Claude Code, Dev con pi. Configurable en `.regista/config.toml`.

17. **Codex auto-descubre skills**: `CodexProvider` ignora el path de instrucciones —  
    Codex lee automáticamente `.agents/skills/` y `AGENTS.md` del proyecto.

18. **Skills inline en `init.rs`**: las instrucciones de rol están como constantes  
    con YAML frontmatter completo (`name`, `description`, `model`).  
    OpenCode usa el campo `model` para pasar `-m <model>` automáticamente.

19. **Health metrics**: `health.rs` calcula métricas del pipeline y las escribe  
    atómicamente a `.regista/health.json`. Preparado para TUI (#11) y cost tracking (#12).

20. **`max_reject_cycles = 8`**: por defecto, 8 ciclos de rechazo antes de `Failed`.  
    `max_iterations = 0`: auto-escala a `max(10, historias × 6)`.

---

## 🚧 Pendiente (roadmap)

| # | Feature | Esfuerzo | Fase |
|---|---------|----------|------|
| 01 | Paralelismo | Alto | 7 (último) |
| 04 | Workflow configurable | Medio | 5 |
| 10 | Cross-story context | Medio | 4 |
| 11 | TUI / dashboard | Medio | 6 |
| 12 | Cost tracking | Medio | 6 |
| 14 | Plan `--from-dir` | Bajo | 3 |
| 15 | Plan `--interactive` | Medio | 6 |

---

## 🧪 Estrategia de testing

- **Tests unitarios**: cada módulo tiene `#[cfg(test)] mod tests` con fixtures inline
- **Tests de arquitectura**: `tests/architecture.rs` verifica R1-R5 (11 tests)
- **Fixtures**: `tests/fixtures/` contiene archivos .md de ejemplo
- **Test ignorado**: `agent::tests::invoke_with_retry_fails_when_agent_not_installed` (requiere `pi` en PATH)
- **Total**: 357 tests pasando, 0 fallos, 1 ignorado

Para añadir tests:
- Usa `make_story()` o `story_fixture()` helpers para crear Stories sintéticas
- No dependas de archivos reales salvo en tests de `story.rs` (que usan fixtures)
- Para tests de providers, usa `from_name()` para obtener una instancia y verificar `build_args()`
- Para tests de async, usa `#[tokio::test]` y `tokio::task::spawn_blocking` para operaciones de bloqueo
- Para tests de workflow, implementa `Workflow` con un struct ad-hoc (ver tests de `AltWorkflow`)
- Usa `tempfile::tempdir()` para aislar el filesystem

---

## ⚠️ Errores comunes y anti-patrones

- ❌ **Romper la arquitectura de capas**: `domain/` NO puede importar `infra/`, `app/`, `cli/`, ni `config`.  
  `infra/` solo puede importar `config`. `app/` no puede importar `cli/`.  
  `tests/architecture.rs` detecta estas violaciones.

- ❌ **Añadir transiciones a `Status::ALL`**: rompe la inmutabilidad del workflow.  
  Las 14 transiciones son el contrato fijo.

- ❌ **Parsear historias sin usar `extract_section()`**: usa las funciones existentes en `domain/story.rs`.

- ❌ **Modificar `raw_content` sin actualizar `status`**: deben estar siempre sincronizados.

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
