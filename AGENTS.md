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
2. Qué skills de `pi` usar para cada rol
3. La máquina de estados fija

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
| Build | `cargo` |

---

## 📁 Estructura del proyecto

```
regista/
├── AGENTS.md                  ← este archivo
├── README.md                  ← descripción general para usuarios
├── DESIGN.md                  ← diseño completo (máquina de estados, arquitectura)
├── HANDOFF.md                 ← handoff de la última sesión (lo implementado y pendiente)
├── Cargo.toml                 ← dependencias y metadata del crate
├── .gitignore
├── src/
│   ├── main.rs                ← CLI (clap), subcomandos, JSON output, exit codes
│   ├── config.rs              ← Config, ProjectConfig, AgentsConfig, LimitsConfig, carga TOML
│   ├── state.rs               ← Status, Actor, Transition, can_transition_to(), ALL
│   ├── story.rs               ← Story, parseo de .md, set_status(), advance_status_in_memory()
│   ├── dependency_graph.rs    ← DependencyGraph, DFS para ciclos, blocks_count()
│   ├── deadlock.rs            ← analyze(), DeadlockResolution, priorización
│   ├── agent.rs               ← invoke_with_retry(), AgentOptions, feedback rico
│   ├── prompts.rs             ← PromptContext, 7 funciones de prompt
│   ├── orchestrator.rs        ← run(), run_real(), run_dry(), process_story(), checkpoint save
│   ├── checkpoint.rs          ← OrchestratorState: save/load/remove (.regista.state.toml)
│   ├── validator.rs           ← validate(): chequeo pre-vuelo de proyecto
│   ├── init.rs                ← init(): scaffolding de proyecto nuevo
│   ├── groom.rs               ← run(): generación de backlog desde spec con bucle validate
│   ├── hooks.rs               ← run_hook(), ejecución de comandos shell post-fase
│   ├── git.rs                 ← snapshot(), rollback(), init_git()
│   └── daemon.rs              ← detach(), status(), kill(), follow(), DaemonState
├── roadmap/                   ← Documentos de diseño de features futuras
│   ├── ROADMAP.md             ← Índice con estado de cada feature
│   ├── 01-paralelismo.md
│   ├── 02-salida-json-ci-cd.md        ← ✅ IMPLEMENTADO
│   ├── 03-dry-run.md                  ← ✅ IMPLEMENTADO
│   ├── 04-workflow-configurable.md
│   ├── 05-validate.md                 ← ✅ IMPLEMENTADO
│   ├── 06-init-scaffold.md            ← ✅ IMPLEMENTADO
│   ├── 07-checkpoint-resume.md        ← ✅ IMPLEMENTADO
│   ├── 08-feedback-agentes.md         ← ✅ IMPLEMENTADO
│   ├── 13-groom-generacion-historias.md ← ✅ IMPLEMENTADO
│   ├── 14-groom-from-dir.md
│   └── 15-groom-interactive.md
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
cargo test --lib groom

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
Draft ──PO(groom)──→ Ready ──QA(tests)──→ Tests Ready ──Dev(implement)──→ In Review
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
- Define `Cli` con clap (16 flags + project_dir posicional)
- Detecta subcomandos `validate`, `init`, `groom`, `help` antes del parseo de clap
- Configura tracing (stderr, env-filter, respeta `--quiet`)
- Salida JSON a stdout con `--json`; exit codes 0/2/3
- Manejo de `--resume` y `--clean-state`

### `config.rs` — Configuración
- `Config` con `#[serde(default)]`: todos los campos tienen defaults razonables
- Carga desde `.regista.toml`; si no existe, usa defaults
- Nuevos campos: `groom_max_iterations` (default 5), `inject_feedback_on_retry` (default true)
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

### `agent.rs` — Invocación de agentes con feedback
- `invoke_with_retry()`: loop con backoff exponencial, acepta `AgentOptions`
- `AgentOptions`: `story_id`, `decisions_dir`, `inject_feedback`
- Feedback rico: guarda stdout/stderr en `decisions/`, inyecta errores en reintentos
- `AgentResult` incluye `attempt` y `attempts: Vec<AttemptTrace>`
- Tests: 3 tests + 1 ignorado

### `prompts.rs` — Generación de prompts
- `PromptContext`: `story_id`, `stories_dir`, `decisions_dir`, `last_rejection`, `from`, `to`
- 7 funciones de prompt (una por transición accionable por agentes)
- Todos los prompts terminan con `"NO preguntes. 100% autónomo."`
- Tests: 4 tests

### `orchestrator.rs` — Loop principal
- `run()`: dispatch a `run_real()` o `run_dry()` según `options.dry_run`
- `run_real()`: loop con carga de historias, transiciones automáticas, deadlock, process_story
  - Acepta `resume_state: Option<OrchestratorState>` para `--resume`
  - Guarda checkpoint tras cada `process_story()` exitoso
  - Limpia checkpoint en `PipelineComplete`
- `run_dry()`: simulación en memoria sin agentes ni escritura a disco
  - Usa `advance_status_in_memory()` para mutar estados
  - Muestra desbloqueos y estima tiempo
- `process_story()`: determina skill + prompt, invoca agente con AgentOptions
- `filter_stories()`: aplica filtros `--story`, `--epic`, `--epics`
- Tests: 18 tests

### `checkpoint.rs` — Persistencia del estado
- `OrchestratorState`: `iteration`, `reject_cycles`, `story_iterations`, `story_errors`
- `save()` / `load()` / `remove()` sobre `.regista.state.toml`
- `load()` maneja archivos corruptos (los borra)
- Tests: 5 tests

### `validator.rs` — Comando `validate`
- Valida: config, skills, historias (parseo, Activity Log), dependencias (refs rotas, ciclos), git
- `ValidationResult` con `Vec<Finding>` (severity: Error/Warning, category, message)
- `--json` para CI, exit codes: 0=OK, 1=errores, 2=warnings
- Tests: 3 tests

### `init.rs` — Comando `init`
- Genera `.regista.toml`, 4 `SKILL.md`, estructura `product/...`
- `--light`: solo config, sin skills
- `--with-example`: incluye historia y épica de ejemplo
- No pisa archivos existentes
- Tests: 5 tests

### `groom.rs` — Comando `groom`
- `run()`: invoca al PO para generar historias desde spec
- **Bucle de validación**: groom → validate dependencias → si errores → feedback al PO → repetir
- Máx iteraciones: `groom_max_iterations` (default 5)
- `--max-stories` (0 = sin límite), `--merge`/`--replace`
- Prompts: `groom_prompt_initial()`, `groom_prompt_fix()` con `GroomCtx`
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
- Estado guardado en `.regista.pid` (TOML)
- Tests: 6 tests

---

## 💡 Decisiones de diseño importantes

1. **Agnóstico al proyecto anfitrión**: regista no sabe de Rust, cargo, ni nada.  
   Solo invoca `pi --skill <path>` con prompts genéricos.

2. **Workflow fijo e inmutable**: las 14 transiciones son canónicas.  
   No se añaden transiciones en runtime **por diseño**.

3. **Subcomandos vía args manual**: `validate`, `init`, `groom` se detectan antes de clap  
   inspeccionando `std::env::args()`. Evita refactorizar toda la CLI con clap subcommands.

4. **Dry-run en memoria**: `advance_status_in_memory()` muta `Story` sin tocar el filesystem.  
   Permite simular pipelines completos sin gastar créditos de LLM.

5. **Checkpoint TOML**: el estado del orquestador se guarda en `.regista.state.toml`.  
   Si el pipeline se interrumpe, `--resume` lo reanuda. Se limpia en `PipelineComplete`.

6. **Feedback rico en reintentos**: cuando un agente falla, su stderr se guarda en  
   `decisions/` y se inyecta en el prompt del reintento. Truncado a 2000 bytes.

7. **Groom con bucle validate**: generar historias no basta — hay que validar que las  
   dependencias son correctas. El PO recibe feedback concreto y corrige en bucle.

8. **Salida JSON dual**: `--json` emite JSON a stdout, logs a stderr.  
   `--quiet` suprime logs. Compatible con `regista --json > report.json`.

9. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos (`delay *= 2`).

10. **`set_status()` con backup atómico**: escribe → re-parsea → si falla, restaura `.bak`.

---

## 🚧 Pendiente (roadmap)

### Features no implementadas

| # | Feature | Esfuerzo |
|---|---------|----------|
| 01 | Paralelismo | Alto |
| 04 | Workflow configurable | Medio |
| 09 | Prompts agnósticos al stack | Bajo |
| 10 | Cross-story context | Medio |
| 11 | TUI / dashboard | Medio |
| 12 | Cost tracking | Medio |
| 14 | Groom `--from-dir` | Bajo |
| 15 | Groom `--interactive` | Medio |

---

## 🧪 Estrategia de testing

- **Tests unitarios**: cada módulo tiene `#[cfg(test)] mod tests` con fixtures inline
- **Fixtures**: `tests/fixtures/` contiene archivos .md de ejemplo para pruebas de parseo
- **Test ignorado**: `agent::tests::invoke_with_retry_fails_when_pi_not_installed` (requiere `pi` en PATH)
- **Total**: 104 tests pasando, 0 fallos, 1 ignorado

Para añadir tests:
- Usa `make_story()` o `story_fixture()` helpers para crear Stories sintéticas
- No dependas de archivos reales salvo en tests de `story.rs` (que usan fixtures)
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

- ❌ **Llamar a `invoke_with_retry` sin `AgentOptions`**: la firma cambió, ahora requiere  
  el 4º argumento. Usa `&AgentOptions::default()` si no necesitas feedback.

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
