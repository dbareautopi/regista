use std::path::{Path, PathBuf};

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
}
