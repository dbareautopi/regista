# 🧠 regista — Session Handoff

> **Fecha**: 2026-05-05
> **Sesión**: v0.9.0 — Reestructuración arquitectónica, migración a tokio, Workflow trait, Health metrics
> **Versión**: v0.9.0
> **Estado**: 357 tests pasando, 0 fallos, 1 ignorado, 0 warnings.

---

## 📍 Estructura actual

Arquitectura en **4 capas** con dependencias unidireccionales verificadas por `tests/architecture.rs`:

```
/root/repos/regista/
├── Cargo.toml                 ← v0.9.0
├── Cargo.lock
├── README.md                  ← Actualizado
├── DESIGN.md                  ← Actualizado
├── AGENTS.md                  ← Actualizado (nueva arquitectura)
├── HANDOFF.md                 ← Este documento
├── .gitignore
├── src/
│   ├── main.rs                ← entry point: mod app, cli, config, domain, infra
│   ├── config.rs              ← Config, AgentsConfig + AgentRoleConfig, StackConfig, carga TOML
│   │
│   ├── cli/                   ← 🟢 CLI: args + handlers (capa exterior)
│   │   ├── args.rs            ← Cli, Commands (Plan/Auto/Run/Logs/Status/Kill/Validate/Init/Update/Board)
│   │   └── handlers.rs        ← dispatch(), handlers, daemon, tracing, exit codes
│   │
│   ├── app/                   ← 🟡 Aplicación: casos de uso (importa domain + infra + config)
│   │   ├── board.rs           ← Dashboard Kanban: conteo por estado, bloqueadas/fallidas, --json, --epic
│   │   ├── health.rs          ← HealthReport: métricas pipeline (iteraciones/hora, coste, tasa rechazo)
│   │   ├── init.rs            ← Scaffolding multi-provider con skills inline (YAML frontmatter)
│   │   ├── pipeline.rs        ← Loop principal (async): run_real(), run_dry(), process_story()
│   │   ├── plan.rs            ← Generación de backlog + bucle plan→validate
│   │   ├── update.rs          ← Auto-update desde crates.io
│   │   └── validate.rs        ← Chequeo pre-vuelo: config, skills, providers (PATH), historias, git
│   │
│   ├── domain/                ← 🔴 Dominio: lógica pura (NO importa otras capas)
│   │   ├── state.rs           ← Status, Actor, Transition (14), SharedState (Arc<RwLock<>>)
│   │   ├── story.rs           ← Story, parseo .md, set_status() atómico, advance_status_in_memory()
│   │   ├── graph.rs           ← DependencyGraph, DFS ciclos, blocks_count()
│   │   ├── deadlock.rs        ← analyze(), DeadlockResolution, priorización
│   │   ├── prompts.rs         ← PromptContext, DomainStackConfig, 7 prompts stack-agnósticos
│   │   └── workflow.rs        ← Trait Workflow + CanonicalWorkflow (extensible para #04)
│   │
│   └── infra/                 ← 🔵 Infraestructura: I/O, procesos (importa solo config)
│       ├── providers.rs       ← trait AgentProvider + Pi/ClaudeCode/Codex/OpenCode + factory
│       ├── agent.rs           ← invoke_with_retry() async, tokio::process::Command, feedback
│       ├── checkpoint.rs      ← OrchestratorState: save/load/remove (.regista/state.toml)
│       ├── daemon.rs          ← detach(), status(), kill(), follow(), PidCleanup
│       ├── git.rs             ← snapshot(), rollback() con spawn_blocking
│       └── hooks.rs           ← run_hook(): comandos shell con tokio::process::Command
│
├── tests/
│   ├── architecture.rs        ← 11 tests: verifica reglas R1-R5 de dependencias entre capas
│   └── fixtures/
│       ├── story_draft.md
│       ├── story_blocked.md
│       └── story_business_review.md
│
└── roadmap/
    ├── ROADMAP.md
    ├── 01-paralelismo.md
    ├── 04-workflow-configurable.md
    └── ...
```

---

## 🆕 Novedades en v0.9.0 (desde v0.7.2)

### 🏗️ Reestructuración arquitectónica (commit `245065e`)

Migración de estructura plana a **arquitectura en capas**:

| Antes (v0.7.x) | Ahora (v0.9.0) |
|---|---|
| `src/state.rs` | `src/domain/state.rs` |
| `src/story.rs` | `src/domain/story.rs` |
| `src/dependency_graph.rs` | `src/domain/graph.rs` |
| `src/deadlock.rs` | `src/domain/deadlock.rs` |
| `src/prompts.rs` | `src/domain/prompts.rs` |
| — | `src/domain/workflow.rs` ✨ nuevo |
| `src/providers.rs` | `src/infra/providers.rs` |
| `src/agent.rs` | `src/infra/agent.rs` |
| `src/checkpoint.rs` | `src/infra/checkpoint.rs` |
| `src/daemon.rs` | `src/infra/daemon.rs` |
| `src/git.rs` | `src/infra/git.rs` |
| `src/hooks.rs` | `src/infra/hooks.rs` |
| `src/orchestrator.rs` | `src/app/pipeline.rs` |
| `src/plan.rs` | `src/app/plan.rs` |
| `src/validator.rs` | `src/app/validate.rs` |
| `src/init.rs` | `src/app/init.rs` |
| `src/board.rs` | `src/app/board.rs` |
| `src/update.rs` | `src/app/update.rs` |
| — | `src/app/health.rs` ✨ nuevo |
| — | `src/cli/args.rs` ✨ nuevo (extraído de main.rs) |
| — | `src/cli/handlers.rs` ✨ nuevo (extraído de main.rs) |
| — | `tests/architecture.rs` ✨ nuevo |

**Reglas verificadas automáticamente** (11 tests):
- **R1**: `domain/` solo importa std + crates externos
- **R2**: `infra/` solo importa `config` + otros módulos `infra/`
- **R3**: `app/` solo importa `domain/` + `infra/` + `config`
- **R4**: `cli/` puede importar cualquier capa
- **R5**: `config` no importa nada del crate

### ⚡ Migración a tokio (async/await)

- `agent.rs`: `invoke_with_retry()` → `async fn` con `tokio::time::sleep`
- `invoke_once()` usa `tokio::process::Command` + `tokio::time::timeout`
- Timeout real mata el proceso por PID (zero zombies)
- `save_agent_decision()` usa `tokio::fs`
- `pipeline.rs`: `run_real()` y `process_story()` → `async fn`
- Git y hooks usan `spawn_blocking()` para no bloquear el runtime
- `RUNTIME`: `LazyLock<tokio::runtime::Runtime>` global para callers síncronos
- `invoke_with_retry_blocking()`: wrapper síncrono para `plan.rs`

### 🧩 Nuevo: `domain/workflow.rs` — Trait `Workflow`

```rust
pub trait Workflow: Sync {
    fn next_status(&self, current: Status) -> Status;
    fn map_status_to_role(&self, status: Status) -> &'static str;
    fn canonical_column_order(&self) -> &[&'static str];
}
```

- `CanonicalWorkflow` implementa las 14 transiciones canónicas
- `pipeline.rs` usa `&dyn Workflow` en lugar de funciones hardcodeadas
- `board.rs` usa `workflow.canonical_column_order()` para columnas dinámicas
- Prepara el terreno para #04 (workflows configurables)
- 35 tests dedicados

### 🔒 Nuevo: `SharedState` en `domain/state.rs`

- `Arc<RwLock<HashMap<String, u32>>>` para `reject_cycles`, `story_iterations`
- `Arc<RwLock<HashMap<String, String>>>` para `story_errors`
- `Clone` comparte el mismo `Arc` — seguro entre tareas
- `read()` concurrente, `write()` exclusivo
- Prepara el terreno para #01 (paralelismo con `tokio::spawn`)

### 📊 Nuevo: `app/health.rs` — Health & Metrics

- `HealthReport`: iteraciones/hora, tiempo medio agente, tasa rechazo, throughput, coste
- `generate_report()`: cálculo puro desde datos crudos
- `is_health_checkpoint()`: disparo cada N iteraciones (default 10)
- `write_health_json()`: escritura atómica `.tmp → rename`
- 19 tests

### 🔧 Mejoras en providers

**OpenCodeProvider**:
- Usa `opencode run --agent <name> --dangerously-skip-permissions` (API corregida)
- Soporte `-m <model>` leído del YAML frontmatter del archivo de instrucción
- `read_yaml_field()` para parsear frontmatter
- Windows: wrapper con `powershell.exe` + escapado correcto (`"`, `$`, `` ` ``)

**CodexProvider**:
- `validate_providers()` genera Warning (no Error) si codex no está en PATH
  (puede instalarse vía npm con nombres no estándar)

### 🧪 Tests de arquitectura (`tests/architecture.rs`)

- 11 tests que verifican dependencias entre capas
- Extrae imports `use crate::X` y verifica contra reglas R1-R5
- Omite imports dentro de `#[cfg(test)]` (deps de test son libres)
- Funciona tanto con estructura plana (legacy) como con directorios (target)

### 🔢 Otras mejoras

- `max_reject_cycles`: 3 → **8** (más tolerante a iteraciones)
- Skills inline en `init.rs` incluyen YAML frontmatter completo (`name`, `model`, `description`)
- `validate_providers()`: verifica binarios de todos los providers en PATH
- `board.rs`: columnas dinámicas según `workflow.canonical_column_order()`, omite vacías
- `qa_tests()`: prompt incluye reglas estrictas (NO crear módulos, NO implementar, solo tests)
- `dev_implement()`: prompt incluye manejo de tests rotos (reportar, no corregir)

---

## ⚙️ Funcionalidades implementadas

### Pipeline base
- Loop principal async con máquina de estados (Draft → Ready → Tests Ready → In Review → Business Review → Done)
- 14 transiciones canónicas (11 por agentes + 3 automáticas)
- Detección de deadlocks con priorización (mayor desbloqueo → menor ID)
- QA fix automático: si Dev reporta tests rotos, se dispara QA en vez de Dev
- `SharedState` con `Arc<RwLock<>>` para estado compartido entre tareas

### Subcomandos

| Comando | Módulo | Función |
|---------|--------|---------|
| `regista plan <spec>` | `app/plan.rs` | Generar historias desde una especificación (daemon) |
| `regista auto <spec>` | `cli/handlers.rs` | `plan` + `run` en un solo paso (daemon) |
| `regista run [dir]` | `app/pipeline.rs` | Pipeline sobre historias existentes (daemon) |
| `regista logs [dir]` | `infra/daemon.rs` | Ver el log del daemon en vivo (`tail -f`) |
| `regista status [dir]` | `infra/daemon.rs` | Consultar si el daemon está corriendo |
| `regista kill [dir]` | `infra/daemon.rs` | Detener el daemon |
| `regista validate [dir]` | `app/validate.rs` | Chequeo pre-vuelo (config, skills, providers, historias, git) |
| `regista init [dir]` | `app/init.rs` | Scaffolding multi-provider: config + instrucciones + dirs |
| `regista update` | `app/update.rs` | Comprobar e instalar nueva versión desde crates.io |
| `regista board [dir]` | `app/board.rs` | Dashboard Kanban: conteo por estado, bloqueadas/fallidas |

> **Nota**: `plan`, `auto`, `run` ejecutan en modo daemon. Usa `--logs` para seguir el progreso.

### Flags comunes (plan / auto / run)

| Flag | Descripción |
|------|-------------|
| `--logs` | Hacer `tail -f` del log tras lanzar el daemon |
| `--dry-run` | Simulación síncrona (sin agentes, sin coste) |
| `--config <path>` | Ruta alternativa al archivo `.regista/config.toml` |
| `--provider <NAME>` | Seleccionar provider (pi, claude, codex, opencode) |
| `--quiet` | Suprimir logs de progreso (solo errores) |

### Flags de pipeline (auto / run)

| Flag | Descripción |
|------|-------------|
| `--story <ID>` | Filtrar por historia (STORY-001) |
| `--epic <ID>` | Filtrar por épica (EPIC-001) |
| `--epics <RANGE>` | Filtrar por rango de épicas (EPIC-001..EPIC-003) |
| `--once` | Una sola iteración del pipeline |
| `--resume` | Reanudar desde el último checkpoint |
| `--clean-state` | Borrar el checkpoint antes de arrancar |

### Flags de plan / auto

| Flag | Descripción |
|------|-------------|
| `--replace` | Reemplazar historias existentes (modo destructivo) |
| `--max-stories N` | Límite de historias a generar (0 = sin límite) |

### Flags de init

| Flag | Descripción |
|------|-------------|
| `--light` | Solo `.regista/config.toml`, sin instrucciones de rol |
| `--with-example` | Incluir historia y épica de ejemplo |
| `--provider <NAME>` | Provider de agente (default: pi) |

### Flags de validate

| Flag | Descripción |
|------|-------------|
| `--json` | Salida JSON para CI/CD |
| `--config <path>` | Ruta alternativa al archivo `.regista/config.toml` |
| `--provider <NAME>` | Provider de agente |

### Flags de board

| Flag | Descripción |
|------|-------------|
| `--json` | Salida JSON para CI/CD |
| `--epic <ID>` | Filtrar por épica |
| `--config <path>` | Ruta alternativa al archivo `.regista/config.toml` |

---

## 🔨 Comandos esenciales

```bash
cargo build              # Debug
cargo build --release    # Release
cargo test               # 357 tests: 346 lib + 11 architecture
cargo test --lib domain  # Solo tests de dominio
cargo clippy -- -D warnings  # 0 issues
cargo fmt --check        # 0 issues
cargo test --test architecture  # Solo tests de arquitectura
```

---

## 🧩 Contrato de historia (.md)

```markdown
# STORY-NNN: Título

## Status
**Draft**

## Epic
EPIC-XXX

## Descripción
...

## Criterios de aceptación
- [ ] CA1: específico y verificable
- [ ] CA2: ...

## Dependencias
- Bloqueado por: STORY-XXX

## Activity Log
- YYYY-MM-DD | Actor | descripción
```

---

## 🚧 Pendiente (roadmap)

### Media prioridad
- **10 - Cross-story context**: agentes reciben contexto de historias relacionadas (Fase 4)
- **04 - Workflow configurable**: transiciones definibles en `.regista/config.toml` (Fase 5)
- **11 - TUI / dashboard**: visualización en vivo del progreso (Fase 6)
- **12 - Cost tracking**: límite de gasto en llamadas LLM (Fase 6)
- **01 - Paralelismo**: ejecutar historias independientes simultáneamente (Fase 7, ÚLTIMO)

### Variantes de plan
- **14 - `plan --from-dir`**: múltiples documentos fuente (Fase 3)
- **15 - `plan --interactive`**: PO entrevista al usuario (Fase 6)

---

## 🔑 Decisiones de diseño

1. **Arquitectura en capas**: `cli → app → domain/infra → config`.  
   `tests/architecture.rs` verifica automáticamente.

2. **Agnóstico al proyecto**: regista no sabe de Rust, cargo, ni nada.

3. **Workflow fijo e inmutable**: 14 transiciones canónicas.

4. **Trait `AgentProvider` devuelve `Vec<String>`**: compatible sync/async.

5. **Async runtime tokio**: `agent.rs` y `pipeline.rs` usan async/await.  
   Timeout real mata procesos por PID.

6. **`SharedState` con `Arc<RwLock<>>`**: preparado para paralelismo (#01).

7. **Trait `Workflow`**: abstrae máquina de estados para workflows configurables (#04).

8. **Shell `true` en hooks**: `hooks.rs` ejecuta con `sh -c`.

9. **Backoff exponencial**: `agent.rs` duplica delay entre reintentos.

10. **`set_status()` con backup atómico**: write → verify → restore .bak si falla.

11. **CLI con clap Subcommand**: 10 subcomandos, `--version`/`--help` nativos.

12. **Dry-run en memoria**: `advance_status_in_memory()` sin tocar filesystem.

13. **Plan con bucle validate**: feedback concreto de errores de dependencias.

14. **Checkpoint TOML**: mismo formato que el proyecto, legible.

15. **Feedback truncado a 2000 bytes**: no desborda ventana de contexto.

16. **`max_iterations = 0`**: auto-escala a `max(10, historias × 6)`.

17. **`max_reject_cycles = 8`**: más tolerante que el anterior 3.

18. **Provider por defecto `"pi"`**: retrocompatibilidad total.

19. **Skills inline con YAML frontmatter**: `name`, `model`, `description`.
