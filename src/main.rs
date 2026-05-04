//! regista — 🎬 AI agent director.
//!
//! Orquestador genérico de agentes para pi, Claude Code, Codex y OpenCode.
//! Pipeline con 3 modos: plan, auto (plan + pipeline), run (pipeline).
//! Toda ejecución es en modo daemon (background). Usa --logs para ver el progreso.

mod agent;
mod board;
mod checkpoint;
mod config;
mod daemon;
mod deadlock;
mod dependency_graph;
mod git;
mod hooks;
mod init;
mod orchestrator;
mod plan;
mod prompts;
mod providers;
mod state;
mod story;
mod update;
mod validator;

use clap::{Args, Parser, Subcommand};
use std::path::Path;

// ═══════════════════════════════════════════════════════════════════════════
// CLI — Estructura de subcomandos
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser, Debug)]
#[command(name = "regista", version, about = "🎬 AI agent director")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
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
struct RepoArgs {
    /// Directorio del proyecto
    #[arg(default_value = ".", num_args = 0..=1)]
    dir: String,
}

/// Args del modo plan.
#[derive(Args, Debug)]
struct PlanModeArgs {
    /// Archivo de especificación de producto
    spec: String,

    /// Reemplazar historias existentes (modo destructivo)
    #[arg(long)]
    replace: bool,

    /// Máximo de historias a generar (0 = sin límite)
    #[arg(long, default_value = "0")]
    max_stories: u32,
}

/// Args comunes a los subcomandos de pipeline.
#[derive(Args, Debug)]
struct CommonArgs {
    /// Ver el log del daemon en vivo tras lanzarlo (Ctrl+C no detiene el daemon)
    #[arg(long)]
    logs: bool,

    /// Simulación síncrona (sin agentes, sin coste)
    #[arg(long)]
    dry_run: bool,

    /// Ruta al archivo .regista/config.toml
    #[arg(long)]
    config: Option<String>,

    /// Provider de agente (pi, claude, codex, opencode)
    #[arg(long)]
    provider: Option<String>,

    /// Suprimir logs de progreso (solo errores)
    #[arg(long)]
    quiet: bool,
}

/// Args de filtrado y control del pipeline.
#[derive(Args, Debug, Default)]
struct PipelineArgs {
    /// Filtrar por historia (STORY-001)
    #[arg(long)]
    story: Option<String>,

    /// Filtrar por épica (EPIC-001)
    #[arg(long, conflicts_with = "epics")]
    epic: Option<String>,

    /// Filtrar por rango de épicas (EPIC-001..EPIC-003)
    #[arg(long, conflicts_with = "epic")]
    epics: Option<String>,

    /// Una sola iteración del pipeline
    #[arg(long)]
    once: bool,

    /// Reanudar desde el último checkpoint
    #[arg(long)]
    resume: bool,

    /// Borrar el checkpoint antes de arrancar
    #[arg(long)]
    clean_state: bool,
}

/// Args internos del daemon (ocultos al usuario).
#[derive(Args, Debug)]
struct DaemonArgs {
    /// [INTERNO] Este proceso es el hijo daemon
    #[arg(long, hide = true)]
    daemon: bool,

    /// [INTERNO] Archivo de log del daemon
    #[arg(long, hide = true)]
    log_file: Option<String>,
}

// ── Subcomandos concretos ─────────────────────────────────────────────────

#[derive(Args, Debug)]
struct PlanArgs {
    #[command(flatten)]
    plan_mode: PlanModeArgs,
    #[command(flatten)]
    repo: RepoArgs,
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(Args, Debug)]
struct AutoArgs {
    #[command(flatten)]
    plan_mode: PlanModeArgs,
    #[command(flatten)]
    repo: RepoArgs,
    #[command(flatten)]
    pipeline: PipelineArgs,
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(Args, Debug)]
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

#[derive(Args, Debug)]
struct ValidateArgs {
    #[command(flatten)]
    repo: RepoArgs,

    /// Salida JSON para CI/CD
    #[arg(long)]
    json: bool,

    /// Ruta al archivo .regista/config.toml
    #[arg(long)]
    config: Option<String>,

    /// Provider de agente
    #[arg(long)]
    provider: Option<String>,
}

#[derive(Args, Debug)]
struct InitArgs {
    #[command(flatten)]
    repo: RepoArgs,

    /// Solo generar .regista/config.toml (sin instrucciones de rol)
    #[arg(long)]
    light: bool,

    /// Incluir historia y épica de ejemplo
    #[arg(long)]
    with_example: bool,

    /// Provider de agente (default: pi)
    #[arg(long, default_value = "pi")]
    provider: String,
}

#[derive(Args, Debug)]
struct UpdateArgs {
    /// Instalar automáticamente sin preguntar
    #[arg(long)]
    yes: bool,
}

#[derive(Args, Debug)]
struct BoardArgs {
    #[command(flatten)]
    repo: RepoArgs,

    /// Salida JSON para CI/CD
    #[arg(long)]
    json: bool,

    /// Ruta al archivo .regista/config.toml
    #[arg(long)]
    config: Option<String>,

    /// Filtrar por épica (ej: EPIC-001)
    #[arg(long)]
    epic: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// main() — dispatch
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Plan(args) => handle_plan(args),
        Commands::Auto(args) => handle_auto(args),
        Commands::Run(args) => handle_run(args),
        Commands::Logs(args) => handle_logs(args),
        Commands::Status(args) => handle_status(args),
        Commands::Kill(args) => handle_kill(args),
        Commands::Validate(args) => handle_validate(args),
        Commands::Init(args) => handle_init(args),
        Commands::Update(args) => handle_update(args),
        Commands::Board(args) => handle_board(args),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

// ── plan ──────────────────────────────────────────────────────────────────

fn handle_plan(args: PlanArgs) {
    let project_root = Path::new(&args.repo.dir);

    // Proceso hijo daemon: ejecutar plan y salir
    if args.daemon.daemon {
        setup_daemon_tracing(args.daemon.log_file.as_deref(), args.common.quiet);
        let _cleanup = daemon::PidCleanup(project_root.to_path_buf());
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );
        match plan::run(
            project_root,
            Path::new(&args.plan_mode.spec),
            &cfg,
            args.plan_mode.max_stories,
            args.plan_mode.replace,
        ) {
            Ok(result) => {
                tracing::info!(
                    "Groom completado: {} historias, {} épicas, {} iteraciones. Dependencias: {}",
                    result.stories_created,
                    result.epics_created,
                    result.iterations,
                    if result.dependencies_clean {
                        "limpias"
                    } else {
                        "con errores"
                    }
                );
                if !result.dependencies_clean {
                    std::process::exit(2);
                }
            }
            Err(e) => {
                tracing::error!("Groom falló: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Modo dry-run: ejecutar plan síncrono
    if args.common.dry_run {
        setup_user_tracing(args.common.quiet, false, None);
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );
        match plan::run(
            project_root,
            Path::new(&args.plan_mode.spec),
            &cfg,
            args.plan_mode.max_stories,
            args.plan_mode.replace,
        ) {
            Ok(result) => {
                println!("✅ Groom completado en {} iteraciones.", result.iterations);
                println!("   Historias generadas: {}", result.stories_created);
                println!("   Épicas generadas:    {}", result.epics_created);
                if result.dependencies_clean {
                    println!("   Grafo de dependencias: limpio ✅");
                } else {
                    println!("   Grafo de dependencias: con errores ⚠️");
                }
            }
            Err(e) => {
                eprintln!("❌ Groom falló: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Modo daemon (default)
    let child_args = build_daemon_args(
        "plan",
        &args.repo.dir,
        Some(&args.plan_mode.spec),
        args.plan_mode.replace,
        args.plan_mode.max_stories,
        &PipelineArgs::default(),
        &args.common,
    );

    spawn_and_optionally_follow(project_root, &child_args, args.common.logs);
}

// ── auto ──────────────────────────────────────────────────────────────────

fn handle_auto(args: AutoArgs) {
    let project_root = Path::new(&args.repo.dir);

    // --clean-state se ejecuta en el padre, antes de spawnear
    if args.pipeline.clean_state && !args.daemon.daemon {
        checkpoint::OrchestratorState::remove(project_root);
        println!("✅ Checkpoint eliminado.");
    }

    // Proceso hijo daemon: plan + pipeline
    if args.daemon.daemon {
        setup_daemon_tracing(args.daemon.log_file.as_deref(), args.common.quiet);
        let _cleanup = daemon::PidCleanup(project_root.to_path_buf());
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );

        // 1. Groom
        match plan::run(
            project_root,
            Path::new(&args.plan_mode.spec),
            &cfg,
            args.plan_mode.max_stories,
            args.plan_mode.replace,
        ) {
            Ok(plan_result) => {
                tracing::info!(
                    "Groom completado: {} historias, {} épicas, deps={}",
                    plan_result.stories_created,
                    plan_result.epics_created,
                    if plan_result.dependencies_clean {
                        "limpias"
                    } else {
                        "con errores"
                    }
                );

                if plan_result.stories_created == 0 {
                    tracing::warn!("No se generaron historias. Omitiendo pipeline.");
                    return;
                }
                if !plan_result.dependencies_clean {
                    tracing::warn!("Grafo de dependencias con errores. Omitiendo pipeline.");
                    std::process::exit(2);
                }
            }
            Err(e) => {
                tracing::error!("Groom falló: {e}");
                std::process::exit(1);
            }
        }

        // 2. Pipeline
        let run_options = build_run_options(&args.pipeline, args.common.quiet);
        let resume_state = if args.pipeline.resume {
            checkpoint::OrchestratorState::load(project_root)
        } else {
            None
        };

        match orchestrator::run(project_root, &cfg, &run_options, resume_state) {
            Ok(report) => {
                tracing::info!(
                    "Pipeline completado: {} total, {} done, {} failed, {} iteraciones, {}s",
                    report.total,
                    report.done,
                    report.failed,
                    report.iterations,
                    report.elapsed.as_secs()
                );
                std::process::exit(exit_code_from_report(&report));
            }
            Err(e) => {
                tracing::error!("Pipeline falló: {e}");
                std::process::exit(1);
            }
        }
    }

    // Modo dry-run: plan síncrono + pipeline dry-run síncrono
    if args.common.dry_run {
        setup_user_tracing(args.common.quiet, false, None);
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );

        // Groom
        match plan::run(
            project_root,
            Path::new(&args.plan_mode.spec),
            &cfg,
            args.plan_mode.max_stories,
            args.plan_mode.replace,
        ) {
            Ok(plan_result) => {
                println!(
                    "✅ Groom: {} historias, {} épicas",
                    plan_result.stories_created, plan_result.epics_created
                );
                if !plan_result.dependencies_clean {
                    println!("⚠️  Dependencias con errores. Omitiendo pipeline.");
                    return;
                }
                if plan_result.stories_created == 0 {
                    println!("⚠️  Sin historias. Omitiendo pipeline.");
                    return;
                }
            }
            Err(e) => {
                eprintln!("❌ Groom falló: {e}");
                std::process::exit(1);
            }
        }

        // Pipeline dry-run
        let run_options = build_run_options(&args.pipeline, args.common.quiet);
        match orchestrator::run(project_root, &cfg, &run_options, None) {
            Ok(report) => print_pipeline_summary(&report),
            Err(e) => {
                eprintln!("❌ Pipeline falló: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Modo daemon (default)
    let child_args = build_daemon_args(
        "auto",
        &args.repo.dir,
        Some(&args.plan_mode.spec),
        args.plan_mode.replace,
        args.plan_mode.max_stories,
        &args.pipeline,
        &args.common,
    );

    spawn_and_optionally_follow(project_root, &child_args, args.common.logs);
}

// ── run ───────────────────────────────────────────────────────────────────

fn handle_run(args: RunArgs) {
    let project_root = Path::new(&args.repo.dir);

    // --clean-state se ejecuta en el padre
    if args.pipeline.clean_state && !args.daemon.daemon {
        checkpoint::OrchestratorState::remove(project_root);
        println!("✅ Checkpoint eliminado.");
    }

    // Proceso hijo daemon: ejecutar pipeline
    if args.daemon.daemon {
        setup_daemon_tracing(args.daemon.log_file.as_deref(), args.common.quiet);
        let _cleanup = daemon::PidCleanup(project_root.to_path_buf());
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );

        let run_options = build_run_options(&args.pipeline, args.common.quiet);
        let resume_state = if args.pipeline.resume {
            checkpoint::OrchestratorState::load(project_root)
        } else {
            None
        };

        match orchestrator::run(project_root, &cfg, &run_options, resume_state) {
            Ok(report) => {
                tracing::info!(
                    "Pipeline completado: {} total, {} done, {} failed, {} iteraciones, {}s",
                    report.total,
                    report.done,
                    report.failed,
                    report.iterations,
                    report.elapsed.as_secs()
                );
                std::process::exit(exit_code_from_report(&report));
            }
            Err(e) => {
                tracing::error!("Pipeline falló: {e}");
                std::process::exit(1);
            }
        }
    }

    // Modo dry-run
    if args.common.dry_run {
        setup_user_tracing(args.common.quiet, false, None);
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );
        let run_options = build_run_options(&args.pipeline, args.common.quiet);
        match orchestrator::run(project_root, &cfg, &run_options, None) {
            Ok(report) => print_pipeline_summary(&report),
            Err(e) => {
                eprintln!("❌ Pipeline falló: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Modo daemon (default)
    let child_args = build_daemon_args(
        "run",
        &args.repo.dir,
        None,
        false,
        0,
        &args.pipeline,
        &args.common,
    );

    spawn_and_optionally_follow(project_root, &child_args, args.common.logs);
}

// ── logs / status / kill ──────────────────────────────────────────────────

fn handle_logs(args: RepoArgs) {
    let project_root = Path::new(&args.dir);
    if let Err(e) = daemon::follow(project_root) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn handle_status(args: RepoArgs) {
    let project_root = Path::new(&args.dir);
    match daemon::status(project_root) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn handle_kill(args: RepoArgs) {
    let project_root = Path::new(&args.dir);
    match daemon::kill(project_root) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

// ── validate ──────────────────────────────────────────────────────────────

fn handle_validate(args: ValidateArgs) {
    let project_root = Path::new(&args.repo.dir);
    let config_path = args.config.as_deref().map(Path::new);

    let result = validator::validate(project_root, config_path);

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into())
        );
    } else {
        if result.findings.is_empty() {
            println!("✅ Todo OK — el proyecto está listo para ejecutar el pipeline.");
        } else {
            for finding in &result.findings {
                let icon = match finding.severity {
                    validator::Severity::Error => "❌",
                    validator::Severity::Warning => "⚠️",
                };
                let story = finding
                    .story_id
                    .as_deref()
                    .map(|id| format!(" [{id}]"))
                    .unwrap_or_default();
                println!("{icon} [{}]{} {}", finding.category, story, finding.message);
            }
            println!(
                "\nResultado: {} errores, {} warnings",
                result.errors, result.warnings
            );
        }
    }

    if result.errors > 0 {
        std::process::exit(1);
    } else if result.warnings > 0 {
        std::process::exit(2);
    }
}

// ── init ──────────────────────────────────────────────────────────────────

fn handle_init(args: InitArgs) {
    let project_root = Path::new(&args.repo.dir);

    match init::init(project_root, args.light, args.with_example, &args.provider) {
        Ok(result) => {
            if !result.created.is_empty() {
                println!("Creados:");
                for p in &result.created {
                    println!("  ✅ {p}");
                }
            }
            if !result.skipped.is_empty() {
                println!("Saltados (ya existen):");
                for p in &result.skipped {
                    println!("  ⏭️  {p}");
                }
            }
            if !result.errors.is_empty() {
                println!("Errores:");
                for e in &result.errors {
                    eprintln!("  ❌ {e}");
                }
                std::process::exit(1);
            }
            if result.created.is_empty() && result.skipped.is_empty() {
                println!("Nada que hacer.");
            } else {
                println!("\n✅ Proyecto inicializado en {}", project_root.display());
            }
        }
        Err(e) => {
            eprintln!("Error inicializando proyecto: {e}");
            std::process::exit(1);
        }
    }
}

// ── update ────────────────────────────────────────────────────────────────

fn handle_update(args: UpdateArgs) {
    match update::run(args.yes) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

// ── board ────────────────────────────────────────────────────────────────

fn handle_board(args: BoardArgs) {
    let project_root = Path::new(&args.repo.dir);
    let config_path = args.config.as_deref().map(Path::new);

    if let Err(e) = board::run(project_root, args.json, args.epic.as_deref(), config_path) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Configura tracing para el proceso daemon hijo.
/// Escribe al archivo de log (--log-file). Respeta --quiet.
fn setup_daemon_tracing(log_file: Option<&str>, quiet: bool) {
    let env_filter = if quiet {
        tracing_subscriber::EnvFilter::new("error")
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };

    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

    if let Some(path) = log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("No se pudo crear/abrir el archivo de log del daemon");
        subscriber.with_writer(std::sync::Mutex::new(file)).init();
    } else {
        subscriber.with_writer(std::io::stderr).init();
    }
}

/// Configura tracing para el usuario (proceso padre).
/// Escribe a stderr. Respeta --quiet y --json.
fn setup_user_tracing(quiet: bool, _json: bool, _log_file: Option<&str>) {
    let env_filter = if quiet {
        tracing_subscriber::EnvFilter::new("error")
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();
}

/// Carga la configuración y aplica el override de provider si se especifica.
fn load_config(
    project_root: &Path,
    config_path: Option<&str>,
    provider_override: Option<&str>,
) -> config::Config {
    let config_path_opt = config_path.map(Path::new);

    let mut cfg = match config::Config::load(project_root, config_path_opt) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Error al cargar configuración: {e}");
            std::process::exit(1);
        }
    };

    if let Some(provider) = provider_override {
        cfg.agents.provider = provider.to_string();
        tracing::info!("Provider override: {provider}");
    }

    cfg
}

/// Construye `RunOptions` desde los flags de pipeline.
fn build_run_options(pipeline: &PipelineArgs, quiet: bool) -> orchestrator::RunOptions {
    let epics_range = pipeline.epics.as_ref().and_then(|range| {
        let parts: Vec<&str> = range.split("..").collect();
        if parts.len() == 2 {
            Some((
                parts[0].trim().to_uppercase(),
                parts[1].trim().to_uppercase(),
            ))
        } else {
            tracing::warn!("Formato de rango de épicas inválido: '{range}'. Ignorando.");
            None
        }
    });

    orchestrator::RunOptions {
        once: pipeline.once,
        story_filter: pipeline.story.clone(),
        epic_filter: pipeline.epic.clone(),
        epics_range,
        dry_run: false, // dry_run se maneja en el handler, no se pasa al daemon
        quiet,
    }
}

/// Construye los argumentos para el proceso hijo daemon.
fn build_daemon_args(
    subcommand: &str,
    dir: &str,
    spec: Option<&str>,
    replace: bool,
    max_stories: u32,
    pipeline: &PipelineArgs,
    common: &CommonArgs,
) -> Vec<String> {
    let log_path = Path::new(dir).join(".regista/daemon.log");
    let mut args = vec![subcommand.to_string()];

    // Spec posicional (solo plan / auto) — va antes de dir porque es required
    if let Some(s) = spec {
        args.push(s.to_string());
    }

    // Dir posicional
    args.push(dir.to_string());

    // Flags internos del daemon
    args.push("--daemon".to_string());
    args.push("--log-file".to_string());
    args.push(log_path.to_string_lossy().to_string());

    // Plan mode flags
    if replace {
        args.push("--replace".to_string());
    }
    if max_stories > 0 {
        args.push("--max-stories".to_string());
        args.push(max_stories.to_string());
    }

    // Pipeline flags (no pasamos --clean-state al hijo: ya se ejecutó)
    if let Some(ref s) = pipeline.story {
        args.push("--story".to_string());
        args.push(s.clone());
    }
    if let Some(ref e) = pipeline.epic {
        args.push("--epic".to_string());
        args.push(e.clone());
    }
    if let Some(ref e) = pipeline.epics {
        args.push("--epics".to_string());
        args.push(e.clone());
    }
    if pipeline.once {
        args.push("--once".to_string());
    }
    if pipeline.resume {
        args.push("--resume".to_string());
    }

    // Common flags
    if let Some(ref c) = common.config {
        args.push("--config".to_string());
        args.push(c.clone());
    }
    if let Some(ref p) = common.provider {
        args.push("--provider".to_string());
        args.push(p.clone());
    }
    if common.quiet {
        args.push("--quiet".to_string());
    }

    args
}

/// Spawnea el daemon y opcionalmente sigue el log.
fn spawn_and_optionally_follow(project_root: &Path, child_args: &[String], follow_log: bool) {
    match daemon::detach(project_root, child_args, None) {
        Ok(pid) => {
            let log_display = project_root.join(".regista/daemon.log");
            println!("🚀 Daemon lanzado (PID: {pid})");
            println!("   Log: {}", log_display.display());
            println!("   Usa: regista logs, regista status, regista kill");

            if follow_log {
                if let Err(e) = daemon::follow(project_root) {
                    eprintln!("Error siguiendo el log: {e}");
                }
            }
        }
        Err(e) => {
            eprintln!("Error al lanzar el daemon: {e}");
            std::process::exit(1);
        }
    }
}

/// Imprime el resumen del pipeline (modo dry-run).
fn print_pipeline_summary(report: &orchestrator::RunReport) {
    if let Some(ref reason) = report.stop_reason {
        println!("\n⚠️  Pipeline detenido: {reason}");
    } else {
        println!("\n🏁 Pipeline completado");
    }
    println!("   Total:     {:>4}", report.total);
    println!("   Done:      {:>4}", report.done);
    println!("   Failed:    {:>4}", report.failed);
    println!("   Blocked:   {:>4}", report.blocked);
    println!("   Draft:     {:>4}", report.draft);
    println!("   Iteraciones: {:>2}", report.iterations);
    println!("   Tiempo:     {:>3}s", report.elapsed.as_secs());
}

/// Calcula el exit code según el resultado del pipeline.
fn exit_code_from_report(report: &orchestrator::RunReport) -> i32 {
    if report.stop_reason.is_some() {
        3
    } else if report.failed > 0 {
        2
    } else {
        0
    }
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

    // ── Helpers ──────────────────────────────────────────────────────

    #[test]
    fn build_daemon_args_for_run() {
        let pipeline = PipelineArgs {
            story: Some("STORY-005".into()),
            epic: None,
            epics: None,
            once: true,
            resume: false,
            clean_state: false,
        };
        let common = CommonArgs {
            logs: false,
            dry_run: false,
            config: None,
            provider: Some("claude".into()),
            quiet: false,
        };

        let args = build_daemon_args("run", ".", None, false, 0, &pipeline, &common);

        // Verificar estructura: ["run", ".", "--daemon", "--log-file", "./.regista/daemon.log", ...]
        assert_eq!(args[0], "run");
        assert_eq!(args[1], ".");
        assert!(args.contains(&"--daemon".to_string()));
        assert!(args.contains(&"--story".to_string()));
        assert!(args.contains(&"STORY-005".to_string()));
        assert!(args.contains(&"--once".to_string()));
        assert!(args.contains(&"--provider".to_string()));
        assert!(args.contains(&"claude".to_string()));
        // --clean-state NO debe pasarse al hijo
        assert!(!args.contains(&"--clean-state".to_string()));
    }

    #[test]
    fn build_daemon_args_for_plan() {
        let pipeline = PipelineArgs::default();
        let common = CommonArgs {
            logs: true, // --logs NO se pasa al hijo
            dry_run: false,
            config: Some("custom.toml".into()),
            provider: None,
            quiet: true,
        };

        let args = build_daemon_args(
            "plan",
            "myproj",
            Some("spec.md"),
            true,
            10,
            &pipeline,
            &common,
        );

        assert_eq!(args[0], "plan");
        assert_eq!(args[1], "spec.md");
        assert_eq!(args[2], "myproj");
        assert!(args.contains(&"--replace".to_string()));
        assert!(args.contains(&"10".to_string())); // max-stories value
        assert!(args.contains(&"--config".to_string()));
        assert!(args.contains(&"custom.toml".to_string()));
        assert!(args.contains(&"--quiet".to_string()));
        // --logs NO debe pasarse al hijo
        assert!(!args.contains(&"--logs".to_string()));
    }

    #[test]
    fn exit_code_all_done_is_zero() {
        let report = orchestrator::RunReport {
            total: 5,
            done: 5,
            failed: 0,
            blocked: 0,
            draft: 0,
            iterations: 3,
            elapsed: std::time::Duration::from_secs(30),
            elapsed_seconds: 30,
            stop_reason: None,
            stories: vec![],
        };
        assert_eq!(exit_code_from_report(&report), 0);
    }

    #[test]
    fn exit_code_with_failures_is_2() {
        let report = orchestrator::RunReport {
            total: 5,
            done: 3,
            failed: 2,
            blocked: 0,
            draft: 0,
            iterations: 5,
            elapsed: std::time::Duration::from_secs(60),
            elapsed_seconds: 60,
            stop_reason: None,
            stories: vec![],
        };
        assert_eq!(exit_code_from_report(&report), 2);
    }

    #[test]
    fn exit_code_stopped_early_is_3() {
        let report = orchestrator::RunReport {
            total: 10,
            done: 2,
            failed: 0,
            blocked: 5,
            draft: 3,
            iterations: 50,
            elapsed: std::time::Duration::from_secs(600),
            elapsed_seconds: 600,
            stop_reason: Some("max_iterations".into()),
            stories: vec![],
        };
        assert_eq!(exit_code_from_report(&report), 3);
    }
}
