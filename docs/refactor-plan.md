# Plan de refactor — arquitectura en capas

> Brújula: `cargo test --test architecture architecture_layers_are_respected`  
> Objetivo: que ese test pase a verde.

## Estado inicial

```
src/
├── main.rs          (1335 líneas — CLI + handlers + helpers + tests)
├── state.rs         → target: domain/
├── story.rs         → target: domain/
├── dependency_graph.rs → target: domain/
├── deadlock.rs      → target: domain/
├── prompts.rs       → target: domain/       ⚠️ violación R1
├── providers.rs     → target: infra/
├── agent.rs         → target: infra/
├── daemon.rs        → target: infra/
├── checkpoint.rs    → target: infra/
├── git.rs           → target: infra/
├── hooks.rs         → target: infra/
├── orchestrator.rs  → target: app/
├── plan.rs          → target: app/
├── validator.rs     → target: app/
├── init.rs          → target: app/
├── board.rs         → target: app/
├── update.rs        → target: app/
└── config.rs        → se queda en raíz      ⚠️ violación R2
```

## Fase 0 — Crear estructura de directorios

```bash
mkdir -p src/cli src/app src/domain src/infra
```

Archivos a crear:
- `src/cli/mod.rs`
- `src/app/mod.rs`
- `src/domain/mod.rs`
- `src/infra/mod.rs`

Cada `mod.rs` por ahora solo re-exporta los módulos que va a contener (se rellena en fase 2).

---

## Fase 1 — Mover módulos a sus capas

Mover SIN modificar contenido:

| Archivo origen | Archivo destino |
|----------------|-----------------|
| `src/state.rs` | `src/domain/state.rs` |
| `src/story.rs` | `src/domain/story.rs` |
| `src/dependency_graph.rs` | `src/domain/graph.rs` |
| `src/deadlock.rs` | `src/domain/deadlock.rs` |
| `src/prompts.rs` | `src/domain/prompts.rs` |
| `src/providers.rs` | `src/infra/providers.rs` |
| `src/agent.rs` | `src/infra/agent.rs` |
| `src/daemon.rs` | `src/infra/daemon.rs` |
| `src/checkpoint.rs` | `src/infra/checkpoint.rs` |
| `src/git.rs` | `src/infra/git.rs` |
| `src/hooks.rs` | `src/infra/hooks.rs` |
| `src/orchestrator.rs` | `src/app/pipeline.rs` |
| `src/plan.rs` | `src/app/plan.rs` |
| `src/validator.rs` | `src/app/validate.rs` |
| `src/init.rs` | `src/app/init.rs` |
| `src/board.rs` | `src/app/board.rs` |
| `src/update.rs` | `src/app/update.rs` |

Verificación: `cargo check` fallará (faltan los `mod`).

---

## Fase 2 — Rellenar `mod.rs` de cada capa

### `src/domain/mod.rs`
```rust
pub mod deadlock;
pub mod graph;
pub mod prompts;
pub mod state;
pub mod story;
```

### `src/infra/mod.rs`
```rust
pub mod agent;
pub mod checkpoint;
pub mod daemon;
pub mod git;
pub mod hooks;
pub mod providers;
```

### `src/app/mod.rs`
```rust
pub mod board;
pub mod init;
pub mod pipeline;
pub mod plan;
pub mod update;
pub mod validate;
```

### `src/cli/mod.rs`
```rust
pub mod args;
pub mod handlers;
```

### `src/main.rs`
Actualizar declaraciones `mod`:
```rust
mod cli;
mod app;
mod domain;
mod infra;
mod config;
```

Verificación: `cargo check` — esperamos errores de `use crate::...` (las rutas cambiaron).

---

## Fase 3 — Actualizar imports en todos los archivos

Esta es la fase más laboriosa. Para cada archivo movido, cambiar `use crate::X` por la nueva ruta.

### Mapeo de imports

| Ruta antigua | Ruta nueva |
|-------------|-----------|
| `use crate::state::...` | `use crate::domain::state::...` |
| `use crate::story::...` | `use crate::domain::story::...` |
| `use crate::dependency_graph::...` | `use crate::domain::graph::...` |
| `use crate::deadlock::...` | `use crate::domain::deadlock::...` |
| `use crate::prompts::...` | `use crate::domain::prompts::...` |
| `use crate::providers::...` | `use crate::infra::providers::...` |
| `use crate::agent::...` | `use crate::infra::agent::...` |
| `use crate::daemon::...` | `use crate::infra::daemon::...` |
| `use crate::checkpoint::...` | `use crate::infra::checkpoint::...` |
| `use crate::git::...` | `use crate::infra::git::...` |
| `use crate::hooks::...` | `use crate::infra::hooks::...` |
| `use crate::orchestrator::...` | `use crate::app::pipeline::...` |
| `use crate::plan::...` | `use crate::app::plan::...` |
| `use crate::validator::...` | `use crate::app::validate::...` |
| `use crate::init::...` | `use crate::app::init::...` |
| `use crate::board::...` | `use crate::app::board::...` |
| `use crate::update::...` | `use crate::app::update::...` |

### Archivos que más cambios necesitan

| Archivo | Imports a actualizar (~estimado) |
|---------|-------------------------------|
| `src/app/pipeline.rs` | ~15 |
| `src/app/plan.rs` | ~10 |
| `src/app/validate.rs` | ~8 |
| `src/app/board.rs` | ~6 |
| `src/domain/prompts.rs` | ~3 |
| `src/infra/agent.rs` | ~3 |
| resto | 1-3 cada uno |

Verificación: `cargo check` — debe compilar limpio.

---

## Fase 4 — Extraer handlers de `main.rs` a `cli/handlers.rs`

Mover estas funciones desde `main.rs`:
- `handle_plan()`
- `handle_auto()`
- `handle_run()`
- `handle_logs()`
- `handle_status()`
- `handle_kill()`
- `handle_validate()`
- `handle_init()`
- `handle_update()`
- `handle_board()`

Y sus helpers:
- `setup_daemon_tracing()`
- `setup_user_tracing()`
- `load_config()`
- `build_run_options()`
- `build_daemon_args()`
- `spawn_and_optionally_follow()`
- `print_pipeline_summary()`
- `exit_code_from_report()`

Ajustar imports dentro de handlers: `use crate::app::pipeline` en vez de `use crate::orchestrator`, etc.

`main.rs` tras Fase 4 (~30 líneas):
```rust
mod cli;
mod app;
mod domain;
mod infra;
mod config;

use clap::Parser;

fn main() {
    let cli = cli::args::Cli::parse();
    cli::handlers::dispatch(cli);
}
```

Verificación: `cargo build` y `cargo test --bins`.

---

## Fase 5 — Extraer Args de `main.rs` a `cli/args.rs`

Mover todos los structs `#[derive(Parser, Args)]`:
- `Cli`
- `Commands`
- `RepoArgs`
- `PlanModeArgs`
- `CommonArgs`
- `PipelineArgs`
- `DaemonArgs`
- `PlanArgs`, `AutoArgs`, `RunArgs`, `ValidateArgs`, `InitArgs`, `UpdateArgs`, `BoardArgs`

Los structs quedan públicos (`pub struct ...`).

`cli/handlers.rs` los importa con `use super::args::...`.

Verificación: `cargo build`.

---

## Fase 6 — Mover tests de CLI

Los tests que están en `src/main.rs` (`#[cfg(test)] mod tests`) se mueven a `src/cli/args.rs` como `#[cfg(test)] mod tests`.

Son tests de parseo de CLI (`try_parse_from`), deben vivir junto a los Args.

Los tests de helpers (`build_daemon_args`, `exit_code_from_report`) van en `src/cli/handlers.rs`.

Verificación: `cargo test --bins` — mismos 177 tests pasando.

---

## Fase 7 — Corregir violaciones de arquitectura

### Violación 1: `domain/prompts.rs` → `config`

**Problema**: `prompts.rs` importa `use crate::config::StackConfig` para el método `render()`.

**Solución**: Definir un struct equivalente en domain:

```rust
// src/domain/prompts.rs (o nuevo src/domain/stack.rs)
/// Representación de dominio de la configuración de stack.
/// No depende de config.rs — la aplicación llena esto desde Config.
pub struct DomainStackConfig {
    pub build: Option<String>,
    pub test: Option<String>,
    pub lint: Option<String>,
    pub fmt: Option<String>,
    pub src_dir: Option<String>,
}
```

`PromptContext` usa `DomainStackConfig` en vez de `StackConfig`.  
En `app/pipeline.rs`, al construir `PromptContext`, se convierte `Config.stack` → `DomainStackConfig`.

### Violación 2: `config.rs` → `providers`

**Problema**: `AgentsConfig::skill_for_role()` llama a `providers::from_name()`.

**Solución**: Mover `skill_for_role()` y `provider_for_role()` a `infra/providers.rs` (o a un nuevo `infra/resolver.rs`). Estos métodos toman `&AgentsConfig` y devuelven el provider/instrucción. `config.rs` queda como datos puros.

Alternativa más ligera: mover solo la validación a `app/validate.rs` y que `config.rs` no importe nada del crate.

Verificación: `cargo test --test architecture` → ✅ verde.

---

## Fase 8 — Verificación final

```bash
cargo test --bins          # 177 tests unitarios
cargo test --test architecture  # architecture_layers_are_respected ✅
cargo build --release      # compilación release
cargo clippy -- -D warnings
cargo fmt -- --check
```

---

## Resumen de comandos por fase

| Fase | Comando de verificación | Resultado esperado |
|------|------------------------|-------------------|
| 0 | `ls src/cli src/app src/domain src/infra` | 4 directorios |
| 1 | `ls src/domain/state.rs src/infra/providers.rs ...` | 17 archivos movidos |
| 2 | `cargo check` | errores de imports (esperado) |
| 3 | `cargo check` | compila limpio |
| 4 | `cargo build` | compila, main.rs ~30 líneas |
| 5 | `cargo build` | compila |
| 6 | `cargo test --bins` | 177 passed |
| 7 | `cargo test --test architecture` | 1 passed (verde) |
| 8 | `cargo test && cargo clippy` | todo limpio |

## Tiempo estimado total: ~1.5 horas

La fase 3 (actualizar imports) es la más larga (~30 min). El resto son movimientos mecánicos.
