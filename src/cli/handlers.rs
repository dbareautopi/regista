use std::path::{Path, PathBuf};

use chrono;
use glob;

use crate::app;
use crate::config;
use crate::infra;

use super::args::{
    AutoArgs, BoardArgs, Cli, Commands, CommonArgs, InitArgs, PipelineArgs, PlanArgs, RepoArgs,
    RunArgs, UpdateArgs, ValidateArgs,
};

// ═══════════════════════════════════════════════════════════════════════════
// dispatch
// ═══════════════════════════════════════════════════════════════════════════

pub fn dispatch(cli: Cli) {
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
        let _cleanup = infra::daemon::PidCleanup(project_root.to_path_buf());
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );

        // Header de sesión (antes de groom, story_count=0)
        emit_session_header(&cfg, project_root, 0, false);

        match app::plan::run(
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
        match app::plan::run(
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
    let cfg = load_config(
        project_root,
        args.common.config.as_deref(),
        args.common.provider.as_deref(),
    );
    let log_file = create_log_file(project_root, &cfg.project.log_dir);
    let child_args = build_daemon_args(
        "plan",
        &args.repo.dir,
        Some(&args.plan_mode.spec),
        args.plan_mode.replace,
        args.plan_mode.max_stories,
        &PipelineArgs::default(),
        &args.common,
        &log_file,
    );

    spawn_and_optionally_follow(project_root, &child_args, &log_file, args.common.logs);
}

// ── auto ──────────────────────────────────────────────────────────────────

fn handle_auto(args: AutoArgs) {
    let project_root = Path::new(&args.repo.dir);

    // --clean-state se ejecuta en el padre, antes de spawnear
    if args.pipeline.clean_state && !args.daemon.daemon {
        infra::checkpoint::OrchestratorState::remove(project_root);
        println!("✅ Checkpoint eliminado.");
    }

    // Proceso hijo daemon: plan + pipeline
    if args.daemon.daemon {
        setup_daemon_tracing(args.daemon.log_file.as_deref(), args.common.quiet);
        let _cleanup = infra::daemon::PidCleanup(project_root.to_path_buf());
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );

        // Header de sesión (antes de groom, story_count=0 porque aún no hay historias)
        emit_session_header(&cfg, project_root, 0, false);

        // 1. Groom
        match app::plan::run(
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
            infra::checkpoint::OrchestratorState::load(project_root)
        } else {
            None
        };

        match app::pipeline::run(project_root, &cfg, &run_options, resume_state) {
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
        match app::plan::run(
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
        let mut run_options = build_run_options(&args.pipeline, args.common.quiet);
        run_options.dry_run = true;
        match app::pipeline::run(project_root, &cfg, &run_options, None) {
            Ok(report) => print_pipeline_summary(&report),
            Err(e) => {
                eprintln!("❌ Pipeline falló: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Modo daemon (default)
    let cfg = load_config(
        project_root,
        args.common.config.as_deref(),
        args.common.provider.as_deref(),
    );
    let log_file = create_log_file(project_root, &cfg.project.log_dir);
    let child_args = build_daemon_args(
        "auto",
        &args.repo.dir,
        Some(&args.plan_mode.spec),
        args.plan_mode.replace,
        args.plan_mode.max_stories,
        &args.pipeline,
        &args.common,
        &log_file,
    );

    spawn_and_optionally_follow(project_root, &child_args, &log_file, args.common.logs);
}

// ── run ───────────────────────────────────────────────────────────────────

fn handle_run(args: RunArgs) {
    let project_root = Path::new(&args.repo.dir);

    // --clean-state se ejecuta en el padre
    if args.pipeline.clean_state && !args.daemon.daemon {
        infra::checkpoint::OrchestratorState::remove(project_root);
        println!("✅ Checkpoint eliminado.");
    }

    // Proceso hijo daemon: ejecutar pipeline
    if args.daemon.daemon {
        setup_daemon_tracing(args.daemon.log_file.as_deref(), args.common.quiet);
        let _cleanup = infra::daemon::PidCleanup(project_root.to_path_buf());
        let cfg = load_config(
            project_root,
            args.common.config.as_deref(),
            args.common.provider.as_deref(),
        );

        // Contar historias para el header de sesión
        let story_count = count_stories(project_root, &cfg);
        emit_session_header(&cfg, project_root, story_count, false);

        let run_options = build_run_options(&args.pipeline, args.common.quiet);
        let resume_state = if args.pipeline.resume {
            infra::checkpoint::OrchestratorState::load(project_root)
        } else {
            None
        };

        match app::pipeline::run(project_root, &cfg, &run_options, resume_state) {
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
        let mut run_options = build_run_options(&args.pipeline, args.common.quiet);
        run_options.dry_run = true;
        match app::pipeline::run(project_root, &cfg, &run_options, None) {
            Ok(report) => print_pipeline_summary(&report),
            Err(e) => {
                eprintln!("❌ Pipeline falló: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Modo daemon (default)
    let cfg = load_config(
        project_root,
        args.common.config.as_deref(),
        args.common.provider.as_deref(),
    );
    let log_file = create_log_file(project_root, &cfg.project.log_dir);
    let child_args = build_daemon_args(
        "run",
        &args.repo.dir,
        None,
        false,
        0,
        &args.pipeline,
        &args.common,
        &log_file,
    );

    spawn_and_optionally_follow(project_root, &child_args, &log_file, args.common.logs);
}

// ── logs / status / kill ──────────────────────────────────────────────────

fn handle_logs(args: RepoArgs) {
    let project_root = Path::new(&args.dir);
    if let Err(e) = infra::daemon::follow(project_root) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn handle_status(args: RepoArgs) {
    let project_root = Path::new(&args.dir);
    match infra::daemon::status(project_root) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn handle_kill(args: RepoArgs) {
    let project_root = Path::new(&args.dir);
    match infra::daemon::kill(project_root) {
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

    let result = app::validate::validate(project_root, config_path);

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
                    app::validate::Severity::Error => "❌",
                    app::validate::Severity::Warning => "⚠️",
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

    match app::init::init(project_root, args.light, args.with_example, &args.provider) {
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
    match app::update::run(args.yes) {
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

    if let Err(e) = app::board::run(project_root, args.json, args.epic.as_deref(), config_path) {
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

/// Emite el header de sesión vía `tracing::info!` usando `format_session_header`.
fn emit_session_header(
    cfg: &config::Config,
    project_root: &Path,
    story_count: usize,
    compact: bool,
) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let header = format_session_header(
        cfg,
        env!("CARGO_PKG_VERSION"),
        project_root,
        story_count,
        compact,
        &now,
    );
    tracing::info!("{}", header);
}

/// Cuenta las historias en `stories_dir` usando el patrón configurado.
fn count_stories(project_root: &Path, cfg: &config::Config) -> usize {
    let stories_dir = project_root.join(&cfg.project.stories_dir);
    let pattern = stories_dir.join(&cfg.project.story_pattern);
    let pattern_str = pattern.to_string_lossy();
    match glob::glob(&pattern_str) {
        Ok(paths) => paths.filter_map(|r| r.ok()).count(),
        Err(_) => 0,
    }
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
fn build_run_options(pipeline: &PipelineArgs, quiet: bool) -> app::pipeline::RunOptions {
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

    app::pipeline::RunOptions {
        once: pipeline.once,
        story_filter: pipeline.story.clone(),
        epic_filter: pipeline.epic.clone(),
        epics_range,
        dry_run: false, // se sobreescribe en los handlers que usan --dry-run
        quiet,
        compact: false,
    }
}

/// Crea un archivo de log con timestamp dentro de `log_dir`.
/// Limpia logs antiguos (conserva solo los últimos 10).
/// Retorna la ruta absoluta al nuevo archivo.
fn create_log_file(project_root: &Path, log_dir: &str) -> PathBuf {
    let logs_dir = project_root.join(log_dir);
    std::fs::create_dir_all(&logs_dir).ok();

    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let log_path = logs_dir.join(format!("regista-log-{timestamp}.log"));

    cleanup_old_logs(&logs_dir, 10);

    log_path
}

/// Elimina los logs más antiguos, conservando solo los `keep` más recientes.
fn cleanup_old_logs(logs_dir: &Path, keep: usize) {
    let mut entries: Vec<_> = std::fs::read_dir(logs_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("regista-log-") && name.ends_with(".log")
        })
        .collect();

    // Ordenar por nombre descendente (los timestamps son ordenables lexicográficamente)
    entries.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

    for entry in entries.iter().skip(keep) {
        let _ = std::fs::remove_file(entry.path());
    }
}

#[allow(clippy::too_many_arguments)]
/// Construye los argumentos para el proceso hijo daemon.
fn build_daemon_args(
    subcommand: &str,
    dir: &str,
    spec: Option<&str>,
    replace: bool,
    max_stories: u32,
    pipeline: &PipelineArgs,
    common: &CommonArgs,
    log_file: &Path,
) -> Vec<String> {
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
    args.push(log_file.to_string_lossy().to_string());

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
fn spawn_and_optionally_follow(
    project_root: &Path,
    child_args: &[String],
    log_file: &Path,
    follow_log: bool,
) {
    match infra::daemon::detach(project_root, child_args, None) {
        Ok(pid) => {
            println!("🚀 Daemon lanzado (PID: {pid})");
            println!("   Log: {}", log_file.display());
            println!("   Usa: regista logs, regista status, regista kill");

            if follow_log {
                if let Err(e) = infra::daemon::follow(project_root) {
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
fn print_pipeline_summary(report: &app::pipeline::RunReport) {
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
fn exit_code_from_report(report: &app::pipeline::RunReport) -> i32 {
    if report.stop_reason.is_some() {
        3
    } else if report.failed > 0 {
        2
    } else {
        0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STORY-026: Header de sesión
// ═══════════════════════════════════════════════════════════════════════════

/// Formatea el header de sesión que se emite al iniciar el daemon.
///
/// En modo detallado (default), produce un bloque multilínea con:
/// - Versión de regista
/// - Timestamp UTC
/// - Directorio del proyecto
/// - Provider global
/// - Modelos por rol (resueltos con `AgentsConfig::model_for_role()`)
/// - Límites (max_iter efectivo, max_reject_cycles, timeout)
/// - Estado de git (habilitado / deshabilitado)
/// - Hooks configurados (o "ninguno")
///
/// En modo compacto, produce una línea.
///
/// `story_count` se usa para calcular `max_iter` efectivo cuando
/// `limits.max_iterations = 0`.
///
/// La resolución de modelos usa `AgentsConfig::model_for_role()` con
/// el path de instrucciones de cada rol (`skill_for_role`).
/// Formatea el header de sesión con metadatos del pipeline.
///
/// En modo detallado (default), emite un bloque multilínea con:
/// versión, timestamp UTC, proyecto, provider, modelos por rol,
/// límites (max_iter efectivo, max_reject, timeout), estado de git,
/// y hooks configurados.
///
/// En modo compacto (`--compact`), reduce el header a una línea.
///
/// La resolución de modelos usa `AgentsConfig::model_for_role()`
/// con las rutas de skill resueltas contra `project_root`.
pub fn format_session_header(
    cfg: &config::Config,
    version: &str,
    project_root: &Path,
    story_count: usize,
    compact: bool,
    now_utc: &str,
) -> String {
    if compact {
        let effective_max = effective_max_iter(story_count, cfg.limits.max_iterations);
        return format!(
            "🛰️  regista v{} | {} | {} UTC | max_iter={}",
            version, cfg.agents.provider, now_utc, effective_max
        );
    }

    // ── Modo detallado ───────────────────────────────────────────
    let sep = "═══════════════════════════════════════════════════════════";
    let mut lines: Vec<String> = Vec::new();

    // Título
    lines.push(format!(
        "🛰️  regista v{} — sesión iniciada {} UTC",
        version, now_utc
    ));

    // Proyecto
    lines.push(format!("   Proyecto   : {}", project_root.display()));

    // Provider
    lines.push(format!("   Provider    : {}", cfg.agents.provider));

    // Modelos (resueltos con model_for_role)
    let models = config::AgentsConfig::all_roles()
        .iter()
        .map(|role| {
            let skill_rel = cfg.agents.skill_for_role(role);
            let skill_abs = project_root.join(&skill_rel);
            let model = cfg.agents.model_for_role(role, &skill_abs);
            let abbr = role_abbreviation(role);
            format!("{abbr}={model}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(format!("   Modelos     : {models}"));

    // Límites
    let effective_max = effective_max_iter(story_count, cfg.limits.max_iterations);
    let limits_str = if cfg.limits.max_iterations == 0 {
        format!(
            "max_iter={} ({} stories × 6), max_reject={}, timeout={}s",
            effective_max,
            story_count,
            cfg.limits.max_reject_cycles,
            cfg.limits.agent_timeout_seconds
        )
    } else {
        format!(
            "max_iter={}, max_reject={}, timeout={}s",
            effective_max, cfg.limits.max_reject_cycles, cfg.limits.agent_timeout_seconds
        )
    };
    lines.push(format!("   Límites     : {limits_str}"));

    // Git
    let git_str = if cfg.git.enabled {
        "habilitado"
    } else {
        "deshabilitado"
    };
    lines.push(format!("   Git         : {git_str}"));

    // Hooks
    let mut active: Vec<&str> = Vec::new();
    if cfg.hooks.post_qa.is_some() {
        active.push("post_qa");
    }
    if cfg.hooks.post_dev.is_some() {
        active.push("post_dev");
    }
    if cfg.hooks.post_reviewer.is_some() {
        active.push("post_reviewer");
    }
    let hooks_str = if active.is_empty() {
        "ninguno".to_string()
    } else {
        active.join(", ")
    };
    lines.push(format!("   Hooks       : {hooks_str}"));

    format!("{sep}\n{}\n{sep}", lines.join("\n"))
}

/// Calcula el max_iter efectivo: si `cfg_max == 0` usa `max(10, story_count * 6)`.
#[allow(dead_code)]
fn effective_max_iter(story_count: usize, cfg_max: u32) -> u32 {
    if cfg_max > 0 {
        cfg_max
    } else {
        let computed = story_count as u32 * 6;
        computed.max(10)
    }
}

/// Abreviaturas de rol para el header.
#[allow(dead_code)]
fn role_abbreviation(role: &str) -> &str {
    match role {
        "product_owner" => "PO",
        "qa_engineer" => "QA",
        "developer" => "Dev",
        "reviewer" => "Reviewer",
        _ => role,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

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
            compact: false,
        };
        let log_file = Path::new(".regista/logs/regista-log-test.log");

        let args = build_daemon_args("run", ".", None, false, 0, &pipeline, &common, log_file);

        // Verificar estructura: ["run", ".", "--daemon", "--log-file", ".regista/logs/regista-log-test.log", ...]
        assert_eq!(args[0], "run");
        assert_eq!(args[1], ".");
        assert!(args.contains(&"--daemon".to_string()));
        assert!(args.contains(&"--log-file".to_string()));
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
            compact: false,
        };
        let log_file = Path::new(".regista/logs/regista-log-test.log");

        let args = build_daemon_args(
            "plan",
            "myproj",
            Some("spec.md"),
            true,
            10,
            &pipeline,
            &common,
            log_file,
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
        let report = app::pipeline::RunReport {
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
        let report = app::pipeline::RunReport {
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
        let report = app::pipeline::RunReport {
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

    // ═══════════════════════════════════════════════════════════
    // STORY-026: Header de sesión con metadatos
    // ═══════════════════════════════════════════════════════════

    mod story026 {
        use super::*;

        // ── CA1: Header detallado ──────────────────────────────

        /// CA1: En modo detallado, el header contiene todas las
        /// secciones requeridas: versión, timestamp, proyecto,
        /// provider, modelos, límites, git, hooks.
        #[test]
        fn detailed_header_contains_all_sections() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/home/user/project"),
                3,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("regista v1.0.0"));
            assert!(header.contains("2026-05-05 12:00:00 UTC"));
            assert!(header.contains("/home/user/project"));
            assert!(header.contains("Provider"));
            assert!(header.contains("pi"));
            assert!(header.contains("Modelos"));
            assert!(header.contains("Límites"));
            assert!(header.contains("Git"));
            assert!(header.contains("Hooks"));
        }

        /// CA1: El header detallado usa formato de bloque con
        /// líneas de separación ═ y el emoji satélite.
        #[test]
        fn detailed_header_has_block_format() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-01-01 00:00:00",
            );

            assert!(
                header.contains('═'),
                "El header detallado debe usar ═ como borde"
            );
            assert!(
                header.contains("🛰️"),
                "El header detallado debe incluir el emoji satélite"
            );
            assert!(
                header.contains("sesión iniciada"),
                "Debe indicar 'sesión iniciada'"
            );
        }

        /// CA1: El header incluye la ruta del proyecto.
        #[test]
        fn detailed_header_includes_project_path() {
            let cfg = config::Config::default();
            let project_path = "/home/dev/mi-app";
            let header = format_session_header(
                &cfg,
                "2.1.0",
                Path::new(project_path),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.contains(project_path),
                "Debe incluir el path del proyecto: {project_path}"
            );
        }

        /// CA1: El campo Provider muestra el nombre del provider global
        /// (no solo "pi" — debe funcionar con cualquier provider).
        #[test]
        fn detailed_header_shows_configured_provider_name() {
            for provider_name in ["claude", "codex", "opencode"] {
                let toml = format!("[agents]\nprovider = \"{provider_name}\"\n");
                let cfg: config::Config = toml::from_str(&toml).unwrap();
                let header = format_session_header(
                    &cfg,
                    "1.0.0",
                    Path::new("/tmp"),
                    0,
                    false,
                    "2026-05-05 12:00:00",
                );

                assert!(
                    header.contains(&format!("Provider    : {provider_name}"))
                        || header.contains(&format!("Provider: {provider_name}")),
                    "Header debe mostrar provider '{provider_name}' en el campo Provider"
                );
            }
        }

        /// CA1: La línea de Modelos usa el formato exacto con comas:
        /// "PO=X, QA=Y, Dev=Z, Reviewer=W".
        #[test]
        fn detailed_header_models_line_has_comma_separated_format() {
            let toml = r#"
[agents]
provider = "pi"
model = "gpt-5"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            // La línea de modelos debe contener los 4 roles separados por ", "
            let models_line = header
                .lines()
                .find(|l| l.contains("Modelos"))
                .expect("Debe existir una línea 'Modelos'");

            assert!(
                models_line.contains("PO="),
                "Línea de Modelos debe contener PO="
            );
            assert!(
                models_line.contains("QA="),
                "Línea de Modelos debe contener QA="
            );
            assert!(
                models_line.contains("Dev="),
                "Línea de Modelos debe contener Dev="
            );
            assert!(
                models_line.contains("Reviewer="),
                "Línea de Modelos debe contener Reviewer="
            );
            // Debe haber al menos 3 comas (separando los 4 roles)
            let commas = models_line.matches(',').count();
            assert!(
                commas >= 3,
                "Línea de Modelos debe tener al menos 3 comas (4 roles), tiene {commas}"
            );
        }

        // ── CA2: Header compacto ───────────────────────────────

        /// CA2: En modo compacto, el header es una sola línea.
        #[test]
        fn compact_header_is_single_line() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                3,
                true,
                "2026-05-05 12:00:00",
            );

            assert!(
                !header.contains('\n'),
                "El header compacto debe ser una sola línea, sin saltos"
            );
        }

        /// CA2: El header compacto contiene los campos requeridos:
        /// versión, provider, fecha UTC, max_iter.
        #[test]
        fn compact_header_contains_required_fields() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                3,
                true,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("regista v1.0.0"));
            assert!(header.contains("pi"));
            assert!(header.contains("2026-05-05 12:00:00 UTC"));
            assert!(header.contains("max_iter="));
        }

        /// CA2: Los modos detallado y compacto producen salida diferente.
        #[test]
        fn compact_differs_from_detailed() {
            let cfg = config::Config::default();
            let detailed = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );
            let compact = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                true,
                "2026-05-05 12:00:00",
            );

            assert_ne!(detailed, compact, "Detallado ≠ Compacto");
            assert!(
                compact.len() < detailed.len(),
                "Compacto debe ser más corto que detallado"
            );
        }

        // ── CA3: Modelos resueltos con model_for_role ──────────

        /// CA3: Con configuración por defecto, los modelos muestran
        /// "desconocido" (no hay modelo configurado ni YAML).
        #[test]
        fn models_show_desconocido_by_default() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("PO=desconocido"));
            assert!(header.contains("QA=desconocido"));
            assert!(header.contains("Dev=desconocido"));
            assert!(header.contains("Reviewer=desconocido"));
        }

        /// CA3: Cuando se configura un modelo global, todos los roles
        /// lo heredan.
        #[test]
        fn models_inherit_global_model() {
            let toml = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.contains("PO=claude-sonnet-4"),
                "PO debe heredar el modelo global"
            );
            assert!(
                header.contains("QA=claude-sonnet-4"),
                "QA debe heredar el modelo global"
            );
            assert!(
                header.contains("Dev=claude-sonnet-4"),
                "Dev debe heredar el modelo global"
            );
            assert!(
                header.contains("Reviewer=claude-sonnet-4"),
                "Reviewer debe heredar el modelo global"
            );
        }

        /// CA3: El modelo por rol prevalece sobre el global.
        #[test]
        fn role_model_overrides_global_in_header() {
            let toml = r#"
[agents]
provider = "pi"
model = "claude-sonnet-4"

[agents.developer]
model = "gpt-5"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            // Dev tiene modelo explícito
            assert!(header.contains("Dev=gpt-5"));
            // Los demás heredan el global
            assert!(header.contains("PO=claude-sonnet-4"));
            assert!(header.contains("QA=claude-sonnet-4"));
            assert!(header.contains("Reviewer=claude-sonnet-4"));
        }

        /// CA3: La resolución de modelos usa AgentsConfig::model_for_role().
        /// Verifica que el header refleja exactamente lo que devuelve
        /// model_for_role para cada rol, usando la misma resolución de
        /// paths que format_session_header (skill path absoluto desde
        /// project_root).
        #[test]
        fn header_uses_model_for_role_resolution() {
            let tmp = tempfile::tempdir().unwrap();
            let project_root = tmp.path();

            // Crear skills para los 4 roles con modelos YAML distintos
            let role_models = [
                ("product-owner", "po-model-v1"),
                ("qa-engineer", "qa-model-v1"),
                ("developer", "dev-model-v1"),
                ("reviewer", "reviewer-model-v1"),
            ];
            for (role_dir, expected_model) in &role_models {
                let skill_dir = project_root.join(".pi/skills").join(role_dir);
                std::fs::create_dir_all(&skill_dir).unwrap();
                std::fs::write(
                    skill_dir.join("SKILL.md"),
                    format!("---\nname: {role_dir}\nmodel: {expected_model}\n---\n# Skill\n"),
                )
                .unwrap();
            }

            let cfg = config::Config::default();
            let header =
                format_session_header(&cfg, "1.0.0", project_root, 0, false, "2026-05-05 12:00:00");

            // Para cada rol canónico, model_for_role con el mismo skill path
            // absoluto que usa format_session_header debe coincidir con el header.
            for (role, role_abbr) in [
                ("product_owner", "PO"),
                ("qa_engineer", "QA"),
                ("developer", "Dev"),
                ("reviewer", "Reviewer"),
            ] {
                let skill_rel = cfg.agents.skill_for_role(role);
                let skill_abs = project_root.join(&skill_rel);
                let expected_model = cfg.agents.model_for_role(role, &skill_abs);
                let expected_fragment = format!("{role_abbr}={expected_model}");
                assert!(
                    header.contains(&expected_fragment),
                    "Header debe contener '{expected_fragment}' (resolución de model_for_role con path absoluto)"
                );
            }

            // Verificación concreta: los modelos del YAML aparecen en el header
            assert!(header.contains("PO=po-model-v1"));
            assert!(header.contains("QA=qa-model-v1"));
            assert!(header.contains("Dev=dev-model-v1"));
            assert!(header.contains("Reviewer=reviewer-model-v1"));
        }

        // ── CA4: Límites con max_iter efectivo ─────────────────

        /// CA4: Cuando max_iterations=0, el header muestra el valor
        /// auto-calculado (story_count × 6).
        #[test]
        fn limits_shows_effective_max_iter_auto() {
            let cfg = config::Config::default(); // max_iterations = 0
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                5,
                false,
                "2026-05-05 12:00:00",
            );

            // 5 stories × 6 = 30
            assert!(
                header.contains("max_iter=30"),
                "Con 5 historias y max_iterations=0, max_iter debe ser 30"
            );
            assert!(
                header.contains("5 stories × 6") || header.contains("5 historias"),
                "Debe indicar cómo se calculó el max_iter efectivo"
            );
        }

        /// CA4: Cuando max_iterations > 0, el header muestra el valor
        /// explícito y NO menciona stories × 6.
        #[test]
        fn limits_shows_explicit_max_iter() {
            let toml = r#"
[limits]
max_iterations = 10
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                5,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.contains("max_iter=10"),
                "max_iter debe ser el valor explícito 10"
            );
            assert!(
                !header.contains("stories × 6"),
                "Con max_iter explícito, no debe mencionar stories × 6"
            );
        }

        /// CA4: El header incluye max_reject_cycles y agent_timeout_seconds.
        #[test]
        fn limits_includes_reject_and_timeout() {
            let toml = r#"
[limits]
max_iterations = 100
max_reject_cycles = 5
agent_timeout_seconds = 900
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("max_reject=5"));
            assert!(header.contains("timeout=900s"));
        }

        /// CA4: Los valores por defecto de límites aparecen correctamente.
        #[test]
        fn limits_shows_default_values() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            // max_iter floor = 10 cuando story_count=0 y max_iterations=0
            assert!(header.contains("max_iter=10"));
            assert!(header.contains("max_reject=8"));
            assert!(header.contains("timeout=1800s"));
        }

        /// CA4: Cuando max_iterations=0 y story_count=1, el floor
        /// (10) prevalece sobre el cálculo (1 × 6 = 6).
        #[test]
        fn limits_shows_floor_when_auto_below_minimum() {
            let cfg = config::Config::default(); // max_iterations = 0
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                1,
                false,
                "2026-05-05 12:00:00",
            );

            // 1 story × 6 = 6, pero effective_max_iterations usa .max(10) = 10
            assert!(
                header.contains("max_iter=10"),
                "Con 1 historia, max_iter debe ser 10 (floor), no 6"
            );
        }

        /// CA4: Cuando max_iterations=0 y story_count=2, el cálculo
        /// (2 × 6 = 12) prevalece sobre el floor (10).
        #[test]
        fn limits_shows_auto_above_floor() {
            let cfg = config::Config::default(); // max_iterations = 0
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                2,
                false,
                "2026-05-05 12:00:00",
            );

            // 2 stories × 6 = 12, que es > floor 10
            assert!(
                header.contains("max_iter=12"),
                "Con 2 historias, max_iter debe ser 12 (2 × 6)"
            );
        }

        // ── CA5: Estado de git ─────────────────────────────────

        /// CA5: Con git.enabled = true, muestra "habilitado".
        #[test]
        fn git_enabled_shows_habilitado() {
            let toml = r#"
[git]
enabled = true
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.to_lowercase().contains("habilitado"),
                "Con git.enabled=true debe mostrar 'habilitado'"
            );
        }

        /// CA5: Con git.enabled = false, muestra "deshabilitado".
        #[test]
        fn git_disabled_shows_deshabilitado() {
            let toml = r#"
[git]
enabled = false
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.to_lowercase().contains("deshabilitado"),
                "Con git.enabled=false debe mostrar 'deshabilitado'"
            );
        }

        // ── CA6: Hooks configurados ────────────────────────────

        /// CA6: Con hooks configurados, el header lista los hooks activos.
        #[test]
        fn hooks_lists_active_hooks() {
            let toml = r#"
[hooks]
post_qa = "cargo test"
post_dev = "cargo build --release"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.contains("post_qa"),
                "Debe listar post_qa cuando está configurado"
            );
            assert!(
                header.contains("post_dev"),
                "Debe listar post_dev cuando está configurado"
            );
        }

        /// CA6: Sin hooks configurados, muestra "ninguno".
        #[test]
        fn hooks_shows_ninguno_when_empty() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.to_lowercase().contains("ninguno"),
                "Sin hooks configurados debe mostrar 'ninguno'"
            );
        }

        /// CA6: Con todos los hooks configurados, lista los tres.
        #[test]
        fn hooks_lists_all_three_when_configured() {
            let toml = r#"
[hooks]
post_qa = "npm test"
post_dev = "npm run build"
post_reviewer = "npm run lint"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("post_qa"));
            assert!(header.contains("post_dev"));
            assert!(header.contains("post_reviewer"));
        }

        // ── CA7: Emisión vía tracing::info! ────────────────────

        /// CA7: La función retorna un String no vacío que puede
        /// pasarse directamente a `tracing::info!`.
        ///
        /// El Developer integrará la llamada `tracing::info!("{}", header)`
        /// en setup_daemon_tracing() o inmediatamente después en los
        /// handlers plan/auto/run.
        #[test]
        fn header_is_suitable_for_tracing_info_macro() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                !header.is_empty(),
                "El header no debe estar vacío; debe ser un mensaje válido para tracing::info!"
            );
            assert!(
                header.is_ascii() || header.contains("🛰️"),
                "El header contiene caracteres válidos para logging"
            );
        }

        /// CA7: La función existe en el scope del módulo handlers
        /// y puede ser invocada desde el contexto de los handlers
        /// del daemon (plan/auto/run).
        #[test]
        fn header_function_is_callable_from_daemon_context() {
            // Simula el contexto de un handler daemon:
            // - Config está cargada
            // - El project root es conocido
            // - Se conoce el número de historias
            let cfg = config::Config::default();
            let project_root = Path::new("/app");

            // Esto es lo que haría el handler daemon:
            let _header: String = format_session_header(
                &cfg,
                env!("CARGO_PKG_VERSION"),
                project_root,
                10, // story_count
                false,
                "2026-05-05 12:00:00",
            );
            // Si compila y no paniquea, el header se puede integrar
            // en el flujo daemon: setup_daemon_tracing(...); tracing::info!("{}", header);
        }

        // ── CA3 (extensión): YAML frontmatter ─────────────────

        /// CA3: Cuando no hay modelo en config pero el skill tiene
        /// modelo en YAML frontmatter, el header refleja ese modelo.
        ///
        /// Este test crea la estructura de skills esperada dentro de
        /// un directorio temporal usado como project_root, para que
        /// model_for_role pueda leer el YAML frontmatter.
        #[test]
        fn header_reflects_yaml_frontmatter_model() {
            let tmp = tempfile::tempdir().unwrap();
            let project_root = tmp.path();

            // Crear estructura de skill de pi para developer
            let skill_dir = project_root.join(".pi/skills/developer");
            std::fs::create_dir_all(&skill_dir).unwrap();
            let skill_file = skill_dir.join("SKILL.md");
            std::fs::write(
                &skill_file,
                "---\nname: developer\nmodel: opencode/gpt-5-nano\n---\n# Developer skill\n",
            )
            .unwrap();

            // Config sin modelo: el modelo debe venir del YAML
            let cfg = config::Config::default();

            // El header debe contener Dev=opencode/gpt-5-nano
            let header =
                format_session_header(&cfg, "1.0.0", project_root, 0, false, "2026-05-05 12:00:00");
            assert!(
                header.contains("Dev=opencode/gpt-5-nano"),
                "Header debe reflejar el modelo del YAML frontmatter: Dev=opencode/gpt-5-nano"
            );
        }

        // ── CA6 (extensión): solo hooks configurados ───────────

        /// CA6: Cuando solo un hook está configurado (post_reviewer),
        /// el header lista SOLO ese hook, no los otros.
        #[test]
        fn hooks_lists_only_configured_hook_not_all() {
            let toml = r#"
[hooks]
post_reviewer = "cargo clippy -- -D warnings"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(
                header.contains("post_reviewer"),
                "Debe listar post_reviewer porque está configurado"
            );
            // Los hooks no configurados NO deben aparecer
            assert!(
                !header.contains("post_qa"),
                "post_qa NO debe aparecer porque no está configurado"
            );
            assert!(
                !header.contains("post_dev"),
                "post_dev NO debe aparecer porque no está configurado"
            );
        }

        /// CA6: Con hooks parcialmente configurados (solo post_dev),
        /// el header lista solo post_dev, no menciona los otros.
        #[test]
        fn hooks_lists_only_post_dev_when_its_the_only_one_configured() {
            let toml = r#"
[hooks]
post_dev = "npm run build"
"#;
            let cfg: config::Config = toml::from_str(toml).unwrap();
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                0,
                false,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("post_dev"), "Debe listar post_dev");
            assert!(!header.contains("post_qa"), "post_qa NO debe aparecer");
            assert!(
                !header.contains("post_reviewer"),
                "post_reviewer NO debe aparecer"
            );
        }

        // ── CA4 (extensión): notación "stories × 6" ────────────

        /// CA4: Cuando max_iter es auto-calculado (max_iterations=0),
        /// la línea de Límites incluye la notación "(M stories × 6)"
        /// para mostrar cómo se obtuvo el valor.
        #[test]
        fn limits_line_shows_stories_multiplier_notation_when_auto() {
            let cfg = config::Config::default(); // max_iterations = 0
            let header = format_session_header(
                &cfg,
                "1.0.0",
                Path::new("/tmp"),
                7,
                false,
                "2026-05-05 12:00:00",
            );

            let limits_line = header
                .lines()
                .find(|l| l.contains("Límites") || l.contains("Limites"))
                .expect("Debe existir una línea de Límites");

            // Debe contener max_iter=42 (7 × 6)
            assert!(
                limits_line.contains("max_iter=42"),
                "Límites debe mostrar max_iter=42 para 7 historias"
            );

            // Debe mostrar la notación de cómo se calculó
            let has_multiplier = limits_line.contains("7") && limits_line.contains('×')
                || limits_line.contains("7 stories")
                || limits_line.contains("7 historias");
            assert!(
                has_multiplier,
                "Límites debe indicar que max_iter se calculó como 7 stories × 6"
            );
        }

        // ── Sanity / regresión ─────────────────────────────────

        /// El header NO debe paniquear cuando las rutas de skills no
        /// existen en disco (model_for_role lo maneja con fallback).
        #[test]
        fn header_does_not_panic_with_nonexistent_skill_paths() {
            let cfg = config::Config::default();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                format_session_header(
                    &cfg,
                    "1.0.0",
                    Path::new("/tmp"),
                    0,
                    false,
                    "2026-05-05 12:00:00",
                )
            }));
            assert!(
                result.is_ok(),
                "format_session_header no debe paniquear con paths de skill inexistentes"
            );
        }

        /// El header en modo compacto también funciona con max_iter
        /// auto-calculado.
        #[test]
        fn compact_header_shows_auto_max_iter() {
            let cfg = config::Config::default();
            let header = format_session_header(
                &cfg,
                "2.0.0",
                Path::new("/tmp"),
                4,
                true,
                "2026-05-05 12:00:00",
            );

            assert!(header.contains("regista v2.0.0"));
            assert!(header.contains("pi"));
            assert!(header.contains("2026-05-05 12:00:00 UTC"));
            // 4 stories × 6 = 24
            assert!(header.contains("max_iter=24"));
        }
    }
}
