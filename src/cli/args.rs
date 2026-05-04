use clap::{Args, Parser, Subcommand};

// ═══════════════════════════════════════════════════════════════════════════
// CLI — Estructura de subcomandos
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser, Debug)]
#[command(name = "regista", version, about = "🎬 AI agent director")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generar historias de usuario desde una especificación (daemon)
    Plan(PlanArgs),
    /// Generar historias y ejecutar el pipeline completo (daemon)
    Auto(AutoArgs),
    /// Ejecutar el pipeline sobre historias existentes (daemon)
    Run(RunArgs),
    /// Ver el log del daemon en vivo
    Logs(RepoArgs),
    /// Consultar si el daemon está corriendo
    Status(RepoArgs),
    /// Detener el daemon
    Kill(RepoArgs),
    /// Validar la configuración y las historias del proyecto
    Validate(ValidateArgs),
    /// Inicializar un nuevo proyecto
    Init(InitArgs),
    /// Comprobar e instalar una nueva versión de regista desde crates.io
    Update(UpdateArgs),
    /// Mostrar el tablero Kanban con el estado de todas las historias
    Board(BoardArgs),
}

// ── Args compartidos ──────────────────────────────────────────────────────

/// Args para subcomandos que solo necesitan directorio de proyecto.
#[derive(Args, Debug)]
pub struct RepoArgs {
    /// Directorio del proyecto
    #[arg(default_value = ".", num_args = 0..=1)]
    pub dir: String,
}

/// Args del modo plan.
#[derive(Args, Debug)]
pub struct PlanModeArgs {
    /// Archivo de especificación de producto
    pub spec: String,

    /// Reemplazar historias existentes (modo destructivo)
    #[arg(long)]
    pub replace: bool,

    /// Máximo de historias a generar (0 = sin límite)
    #[arg(long, default_value = "0")]
    pub max_stories: u32,
}

/// Args comunes a los subcomandos de pipeline.
#[derive(Args, Debug)]
pub struct CommonArgs {
    /// Ver el log del daemon en vivo tras lanzarlo (Ctrl+C no detiene el daemon)
    #[arg(long)]
    pub logs: bool,

    /// Simulación síncrona (sin agentes, sin coste)
    #[arg(long)]
    pub dry_run: bool,

    /// Ruta al archivo .regista/config.toml
    #[arg(long)]
    pub config: Option<String>,

    /// Provider de agente (pi, claude, codex, opencode)
    #[arg(long)]
    pub provider: Option<String>,

    /// Suprimir logs de progreso (solo errores)
    #[arg(long)]
    pub quiet: bool,
}

/// Args de filtrado y control del pipeline.
#[derive(Args, Debug, Default)]
pub struct PipelineArgs {
    /// Filtrar por historia (STORY-001)
    #[arg(long)]
    pub story: Option<String>,

    /// Filtrar por épica (EPIC-001)
    #[arg(long, conflicts_with = "epics")]
    pub epic: Option<String>,

    /// Filtrar por rango de épicas (EPIC-001..EPIC-003)
    #[arg(long, conflicts_with = "epic")]
    pub epics: Option<String>,

    /// Una sola iteración del pipeline
    #[arg(long)]
    pub once: bool,

    /// Reanudar desde el último checkpoint
    #[arg(long)]
    pub resume: bool,

    /// Borrar el checkpoint antes de arrancar
    #[arg(long)]
    pub clean_state: bool,
}

/// Args internos del daemon (ocultos al usuario).
#[derive(Args, Debug)]
pub struct DaemonArgs {
    /// [INTERNO] Este proceso es el hijo daemon
    #[arg(long, hide = true)]
    pub daemon: bool,

    /// [INTERNO] Archivo de log del daemon
    #[arg(long, hide = true)]
    pub log_file: Option<String>,
}

// ── Subcomandos concretos ─────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct PlanArgs {
    #[command(flatten)]
    pub plan_mode: PlanModeArgs,
    #[command(flatten)]
    pub repo: RepoArgs,
    #[command(flatten)]
    pub common: CommonArgs,
    #[command(flatten)]
    pub daemon: DaemonArgs,
}

#[derive(Args, Debug)]
pub struct AutoArgs {
    #[command(flatten)]
    pub plan_mode: PlanModeArgs,
    #[command(flatten)]
    pub repo: RepoArgs,
    #[command(flatten)]
    pub pipeline: PipelineArgs,
    #[command(flatten)]
    pub common: CommonArgs,
    #[command(flatten)]
    pub daemon: DaemonArgs,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    #[command(flatten)]
    pub repo: RepoArgs,
    #[command(flatten)]
    pub pipeline: PipelineArgs,
    #[command(flatten)]
    pub common: CommonArgs,
    #[command(flatten)]
    pub daemon: DaemonArgs,
}

#[derive(Args, Debug)]
pub struct ValidateArgs {
    #[command(flatten)]
    pub repo: RepoArgs,

    /// Salida JSON para CI/CD
    #[arg(long)]
    pub json: bool,

    /// Ruta al archivo .regista/config.toml
    #[arg(long)]
    pub config: Option<String>,

    /// Provider de agente
    #[arg(long)]
    pub provider: Option<String>,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    #[command(flatten)]
    pub repo: RepoArgs,

    /// Solo generar .regista/config.toml (sin instrucciones de rol)
    #[arg(long)]
    pub light: bool,

    /// Incluir historia y épica de ejemplo
    #[arg(long)]
    pub with_example: bool,

    /// Provider de agente (default: pi)
    #[arg(long, default_value = "pi")]
    pub provider: String,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Instalar automáticamente sin preguntar
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct BoardArgs {
    #[command(flatten)]
    pub repo: RepoArgs,

    /// Salida JSON para CI/CD
    #[arg(long)]
    pub json: bool,

    /// Ruta al archivo .regista/config.toml
    #[arg(long)]
    pub config: Option<String>,

    /// Filtrar por épica (ej: EPIC-001)
    #[arg(long)]
    pub epic: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── CLI parsing ──────────────────────────────────────────────────

    #[test]
    fn run_defaults() {
        let args = Cli::try_parse_from(["regista", "run"]).unwrap();
        match args.command {
            Commands::Run(r) => {
                assert_eq!(r.repo.dir, ".");
                assert!(!r.common.dry_run);
                assert!(!r.common.logs);
                assert!(r.pipeline.story.is_none());
                assert!(!r.pipeline.once);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn run_with_filters() {
        let args = Cli::try_parse_from([
            "regista",
            "run",
            "myproject",
            "--story",
            "STORY-001",
            "--once",
            "--dry-run",
        ])
        .unwrap();
        match args.command {
            Commands::Run(r) => {
                assert_eq!(r.repo.dir, "myproject");
                assert_eq!(r.pipeline.story.unwrap(), "STORY-001");
                assert!(r.pipeline.once);
                assert!(r.common.dry_run);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn run_epic_conflicts_with_epics() {
        let err = Cli::try_parse_from([
            "regista",
            "run",
            ".",
            "--epic",
            "EPIC-001",
            "--epics",
            "EPIC-001..EPIC-003",
        ])
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("--epic") || msg.contains("--epics"),
            "expected conflict, got: {msg}"
        );
    }

    #[test]
    fn plan_basic() {
        let args = Cli::try_parse_from(["regista", "plan", "spec.md"]).unwrap();
        match args.command {
            Commands::Plan(p) => {
                assert_eq!(p.plan_mode.spec, "spec.md");
                assert_eq!(p.repo.dir, ".");
                assert!(!p.plan_mode.replace);
                assert_eq!(p.plan_mode.max_stories, 0);
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn plan_with_replace_and_limit() {
        let args = Cli::try_parse_from([
            "regista",
            "plan",
            "docs/spec.md",
            "--replace",
            "--max-stories",
            "15",
        ])
        .unwrap();
        match args.command {
            Commands::Plan(p) => {
                assert_eq!(p.plan_mode.spec, "docs/spec.md");
                assert!(p.plan_mode.replace);
                assert_eq!(p.plan_mode.max_stories, 15);
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn plan_with_logs() {
        let args = Cli::try_parse_from(["regista", "plan", "spec.md", "--logs"]).unwrap();
        match args.command {
            Commands::Plan(p) => {
                assert!(p.common.logs);
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn auto_full() {
        let args = Cli::try_parse_from([
            "regista",
            "auto",
            "spec.md",
            "--replace",
            "--max-stories",
            "20",
            "--epic",
            "EPIC-001",
            "--once",
            "--logs",
        ])
        .unwrap();
        match args.command {
            Commands::Auto(a) => {
                assert_eq!(a.plan_mode.spec, "spec.md");
                assert!(a.plan_mode.replace);
                assert_eq!(a.plan_mode.max_stories, 20);
                assert_eq!(a.pipeline.epic.unwrap(), "EPIC-001");
                assert!(a.pipeline.once);
                assert!(a.common.logs);
            }
            _ => panic!("expected Auto"),
        }
    }

    #[test]
    fn logs_subcommand() {
        let args = Cli::try_parse_from(["regista", "logs", "myproject"]).unwrap();
        match args.command {
            Commands::Logs(l) => assert_eq!(l.dir, "myproject"),
            _ => panic!("expected Logs"),
        }
    }

    #[test]
    fn logs_default_dir() {
        let args = Cli::try_parse_from(["regista", "logs"]).unwrap();
        match args.command {
            Commands::Logs(l) => assert_eq!(l.dir, "."),
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
        let args = Cli::try_parse_from(["regista", "kill", "."]).unwrap();
        match args.command {
            Commands::Kill(k) => assert_eq!(k.dir, "."),
            _ => panic!("expected Kill"),
        }
    }

    #[test]
    fn validate_subcommand() {
        let args = Cli::try_parse_from(["regista", "validate", ".", "--json"]).unwrap();
        match args.command {
            Commands::Validate(v) => {
                assert_eq!(v.repo.dir, ".");
                assert!(v.json);
            }
            _ => panic!("expected Validate"),
        }
    }

    #[test]
    fn init_subcommand() {
        let args = Cli::try_parse_from(["regista", "init", ".", "--light", "--provider", "claude"])
            .unwrap();
        match args.command {
            Commands::Init(i) => {
                assert_eq!(i.repo.dir, ".");
                assert!(i.light);
                assert!(!i.with_example);
                assert_eq!(i.provider, "claude");
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn board_defaults() {
        let args = Cli::try_parse_from(["regista", "board"]).unwrap();
        match args.command {
            Commands::Board(b) => {
                assert_eq!(b.repo.dir, ".");
                assert!(!b.json);
                assert!(b.epic.is_none());
                assert!(b.config.is_none());
            }
            _ => panic!("expected Board"),
        }
    }

    #[test]
    fn board_with_epic_and_json() {
        let args = Cli::try_parse_from([
            "regista",
            "board",
            "myproject",
            "--epic",
            "EPIC-002",
            "--json",
        ])
        .unwrap();
        match args.command {
            Commands::Board(b) => {
                assert_eq!(b.repo.dir, "myproject");
                assert!(b.json);
                assert_eq!(b.epic.unwrap(), "EPIC-002");
            }
            _ => panic!("expected Board"),
        }
    }

    #[test]
    fn board_with_config() {
        let args = Cli::try_parse_from(["regista", "board", "--config", "custom.toml"]).unwrap();
        match args.command {
            Commands::Board(b) => {
                assert_eq!(b.config.unwrap(), "custom.toml");
            }
            _ => panic!("expected Board"),
        }
    }

    #[test]
    fn init_with_example() {
        let args = Cli::try_parse_from([
            "regista",
            "init",
            "newproject",
            "--with-example",
            "--provider",
            "codex",
        ])
        .unwrap();
        match args.command {
            Commands::Init(i) => {
                assert_eq!(i.repo.dir, "newproject");
                assert!(i.with_example);
                assert!(!i.light);
                assert_eq!(i.provider, "codex");
            }
            _ => panic!("expected Init"),
        }
    }
}
