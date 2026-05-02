# 01 — Refactor CLI: subcomandos + 100% daemon

> **Fecha**: 2026-05-02
> **Estado**: ✍️ Especificación
> **Esfuerzo**: Alto
> **Dependencias**: Ninguna (es refactor del entry point)

---

## 🎯 Objetivo

Rediseñar la CLI de `regista` con dos cambios de paradigma:

1. **100% daemon**: toda ejecución de pipeline spawnea un proceso en background.
   No existe el modo bloqueante. Si el usuario quiere ver el progreso, usa
   `--logs` (tail del log en vivo, cancelable con Ctrl+C sin matar el daemon).

2. **Subcomandos con clap derive**: se abandona la detección manual de
   subcomandos vía `std::env::args()`. Se migra a `#[derive(Subcommand)]`
   de clap, que es más mantenible y genera `--help` automático por subcomando.

---

## ❓ Problema actual

### CLI actual (v0.3.0)

```
regista [DIR] [flags]                  ← pipeline (bloqueante por defecto)
regista --detach [flags]               ← pipeline en background (opt-in)
regista --follow                       ← tail del log de un daemon existente
regista validate [DIR] --json ...      ← validación (manual args)
regista init [DIR] --light ...         ← scaffolding (manual args)
regista groom <SPEC> --run ...         ← groom + pipeline (manual args)
regista help                           ← ayuda (manual args)
```

Problemas:

| Problema | Detalle |
|----------|---------|
| **Bloqueante por defecto** | Un pipeline puede durar horas (muchas llamadas LLM). El usuario se queda atado a la terminal. |
| **`--detach` es opt-in** | Va contra la naturaleza de la herramienta (orquestador autónomo). Debería ser fuego-y-olvido. |
| **`groom --run` es bloqueante** | Peor aún: groom + pipeline. El usuario espera el doble. |
| **`--follow` ambiguo** | ¿Seguir el log O ejecutar en primer plano? El nombre no ayuda. |
| **Args manuales** | `validate`, `init`, `groom` se detectan inspeccionando `std::env::args()` antes de clap. Frágil, sin ayuda automática, difícil de extender. |
| **`regista` a secas ejecuta** | Sin subcomando, `regista .` lanza el pipeline. Poco ortogonal. |

---

## ✅ Solución propuesta

### Nuevo CLI

```
regista <subcomando> [args]

Subcomandos de pipeline (daemon):
  plan      <spec> [flags]   Generar historias desde spec
  auto      <spec> [flags]   Generar historias + ejecutar pipeline
  run                [flags] Ejecutar pipeline sobre historias existentes

Subcomandos de gestión del daemon:
  logs      [dir]            Tail del log del daemon en vivo
  status    [dir]            Consultar si el daemon está corriendo
  kill      [dir]            Detener el daemon

Subcomandos auxiliares:
  validate  [dir] [flags]    Validar configuración e historias
  init      [dir] [flags]    Inicializar estructura del proyecto
```

`regista` sin subcomando → **error**, muestra ayuda automática de clap.

### Principios

| Principio | Cómo se cumple |
|-----------|----------------|
| Siempre daemon | `plan`, `auto`, `run` spawnean proceso hijo con `--daemon`. El padre imprime PID y sale. |
| Cero bloqueante | No existe modo foreground. Si quieres ver progreso: `--logs`. |
| `--logs` no bloquea el daemon | `--logs` hace tail del archivo de log. Si el usuario hace Ctrl+C, el daemon **sigue corriendo**. |
| `--dry-run` síncrono | Es rápido (sin agentes, sin coste). No necesita daemon. |
| `--json` deprecado | Se ignora con warning. Se rediseñará en otra spec (p.ej. escribiendo `.regista/report.json` al terminar el daemon). |

---

## 📋 Subcomandos en detalle

### `regista plan` — Generar backlog

```
regista plan <spec> [flags]

Argumentos:
  <spec>               Ruta al archivo de especificación (obligatorio)

Flags:
  --replace            Modo destructivo: borrar historias/épicas existentes antes de generar.
                       Sin este flag, el comportamiento por defecto es merge (añade sin borrar).
  --max-stories <N>    Límite de historias a generar. 0 = sin límite. Default: 0.
  --logs               Tras spawnear el daemon, hacer tail del log en vivo.
  --dry-run            Ejecutar groom de forma síncrona (sin daemon). Útil para debugear la spec.
  --config <PATH>      Ruta al archivo .regista/config.toml.
  --provider <NAME>    Provider para el rol Product Owner (pi, claude, codex, opencode).
  --quiet              Suprimir logs de progreso (solo errores).

  --daemon             [OCULTO] Flag interno: indica que este proceso es el hijo daemon.
  --log-file <PATH>    [OCULTO] Ruta al archivo de log del daemon.
```

**Comportamiento:**

```
regista plan spec.md
│
├─ ¿--dry-run?
│   └─ SÍ → groom::run() síncrono → imprimir resultado → exit
│
└─ default (daemon):
    1. Construir child_args para el proceso hijo:
       ["plan", "spec.md", "--daemon", "--log-file", ".regista/daemon.log",
        flags: --replace, --max-stories, --config, --provider, --quiet]
       (NUNCA incluir: --logs, --dry-run)
    2. daemon::detach(project_dir, &child_args) → PID
    3. Imprimir: "🚀 Daemon PID: 12345  |  Log: .regista/daemon.log"
    4. ¿--logs? → daemon::follow(project_dir)
```

**Proceso hijo** (`--daemon`):

```
regista plan spec.md --daemon --log-file .regista/daemon.log ...
│
└─ groom::run(project_root, spec_path, &cfg, max_stories, replace)
   → escribe historias en .regista/stories/
   → escribe épicas en .regista/epics/
   → exit (el daemon termina)
```

**Notas:**
- El groom se ejecuta con el bucle de validación de dependencias existente (`groom_max_iterations`).
- Si el grafo de dependencias queda sucio tras `groom_max_iterations`, el daemon sale con warning en el log.
- No se lanza el pipeline automáticamente (solo planifica). Para eso está `auto`.
- El flag `--replace` mapea al parámetro `replace: bool` de `groom::run()`.

---

### `regista auto` — Planificar + ejecutar (full auto)

```
regista auto <spec> [flags]

Argumentos:
  <spec>               Ruta al archivo de especificación (obligatorio)

Flags de planificación (hereda de plan):
  --replace, --max-stories <N>

Flags de pipeline (hereda de run):
  --story <ID>         Filtrar por historia
  --epic <ID>          Filtrar por épica
  --epics <RANGO>      Filtrar por rango (EPIC-001..EPIC-003)
  --once               Una sola iteración del pipeline
  --resume             Reanudar desde checkpoint anterior
  --clean-state        Borrar checkpoint antes de arrancar

Flags comunes:
  --logs, --dry-run, --config <PATH>, --provider <NAME>, --quiet

  --daemon, --log-file [OCULTOS]
```

**Comportamiento:**

```
regista auto spec.md
│
├─ ¿--clean-state? → borrar checkpoint
│
├─ ¿--dry-run?
│   └─ SÍ → groom::run() síncrono → orchestrator::run() dry → imprimir → exit
│
└─ default (daemon):
    1. child_args = ["auto", "spec.md", "--daemon", "--log-file", "...",
        flags: --replace, --max-stories, --story, --epic, --epics, --once,
               --config, --provider, --quiet, --resume]
       (NUNCA: --logs, --dry-run, --clean-state)
    2. daemon::detach(...) → PID
    3. Imprimir PID + log
    4. ¿--logs? → daemon::follow(...)
```

**Proceso hijo** (`--daemon`):

```
regista auto spec.md --daemon --log-file ... [flags]
│
├─ groom::run(..., replace, max_stories)
│   └─ Si falla o no genera historias → exit con error
│
└─ orchestrator::run(project_root, &cfg, &run_options, resume_state)
   → pipeline completo → exit
```

**Notas:**
- Si el groom no genera historias o el grafo queda sucio, **no** se ejecuta el pipeline.
- El `resume_state` se carga si `--resume` está presente.
- Las flags `--story`, `--epic`, `--epics`, `--once` aplican **solo al pipeline**, no al groom.

---

### `regista run` — Ejecutar pipeline

```
regista run [dir] [flags]

Argumentos:
  [dir]                Directorio del proyecto. Default: "."

Flags de pipeline:
  --story <ID>         Filtrar por historia
  --epic <ID>          Filtrar por épica
  --epics <RANGO>      Filtrar por rango
  --once               Una sola iteración
  --resume             Reanudar desde checkpoint
  --clean-state        Borrar checkpoint antes de arrancar

Flags comunes:
  --logs, --dry-run, --config <PATH>, --provider <NAME>, --quiet

  --daemon, --log-file [OCULTOS]
```

**Comportamiento:**

```
regista run [dir]
│
├─ ¿--clean-state? → borrar checkpoint
│
├─ ¿--dry-run?
│   └─ SÍ → orchestrator::run() dry → imprimir → exit
│
└─ default (daemon):
    1. child_args = ["run", dir, "--daemon", "--log-file", "...",
        flags: --story, --epic, --epics, --once, --config, --provider, --quiet, --resume]
       (NUNCA: --logs, --dry-run, --clean-state)
    2. daemon::detach(...) → PID
    3. Imprimir PID + log
    4. ¿--logs? → daemon::follow(...)
```

**Proceso hijo** (`--daemon`):

```
regista run [dir] --daemon --log-file ... [flags]
│
└─ orchestrator::run(project_root, &cfg, &run_options, resume_state)
   → pipeline completo → exit
```

---

### `regista logs` — Tail del log

```
regista logs [dir]

Argumentos:
  [dir]                Directorio del proyecto. Default: "."
```

**Comportamiento:** `daemon::follow(dir)` — igual que el antiguo `--follow`.
Si no hay daemon corriendo (no existe `.regista/daemon.pid` o el PID no está vivo) → error descriptivo.

---

### `regista status` — Consultar daemon

```
regista status [dir]
```

**Comportamiento:** `daemon::status(dir)` → imprimir y salir.

Salidas posibles:
- `✅ Daemon corriendo (PID: 12345, log: .regista/daemon.log)`
- `❌ No se encontró archivo PID. El daemon no está corriendo.`
- `❌ PID 12345 ya no existe. Archivo PID huérfano limpiado.`

---

### `regista kill` — Detener daemon

```
regista kill [dir]
```

**Comportamiento:** `daemon::kill(dir)` → SIGTERM, esperar 2s, SIGKILL si necesario → imprimir y salir.

---

### `regista validate` — Sin cambios funcionales

```
regista validate [dir] [--json] [--config <PATH>] [--provider <NAME>]
```

Misma lógica que el `validate` actual, pero ahora como subcomando clap.

---

### `regista init` — Sin cambios funcionales

```
regista init [dir] [--light] [--with-example] [--provider <NAME>]
```

Misma lógica que el `init` actual, ahora como subcomando clap.

---

## 🔧 Cambios en `daemon.rs`

### Refactor de `detach()` — aceptar args explícitos

**Antes:**

```rust
pub fn detach(project_dir: &Path, log_file_override: Option<&Path>) -> anyhow::Result<u32> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    // filtra --detach, --follow, --status, --kill
    // añade --daemon, --log-file
    // spawn...
}
```

**Después:**

```rust
/// Lanza el orquestador en segundo plano (modo daemon).
///
/// `child_args` son los argumentos COMPLETOS que se pasarán al proceso hijo,
/// excluyendo el path del binario. Deben incluir `--daemon` y `--log-file`.
///
/// Ejemplo de child_args:
///   ["run", ".", "--daemon", "--log-file", ".regista/daemon.log", "--epic", "EPIC-001"]
pub fn detach(
    project_dir: &Path,
    child_args: &[String],
    log_file_override: Option<&Path>,
) -> anyhow::Result<u32> {
    let exe = std::env::current_exe()?;
    let canonical_project = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    // Determinar archivo de log
    let log_file = match log_file_override {
        Some(p) => p.to_path_buf(),
        None => canonical_project.join(".regista/daemon.log"),
    };

    // Crear directorio padre del log
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent)?;
    }

    // Crear/truncar archivo de log
    let log_handle = fs::File::create(&log_file)?;

    // Spawnear con los args proporcionados por el caller
    let child = Command::new(&exe)
        .args(child_args)
        .stdin(std::process::Stdio::null())
        .stdout(log_handle)
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let pid = child.id();

    // Guardar estado
    let state = DaemonState {
        pid,
        log_file: log_file.clone(),
        project_dir: canonical_project.clone(),
    };
    state.save(&canonical_project)?;

    Ok(pid)
}
```

**Ventajas:**
- El caller tiene control total sobre qué args recibe el hijo.
- La función ya no asume nada sobre la estructura de la CLI (subcomandos, flags).
- Compatible con `plan`, `auto`, `run` sin cambios adicionales.

### Sin cambios en:

- `DaemonState` y sus métodos (`save`, `load`, `remove`, `pid_file`)
- `PidCleanup`
- `status()`, `kill()`, `follow()`
- `is_process_alive()`, `send_signal()`, `drain_remaining()`

---

## 🧱 Reestructura de `main.rs` con clap derive

### Estructura de tipos

```rust
use clap::{Parser, Subcommand, Args};

#[derive(Parser)]
#[command(name = "regista", version, about = "🎬 AI agent director")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generar historias de usuario desde una especificación
    Plan(PlanArgs),
    /// Generar historias y ejecutar el pipeline completo
    Auto(AutoArgs),
    /// Ejecutar el pipeline sobre historias existentes
    Run(RunArgs),
    /// Ver el log del daemon en vivo
    Logs(RepoArgs),
    /// Consultar si el daemon está corriendo
    Status(RepoArgs),
    /// Detener el daemon
    Kill(RepoArgs),
    /// Validar la configuración y las historias
    Validate(ValidateArgs),
    /// Inicializar un nuevo proyecto
    Init(InitArgs),
}

// ── Args compartidos ────────────────────────────────────────────

/// Args para subcomandos que solo necesitan un directorio de proyecto.
#[derive(Args)]
struct RepoArgs {
    /// Directorio del proyecto (default: .)
    dir: Option<String>,
}

/// Args para el modo plan (groom).
#[derive(Args)]
struct PlanModeArgs {
    /// Archivo de especificación
    spec: String,

    /// Reemplazar historias existentes (modo destructivo)
    #[arg(long)]
    replace: bool,

    /// Máximo de historias a generar (0 = sin límite)
    #[arg(long, default_value = "0")]
    max_stories: u32,
}

/// Args comunes a todos los subcomandos de pipeline.
#[derive(Args)]
struct CommonArgs {
    /// Tail del log en vivo tras spawnear el daemon
    #[arg(long)]
    logs: bool,

    /// Simulación síncrona (sin agentes, sin coste)
    #[arg(long)]
    dry_run: bool,

    /// Ruta al archivo de configuración
    #[arg(long)]
    config: Option<String>,

    /// Provider de agente (pi, claude, codex, opencode)
    #[arg(long)]
    provider: Option<String>,

    /// Suprimir logs de progreso
    #[arg(long)]
    quiet: bool,
}

/// Args de pipeline (filtros, límites).
#[derive(Args)]
struct PipelineArgs {
    /// Filtrar por historia
    #[arg(long)]
    story: Option<String>,

    /// Filtrar por épica
    #[arg(long, conflicts_with = "epics")]
    epic: Option<String>,

    /// Filtrar por rango de épicas (EPIC-001..EPIC-003)
    #[arg(long, conflicts_with = "epic")]
    epics: Option<String>,

    /// Una sola iteración del pipeline
    #[arg(long)]
    once: bool,

    /// Reanudar desde checkpoint
    #[arg(long)]
    resume: bool,

    /// Borrar checkpoint antes de arrancar
    #[arg(long)]
    clean_state: bool,
}

/// Args internos del daemon (ocultos al usuario).
#[derive(Args)]
struct DaemonArgs {
    /// Flag interno: este proceso es el hijo daemon
    #[arg(long, hide = true)]
    daemon: bool,

    /// Archivo de log del daemon
    #[arg(long, hide = true)]
    log_file: Option<String>,
}

// ── Subcomandos concretos ───────────────────────────────────────

#[derive(Args)]
struct PlanArgs {
    #[command(flatten)]
    repo: RepoArgs,
    #[command(flatten)]
    plan_mode: PlanModeArgs,
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(Args)]
struct AutoArgs {
    #[command(flatten)]
    repo: RepoArgs,
    #[command(flatten)]
    plan_mode: PlanModeArgs,
    #[command(flatten)]
    pipeline: PipelineArgs,
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(Args)]
struct RunArgs {
    #[command(flatten)]
    repo: RepoArgs,
    #[command(flatten)]
    pipeline: PipelineArgs,
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(Args)]
struct ValidateArgs {
    #[command(flatten)]
    repo: RepoArgs,

    /// Salida JSON para CI/CD
    #[arg(long)]
    json: bool,

    /// Ruta al archivo de configuración
    #[arg(long)]
    config: Option<String>,

    /// Provider de agente
    #[arg(long)]
    provider: Option<String>,
}

#[derive(Args)]
struct InitArgs {
    #[command(flatten)]
    repo: RepoArgs,

    /// Solo generar .regista/config.toml, sin instrucciones de rol
    #[arg(long)]
    light: bool,

    /// Incluir historia y épica de ejemplo
    #[arg(long)]
    with_example: bool,

    /// Provider de agente (default: pi)
    #[arg(long, default_value = "pi")]
    provider: String,
}
```

### Lógica de `main()`

```rust
fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Plan(args)       => handle_plan(args),
        Commands::Auto(args)       => handle_auto(args),
        Commands::Run(args)        => handle_run(args),
        Commands::Logs(args)       => handle_logs(args),
        Commands::Status(args)     => handle_status(args),
        Commands::Kill(args)       => handle_kill(args),
        Commands::Validate(args)   => handle_validate(args),
        Commands::Init(args)       => handle_init(args),
    }
}
```

### Helpers de handlers

```rust
/// Resuelve el directorio del proyecto desde RepoArgs.
fn resolve_dir(repo: &RepoArgs) -> &Path {
    // RepoArgs.dir es Option<String>. Se resuelve aquí para no repetir.
    // ...
}

/// Carga la configuración, aplica --provider, configura logging.
fn bootstrap(args: &CommonArgs, project_root: &Path) -> anyhow::Result<Config> {
    // Configura tracing (respeta --quiet)
    // Carga Config::load()
    // Aplica provider override
    // ...
}

/// Construye `RunOptions` desde `PipelineArgs`.
fn build_run_options(args: &PipelineArgs, common: &CommonArgs) -> orchestrator::RunOptions {
    // Parsea --epics en rango
    // Mapea flags a RunOptions
    // ...
}

/// Construye los child_args para el daemon a partir de los flags activos.
fn build_daemon_args(
    subcommand: &str,           // "plan", "auto", o "run"
    project_dir: &str,
    spec: Option<&str>,         // solo para plan/auto
    replace: bool,
    max_stories: u32,
    pipeline: &PipelineArgs,
    common: &CommonArgs,
) -> Vec<String> {
    let mut args = vec![
        subcommand.to_string(),
        project_dir.to_string(),
        "--daemon".to_string(),
        "--log-file".to_string(),
        format!("{}/.regista/daemon.log", project_dir),
    ];

    if let Some(spec) = spec {
        args.push(spec.to_string());
    }
    if replace {
        args.push("--replace".to_string());
    }
    if max_stories > 0 {
        args.push("--max-stories".to_string());
        args.push(max_stories.to_string());
    }
    if let Some(ref story) = pipeline.story {
        args.push("--story".to_string());
        args.push(story.clone());
    }
    // ... mismos para epic, epics, once, resume, config, provider, quiet

    args
}
```

---

## 🔄 Migración desde CLI antigua

### Flags y comandos que desaparecen

| Antes | Después | Nota |
|-------|---------|------|
| `regista [dir]` | `regista run [dir]` | Pipeline requiere subcomando explícito |
| `regista --detach` | *Desaparece* | Detach es el default, no hace falta flag |
| `regista --follow` | `regista logs` | Subcomando separado |
| `regista groom <spec>` | `regista plan <spec>` | Solo genera historias |
| `regista groom <spec> --run` | `regista auto <spec>` | Genera + ejecuta |
| `regista --json` | *Deprecado* | Se ignora con warning |
| `regista help` | `regista --help` / `regista <sub> --help` | Automático de clap |

### Flags que se mantienen (con mismo significado)

| Flag | Subcomandos donde aplica |
|------|--------------------------|
| `--story <ID>` | `auto`, `run` |
| `--epic <ID>` | `auto`, `run` |
| `--epics <RANGO>` | `auto`, `run` |
| `--once` | `auto`, `run` |
| `--dry-run` | `plan`, `auto`, `run` |
| `--quiet` | `plan`, `auto`, `run` |
| `--config <PATH>` | `plan`, `auto`, `run`, `validate` |
| `--provider <NAME>` | `plan`, `auto`, `run`, `validate`, `init` |
| `--resume` | `auto`, `run` |
| `--clean-state` | `auto`, `run` |
| `--max-stories <N>` | `plan`, `auto` |
| `--replace` | `plan`, `auto` |

### Nuevos flags

| Flag | Subcomandos | Significado |
|------|-------------|-------------|
| `--logs` | `plan`, `auto`, `run` | Tail del log tras spawnear daemon |

---

## 🧪 Estrategia de testing

### Tests de CLI (clap)

Se actualizan los tests existentes en `main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_default_dir() {
        let args = Cli::try_parse_from(["regista", "run"]).unwrap();
        match args.command {
            Commands::Run(r) => {
                assert_eq!(r.repo.dir.unwrap_or_else(|| ".".into()), ".");
                assert!(!r.common.dry_run);
                assert!(!r.common.logs);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn plan_with_replace() {
        let args = Cli::try_parse_from([
            "regista", "plan", "spec.md", "--replace", "--max-stories", "10"
        ]).unwrap();
        match args.command {
            Commands::Plan(p) => {
                assert_eq!(p.plan_mode.spec, "spec.md");
                assert!(p.plan_mode.replace);
                assert_eq!(p.plan_mode.max_stories, 10);
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn auto_with_pipeline_flags() {
        let args = Cli::try_parse_from([
            "regista", "auto", "spec.md", "--epic", "EPIC-001", "--once", "--logs"
        ]).unwrap();
        match args.command {
            Commands::Auto(a) => {
                assert_eq!(a.plan_mode.spec, "spec.md");
                assert_eq!(a.pipeline.epic.unwrap(), "EPIC-001");
                assert!(a.pipeline.once);
                assert!(a.common.logs);
            }
            _ => panic!("expected Auto"),
        }
    }

    #[test]
    fn run_with_story_filter() {
        let args = Cli::try_parse_from([
            "regista", "run", "/tmp/proj", "--story", "STORY-001", "--dry-run"
        ]).unwrap();
        match args.command {
            Commands::Run(r) => {
                assert_eq!(r.repo.dir.unwrap(), "/tmp/proj");
                assert_eq!(r.pipeline.story.unwrap(), "STORY-001");
                assert!(r.common.dry_run);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn logs_subcommand() {
        let args = Cli::try_parse_from(["regista", "logs", "/tmp/proj"]).unwrap();
        match args.command {
            Commands::Logs(l) => assert_eq!(l.dir.unwrap(), "/tmp/proj"),
            _ => panic!("expected Logs"),
        }
    }

    #[test]
    fn status_subcommand() {
        let args = Cli::try_parse_from(["regista", "status"]).unwrap();
        assert!(matches!(args.command, Commands::Status(_)));
    }

    #[test]
    fn kill_subcommand() {
        let args = Cli::try_parse_from(["regista", "kill"]).unwrap();
        assert!(matches!(args.command, Commands::Kill(_)));
    }

    #[test]
    fn run_epic_conflicts_with_epics() {
        let err = Cli::try_parse_from([
            "regista", "run", ".", "--epic", "EPIC-001", "--epics", "EPIC-001..EPIC-003"
        ]).unwrap_err();
        assert!(err.to_string().contains("--epic"));
    }
}
```

### Tests de integración

Los tests de `daemon.rs` no requieren cambios (no testean `detach()` directamente, solo helpers).

Los tests de `groom.rs` y `orchestrator.rs` no requieren cambios (su lógica no cambia).

---

## 📁 Archivos modificados

| Archivo | Tipo de cambio | Líneas estimadas |
|---------|----------------|------------------|
| `src/daemon.rs` | Refactor de firma de `detach()` | ~30 líneas modificadas |
| `src/main.rs` | Reescritura completa: `#[derive(Subcommand)]`, handlers por subcomando, helpers | ~400 líneas (nuevo) |
| `src/groom.rs` | Sin cambios | 0 |
| `src/orchestrator.rs` | Sin cambios | 0 |
| `src/config.rs` | Sin cambios | 0 |
| `src/providers.rs` | Sin cambios | 0 |
| `src/agent.rs` | Sin cambios | 0 |
| Resto de `src/` | Sin cambios | 0 |

---

## 🚦 Orden de implementación

1. **`daemon.rs`**: cambiar firma de `detach(child_args)`.
2. **`main.rs`**: reescribir completamente con clap subcommands:
   - Definir `Cli`, `Commands`, y todos los `Args` structs.
   - Implementar `handle_plan`, `handle_auto`, `handle_run`, `handle_logs`, `handle_status`, `handle_kill`, `handle_validate`, `handle_init`.
   - Implementar helpers: `bootstrap()`, `build_run_options()`, `build_daemon_args()`.
3. **Tests**: actualizar tests de CLI.
4. **Compilar y verificar**: `cargo build`, `cargo test`, `cargo clippy`.

---

## ⚠️ Riesgos y notas

- **Rompe retrocompatibilidad**: `regista .` ya no funciona. `regista --detach` ya no existe. `regista groom spec.md --run` cambia a `regista auto spec.md`. Hay que actualizar README, DESIGN.md, AGENTS.md, y HANDOFF.md.
- **El flag `--json` se depreca**: los usuarios de CI/CD que dependían de `regista --json` necesitarán una alternativa (spec futura). Mientras tanto, `--json` se ignora con warning a stderr.
- **Migración de `groom` standalone**: el viejo `regista groom spec.md` (sin `--run`) se convierte en `regista plan spec.md`. El comando `groom` como subcomando **desaparece**.
- **Detección manual de subcomandos**: se elimina por completo. Todo pasa por clap.
