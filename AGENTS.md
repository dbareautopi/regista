# AGENTS.md — regista

Guía para agentes de codificación que trabajen en este proyecto.  
Incluye arquitectura, convenciones, comandos, y decisiones de diseño.

---

## 📌 ¿Qué es esto?

`regista` es un **orquestador genérico de agentes** para [`pi`](https://github.com/mariozechner/pi-coding-agent).  
Automatiza un pipeline de desarrollo de software con 4 roles (PO, QA, Dev, Reviewer)  
gobernado por una **máquina de estados** formal con detección de deadlocks.

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
├── .regista.toml              ← config de ejemplo (no es la config real de este proyecto)
├── .gitignore
├── src/
│   ├── main.rs                ← CLI (clap), entry point, setup de tracing
│   ├── config.rs              ← Config, ProjectConfig, AgentsConfig, LimitsConfig, carga TOML
│   ├── state.rs               ← Status, Actor, Transition, can_transition_to(), ALL
│   ├── story.rs               ← Story, parseo de .md, set_status(), extract_section()
│   ├── dependency_graph.rs    ← DependencyGraph, DFS para ciclos, blocks_count()
│   ├── deadlock.rs            ← analyze(), DeadlockResolution, priorización
│   ├── agent.rs               ← invoke_with_retry(), invoke_once(), backoff exponencial
│   ├── prompts.rs             ← PromptContext, 7 funciones de prompt (po_groom, qa_tests, etc.)
│   ├── orchestrator.rs        ← run(), process_story(), apply_automatic_transitions()
│   ├── hooks.rs               ← run_hook(), ejecución de comandos shell post-fase
│   └── git.rs                 ← snapshot(), rollback(), init_git()
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
cargo test --lib deadlock

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

### Estados "stuck" (requieren intervención del PO)

- `Draft` — necesita refinamiento
- `Blocked` con bloqueador en `Draft` — el Draft bloqueante debe avanzar
- `Blocked` con ciclo de dependencias — PO debe romper el ciclo

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
- 2026-04-30 | PO | Movida de Draft a Ready
```

### Reglas de parseo (`story.rs`)

| Campo | Cómo se extrae |
|-------|---------------|
| **Status** | Busca `## Status` (case-insensitive), lee la línea siguiente, limpia `**...**` |
| **Bloqueadores** | Busca `Bloqueado por:` (case-insensitive) dentro de `## Dependencias`, extrae `STORY-\d+` |
| **Epic** | Busca `## Epic`, lee la línea siguiente, extrae `EPIC-\d+` |
| **Last rejection** | Busca `## Activity Log`, última línea que contiene "rechaz" (case-insensitive) |

---

## 🧩 Descripción de módulos

### `main.rs` — Entry point
- Define `Cli` con clap (11 flags)
- Configura tracing (stderr, env-filter desde `RUST_LOG`)
- Carga config y llama a `orchestrator::run()`
- Flags implementados: `--config`, `--once`, `--story`, `--epic`, `--epics`, `--detach`, `--follow`, `--status`, `--kill`, `--log-file`

### `config.rs` — Configuración
- `Config` con `#[serde(default)]`: todos los campos tienen defaults razonables
- Carga desde `.regista.toml`; si no existe, usa defaults
- `validate()`: verifica que `stories_dir` existe, crea `decisions_dir` y `log_dir`
- `resolve()`: convierte rutas relativas a absolutas

### `state.rs` — Máquina de estados
- `Status` enum: 9 variantes (`Draft`, `Ready`, `TestsReady`, `InProgress`, `InReview`, `BusinessReview`, `Done`, `Blocked`, `Failed`)
- `Actor` enum: `ProductOwner`, `QaEngineer`, `Developer`, `Reviewer`, `Orchestrator`
- `Transition` struct: `from`, `to`, `actor`
- `Status::ALL` — array canónico de 12 transiciones (const)
- Métodos: `allowed_from()`, `can_transition_to()`, `is_terminal()`, `is_actionable()`, `is_stuck()`
- Tests: 23 tests unitarios de transiciones válidas e inválidas

### `story.rs` — Parseo de historias
- `Story` struct: `id`, `path`, `status`, `epic`, `blockers`, `last_rejection`, `raw_content`
- `load()`: lee archivo .md y parsea todos los campos
- `set_status()`: reemplaza la línea `**StatusActual**` por `**NuevoStatus**` en el archivo
  - Hace backup a `.md.bak` antes de escribir
  - Verifica tras escritura (re-parsea); si falla, restaura backup
- `blocks_stories()`: retorna IDs de historias que este story bloquea
- Funciones de parseo: `parse_status()`, `parse_epic()`, `parse_blockers()`, `parse_last_rejection()`
- `extract_section()`: extrae contenido entre dos `## Headers`
- Tests: 8 tests de parseo

### `dependency_graph.rs` — Grafo de dependencias
- `DependencyGraph`: `forward` (bloqueador → bloqueados), `reverse` (bloqueado → bloqueadores)
- `from_stories()`: construye el grafo desde `Vec<Story>`
- `blocks_count()`: cuántas historias bloquea esta
- `has_cycle_from()`: DFS con colores (0=no visitado, 1=en pila, 2=procesado)
- `has_any_cycle()`: detecta si existe algún ciclo
- `find_cycle_members()`: retorna HashSet de IDs en ciclos
- Tests: 4 tests (no-ciclo, 2-nodos, 3-nodos, conteo)

### `deadlock.rs` — Detección de bloqueos
- `DeadlockResolution` enum: `NoDeadlock`, `InvokePoFor {story_id, unblocks, reason}`, `PipelineComplete`
- `analyze()`: algoritmo de 4 pasos:
  1. Si hay historias accionables → `NoDeadlock`
  2. Si todo está terminal → `PipelineComplete`
  3. Evalúa Draft (necesitan grooming) y Blocked (bloqueador en Draft, ciclo)
  4. Prioriza por: mayor `unblocks`, luego menor ID numérico
- Tests: 7 tests (draft, blocked-by-draft, accionable, done, ciclo, in-progress, prioridad)

### `agent.rs` — Invocación de agentes
- `invoke_with_retry()`: loop con backoff exponencial (delay ×= 2)
- `invoke_once()`: `pi -p "<prompt>" --skill <path> --no-session`
- Timeout y reintentos configurados desde `LimitsConfig`
- Tests: 1 test (ignorado, requiere pi instalado)

### `prompts.rs` — Generación de prompts
- `PromptContext`: `story_id`, `stories_dir`, `decisions_dir`, `last_rejection`, `from`, `to`
- 7 funciones de prompt (una por transición accionable por agentes):
  - `po_groom()` — Draft → Ready
  - `po_validate()` — Business Review → Done
  - `qa_tests()` — Ready → Tests Ready
  - `qa_fix_tests()` — Tests Ready → Tests Ready
  - `dev_implement()` — Tests Ready → In Review
  - `dev_fix()` — In Progress → In Review
  - `reviewer()` — In Review → Business Review / In Progress
- Todos los prompts terminan con `"NO preguntes. 100% autónomo."`
- Tests: 4 tests (presencia de story_id, rejection, no-preguntes)

### `orchestrator.rs` — Loop principal
- `run()`: itera hasta `max_iterations` o `max_wall_time`:
  1. Carga historias → `load_all_stories()`
  2. Aplica transiciones automáticas → `apply_automatic_transitions()`
  3. Detecta deadlock → `deadlock::analyze()`
  4. Si deadlock: dispara PO para la historia stuck
  5. Si no: `pick_next_actionable()` → `process_story()`
- `process_story()`:
  1. Determina skill + prompt según status actual
  2. Snapshot git (si `git.enabled`)
  3. Invoca agente con retry
  4. Verifica cambio de estado; incrementa `reject_cycles` si hubo rechazo
  5. Ejecuta hook post-fase; rollback si falla
- `pick_next_actionable()`: prioriza por `status_priority()` → `blocks_count()` → ID más bajo
- `status_priority()`: BusinessReview(6) > InReview(5) > InProgress(4) > TestsReady(3) > Ready(2)
- `daemon.rs` — `detach()`, `status()`, `kill()`, `follow()`, `PidCleanup`, `DaemonState` | 6 tests |

### `hooks.rs` — Hooks post-fase
- `run_hook()`: ejecuta `sh -c "<comando>"`, retorna error si exit code ≠ 0
- Los hooks son comandos shell, no binarios directos

### `git.rs` — Snapshots y rollback
- `snapshot()`: `git add -A && git commit -q -m "snapshot: {label}"`, retorna hash
  - Si no hay repo git, lo inicializa con `user.email regista@pi.local`
- `rollback()`: `git reset --hard <hash>`
- `current_hash()`: `git rev-parse HEAD`

---

## 💡 Decisiones de diseño importantes

1. **Agnóstico al proyecto anfitrión**: el orquestador no sabe de Rust, cargo, ni Purist.  
   Solo invoca `pi --skill <path>` con prompts genéricos.

2. **Workflow fijo e inmutable**: las 12 transiciones en `Status::ALL` son canónicas.  
   No se añaden transiciones en runtime **por diseño**.

3. **Shell `true` en hooks**: `hooks.rs` ejecuta con `sh -c`, igual que el wrapper original en bash.

4. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos (`delay *= 2`).

5. **`set_status()` con backup atómico**: escribe → re-parsea → si falla, restaura `.bak`.

6. **Prioridad de estados en el loop**: `BusinessReview` > `InReview` > `InProgress` > `TestsReady` > `Ready`.  
   Esto asegura que historias casi terminadas se completen antes de empezar nuevas.

7. **Git opcional pero auto-inicializable**: si `git.enabled = true` y no hay repo, lo crea.

8. **Daemon mode vía respawn**: `--detach` spawnea un proceso hijo con `--daemon` interno,  
   evitando dependencias nativas de `fork()`. El estado se guarda en `.regista.pid` (TOML).

9. **Detección de QA fix**: cuando el último actor en el Activity Log es "Dev" estando en  
   `TestsReady`, el orquestador dispara QA para corregir tests en vez de Dev para implementar.

---

## 🚧 Pendiente y roadmap

### Alta prioridad (completado ✅)
1. ~~Filtro `--story`~~ — implementado: `RunOptions.story_filter` en `filter_stories()`
2. ~~Filtro `--epics` / `--epic`~~ — implementado: `RunOptions.epic_filter` y `RunOptions.epics_range`
3. ~~Flag `--once`~~ — implementado: break al final de la primera iteración
4. ~~`--log-file`~~ — implementado: redirige tracing a archivo si se especifica

### Media prioridad (completado ✅)
4. ~~Daemon mode~~ — implementado: `daemon.rs` con `--detach`, `--follow`, `--status`, `--kill`
5. ~~`--log-file`~~ — ya estaba implementado en la sesión anterior

### Baja prioridad (completado ✅)
6. ~~`TestsReady → TestsReady`~~ — implementado: `process_story()` detecta `last_actor() == "Dev"` y dispara QA fix
7. ~~Limpiar dead code warnings~~ — 0 warnings. Añadidos `#[allow(dead_code)]` en API pública.

### Pendiente real

---

## 🧪 Estrategia de testing

- **Tests unitarios**: cada módulo tiene `#[cfg(test)] mod tests` con fixtures inline
- **Fixtures**: `tests/fixtures/` contiene archivos .md de ejemplo para pruebas de parseo
- **Test ignorado**: `agent::tests::invoke_with_retry_fails_when_pi_not_installed` (requiere `pi` en PATH)
- **Total**: 71 tests pasando, 0 fallos, 1 ignorado (al momento del handoff)

Para añadir tests:
- Usa `make_story()` helper (definido en varios módulos) para crear Stories sintéticas
- No dependas de archivos reales salvo en tests de `story.rs` (que usan fixtures)
- Para nuevos tests de parseo, añade fixtures en `tests/fixtures/`

---

## ⚠️ Errores comunes y anti-patrones

- ❌ **Añadir transiciones a `Status::ALL`**: rompe la inmutabilidad del workflow.  
  Las 12 transiciones son el contrato fijo.

- ❌ **Parsear historias sin usar `extract_section()`**: reinventar el parseo de secciones  
  markdown lleva a bugs. Usa las funciones existentes en `story.rs`.

- ❌ **Modificar `raw_content` sin actualizar `status`**: si cambias el contenido en crudo,  
  asegúrate de que `self.status` y `self.raw_content` estén sincronizados.

- ❌ **Ejecutar hooks sin `sh -c`**: los hooks son comandos shell. Si los ejecutas como  
  binario directo, fallarán para pipelines y redirects.

- ❌ **Asumir que todos los bloqueadores existen**: una historia puede referenciar un  
  STORY-XXX que no está en el filesystem. `dependency_graph.rs` lo maneja, pero  
  `deadlock.rs` debe filtrar con `status_map.get()`.

---

## 🔑 Convenciones de código

- **Idioma**: código y comentarios en español, nombres de variables/funciones en inglés
- **Formato**: `cargo fmt` (rustfmt estándar)
- **Documentación**: `//!` para módulos, `///` para items públicos
- **Errores**: `anyhow::Result<T>` y `anyhow::bail!()` (nunca `unwrap()` en lógica de negocio)
- **Logging**: `tracing::info!()` / `warn!()` / `error!()` / `debug!()` (nunca `println!`)
- **Regex estáticos**: usa `LazyLock<Regex>` para compilar una sola vez
- **Defaults de serde**: `#[serde(default)]` + funciones `default_xxx()` (no `Default::default()` en attributes)
- **Tests**: usa `assert!()` / `assert_eq!()` con mensajes descriptivos; evita `unwrap()` en tests
