//! regista — 🎬 AI agent director for pi.
//!
//! Escanea historias en un directorio configurable y dispara agentes
//! según una máquina de estados fija: Draft → Ready → Tests Ready →
//! In Review → Business Review → Done, con detección de deadlocks.
//!
//! Totalmente agnóstico al proyecto anfitrión: se configura mediante
//! un archivo `.regista.toml` en la raíz del proyecto.

mod agent;
mod checkpoint;
mod config;
mod daemon;
mod deadlock;
mod dependency_graph;
mod git;
mod groom;
mod hooks;
mod init;
mod orchestrator;
mod prompts;
mod providers;
mod state;
mod story;
mod validator;

use clap::Parser;
use std::path::Path;

/// 🎬 regista — AI agent director.
///
/// Dado un directorio de proyecto que contenga un archivo `.regista.toml`
/// e historias de usuario en el formato esperado, ejecuta el pipeline completo
/// de forma autónoma: PO → QA → Dev → Reviewer → PO → Done.
#[derive(Parser, Debug)]
#[command(name = "regista", version, about)]
pub struct Cli {
    /// Directorio raíz del proyecto a orquestar.
    /// Debe contener un archivo `.regista.toml` (salvo que se indique otro con --config).
    #[arg(default_value = ".")]
    pub project_dir: String,

    /// Ruta al archivo de configuración TOML.
    /// Por defecto: <PROJECT_DIR>/.regista.toml
    #[arg(long)]
    pub config: Option<String>,

    /// Filtrar por rango de épicas. Ejemplo: "EPIC-001..EPIC-003"
    #[arg(long, conflicts_with = "epic")]
    pub epics: Option<String>,

    /// Filtrar por una sola épica. Ejemplo: "EPIC-001"
    #[arg(long)]
    pub epic: Option<String>,

    /// Procesar solo una historia concreta (ej: "STORY-001").
    #[arg(long, conflicts_with_all = ["epics", "epic"])]
    pub story: Option<String>,

    /// Ejecutar una sola iteración del pipeline y salir.
    #[arg(long)]
    pub once: bool,

    /// Salida JSON estructurada a stdout (para CI/CD).
    #[arg(long)]
    pub json: bool,

    /// Suprimir logs de progreso (solo errores).
    #[arg(long)]
    pub quiet: bool,

    /// Modo simulación: no invoca agentes ni modifica archivos.
    #[arg(long)]
    pub dry_run: bool,

    /// Reanudar desde el último checkpoint guardado.
    #[arg(long)]
    pub resume: bool,

    /// Borrar el checkpoint sin ejecutar el pipeline.
    #[arg(long)]
    pub clean_state: bool,

    /// Lanzar en segundo plano (modo daemon). El proceso sobrevive a la desconexión SSH.
    #[arg(long, conflicts_with_all = ["follow", "status", "kill"])]
    pub detach: bool,

    /// Ver el log en vivo de un orquestador lanzado con --detach.
    #[arg(long, conflicts_with_all = ["detach", "status", "kill", "once", "epics", "epic", "story", "config"])]
    pub follow: bool,

    /// Consultar si el orquestador en segundo plano sigue corriendo.
    #[arg(long, conflicts_with_all = ["detach", "follow", "kill", "once", "epics", "epic", "story", "config"])]
    pub status: bool,

    /// Detener el orquestador en segundo plano.
    #[arg(long, conflicts_with_all = ["detach", "follow", "status", "once", "epics", "epic", "story", "config"])]
    pub kill: bool,

    /// Provider de agente a usar (pi, claude, codex, opencode).
    /// Sobreescribe el provider definido en .regista/config.toml.
    #[arg(long)]
    pub provider: Option<String>,

    /// Ruta específica para el archivo de log.
    #[arg(long)]
    pub log_file: Option<String>,

    /// Flag interno: indica que este proceso es el hijo daemon lanzado por --detach.
    #[arg(long, hide = true)]
    pub daemon: bool,
}

fn main() {
    // ── Detectar subcomandos "validate" e "init" antes de clap ──────
    let raw_args: Vec<String> = std::env::args().collect();
    if raw_args.len() > 1 {
        match raw_args[1].as_str() {
            "validate" => return run_validate(&raw_args[2..]),
            "init" => return run_init(&raw_args[2..]),
            "groom" => return run_groom(&raw_args[2..]),
            "help" => return run_help(),
            _ => {}
        }
    }

    let cli = Cli::parse();
    let project_root = Path::new(&cli.project_dir);

    // ── Limpiar checkpoint y salir ────────────────────────────────
    if cli.clean_state {
        checkpoint::OrchestratorState::remove(project_root);
        println!("✅ Checkpoint eliminado.");
        return;
    }

    // ── Comandos de gestión del daemon (salen inmediatamente) ───────────

    if cli.follow {
        if let Err(e) = daemon::follow(project_root) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    if cli.status {
        match daemon::status(project_root) {
            Ok(msg) => println!("{msg}"),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if cli.kill {
        match daemon::kill(project_root) {
            Ok(msg) => println!("{msg}"),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if cli.detach {
        let log_file_override = cli.log_file.as_ref().map(|p| Path::new(p.as_str()));
        match daemon::detach(project_root, log_file_override) {
            Ok(pid) => {
                println!("Daemon lanzado con PID: {pid}");
                println!(
                    "Log: {}",
                    cli.log_file.as_deref().unwrap_or(".regista/daemon.log")
                );
                println!(
                    "Usa --status para consultar, --follow para ver el log, --kill para detener."
                );
            }
            Err(e) => {
                eprintln!("Error al lanzar daemon: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // ── Configurar logging ──────────────────────────────────────────────

    let env_filter = if cli.quiet {
        tracing_subscriber::EnvFilter::new("error")
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };

    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

    if let Some(ref log_file) = cli.log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .expect("No se pudo crear/abrir el archivo de log");
        subscriber.with_writer(std::sync::Mutex::new(file)).init();
    } else if cli.json {
        // En modo JSON, logs a stderr, reporte a stdout
        subscriber.with_writer(std::io::stderr).init();
    } else {
        subscriber.with_writer(std::io::stderr).init();
    }

    tracing::info!("regista v{} — arrancando", env!("CARGO_PKG_VERSION"));
    tracing::info!("project_dir = {}", cli.project_dir);

    // ── Limpieza de PID al salir (solo en modo daemon hijo) ─────────────

    let _pid_cleanup = if cli.daemon {
        let canonical = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        Some(daemon::PidCleanup(canonical))
    } else {
        None
    };

    // ── Cargar configuración ────────────────────────────────────────────

    let config_path = cli.config.as_ref().map(|p| Path::new(p.as_str()));

    let mut cfg = match config::Config::load(project_root, config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Error al cargar configuración: {e}");
            if cli.json {
                output_json_error(&e.to_string());
            }
            std::process::exit(1);
        }
    };

    // Sobreescribir provider si se pasó --provider
    if let Some(ref provider) = cli.provider {
        cfg.agents.provider = provider.clone();
        tracing::info!("Provider override: {provider}");
    }

    tracing::info!(
        "Configuración cargada: stories_dir={}, agents={{ provider={}, PO={}, QA={}, Dev={}, Reviewer={} }}",
        cfg.project.stories_dir,
        cfg.agents.provider,
        cfg.agents.skill_for_role("product_owner"),
        cfg.agents.skill_for_role("qa_engineer"),
        cfg.agents.skill_for_role("developer"),
        cfg.agents.skill_for_role("reviewer")
    );

    // ── Opciones de ejecución ───────────────────────────────────────────

    let epics_range = if let Some(ref range) = cli.epics {
        let parts: Vec<&str> = range.split("..").collect();
        if parts.len() != 2 {
            tracing::error!(
                "Formato de rango inválido: '{}'. Use 'EPIC-001..EPIC-003'",
                range
            );
            std::process::exit(1);
        }
        Some((
            parts[0].trim().to_uppercase(),
            parts[1].trim().to_uppercase(),
        ))
    } else {
        None
    };

    let run_options = orchestrator::RunOptions {
        once: cli.once,
        story_filter: cli.story.clone(),
        epic_filter: cli.epic.clone(),
        epics_range,
        dry_run: cli.dry_run,
        quiet: cli.quiet || cli.json,
    };

    tracing::info!(
        "Filtros: story={:?}, epic={:?}, epics_range={:?}, once={}, dry_run={}",
        run_options.story_filter,
        run_options.epic_filter,
        run_options.epics_range,
        run_options.once,
        run_options.dry_run
    );

    // ── Ejecutar pipeline ───────────────────────────────────────────────

    let resume_state = if cli.resume {
        checkpoint::OrchestratorState::load(project_root)
    } else {
        None
    };

    tracing::info!("🚀 Iniciando pipeline...");

    match orchestrator::run(project_root, &cfg, &run_options, resume_state) {
        Ok(report) => {
            if cli.json {
                output_json_report(&report, &cli.project_dir);
            } else {
                if let Some(ref reason) = report.stop_reason {
                    tracing::info!("╔══════════════════════════════════╗");
                    tracing::info!("║  ⚠️  Pipeline detenido (límite)  ║");
                    tracing::info!("╠══════════════════════════════════╣");
                    tracing::info!("║ Razón: {:<23} ║", reason);
                } else {
                    tracing::info!("╔══════════════════════════════════╗");
                    tracing::info!("║     🏁 Pipeline completado      ║");
                    tracing::info!("╠══════════════════════════════════╣");
                }
                tracing::info!("║ Historias totales:   {:>4}       ║", report.total);
                tracing::info!("║ Done:                {:>4}       ║", report.done);
                tracing::info!("║ Failed:              {:>4}       ║", report.failed);
                tracing::info!("║ Blocked:             {:>4}       ║", report.blocked);
                tracing::info!("║ Draft:               {:>4}       ║", report.draft);
                tracing::info!("║ Iteraciones:         {:>4}       ║", report.iterations);
                tracing::info!(
                    "║ Tiempo:              {:>4}s      ║",
                    report.elapsed.as_secs()
                );
                tracing::info!("╚══════════════════════════════════╝");
            }

            // Exit code según resultado
            std::process::exit(exit_code_from_report(&report));
        }
        Err(e) => {
            tracing::error!("❌ Pipeline falló: {e}");
            if cli.json {
                output_json_error(&e.to_string());
            }
            std::process::exit(1);
        }
    }
}

/// Ejecuta el subcomando `validate`.
fn run_validate(args: &[String]) {
    let project_dir = if args.first().map(|s| s.starts_with('-')).unwrap_or(true) {
        "."
    } else {
        args[0].as_str()
    };
    let json = args.iter().any(|a| a == "--json");
    let config = args
        .iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());
    let _provider_override = args
        .iter()
        .position(|a| a == "--provider")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_lowercase());

    let project_root = Path::new(project_dir);
    let config_path = config.map(Path::new);

    let result = validator::validate(project_root, config_path);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into())
        );
    } else {
        // Salida legible
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

    // Exit code
    if result.errors > 0 {
        std::process::exit(1);
    } else if result.warnings > 0 {
        std::process::exit(2);
    }
    // else: exit 0 (OK)
}

/// Ejecuta el subcomando `init`.
fn run_init(args: &[String]) {
    let project_dir = if args.first().map(|s| s.starts_with('-')).unwrap_or(true) {
        "."
    } else {
        args[0].as_str()
    };
    let light = args.iter().any(|a| a == "--light");
    let with_example = args.iter().any(|a| a == "--with-example");
    let provider = args
        .iter()
        .position(|a| a == "--provider")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("pi");

    let project_root = Path::new(project_dir);

    match init::init(project_root, light, with_example, provider) {
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
                if !light {
                    println!("💡 Configura las skills en .pi/skills/ según tu stack.");
                }
            }
        }
        Err(e) => {
            eprintln!("Error inicializando proyecto: {e}");
            std::process::exit(1);
        }
    }
}

/// Ejecuta el subcomando `groom`.
fn run_groom(args: &[String]) {
    if args.is_empty() || args[0].starts_with('-') {
        eprintln!("Uso: regista groom <SPEC.md> [--max-stories N] [--merge|--replace] [--provider pi|claude|codex|opencode] [--run] [...]");
        std::process::exit(1);
    }

    let spec_path_str = &args[0];
    let max_stories: u32 = args
        .iter()
        .position(|a| a == "--max-stories")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let replace = args.iter().any(|a| a == "--replace");
    let config = args
        .iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());
    let provider_override = args
        .iter()
        .position(|a| a == "--provider")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_lowercase());

    // ── Flags de pipeline para --run ──────────────────────────────
    let run_after = args.iter().any(|a| a == "--run");
    let once = args.iter().any(|a| a == "--once");
    let story_filter = args
        .iter()
        .position(|a| a == "--story")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let epic_filter = args
        .iter()
        .position(|a| a == "--epic")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let epics_range_str = args
        .iter()
        .position(|a| a == "--epics")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let json = args.iter().any(|a| a == "--json");
    let quiet = args.iter().any(|a| a == "--quiet");
    let resume = args.iter().any(|a| a == "--resume");

    // Parsear rango de épicas
    let epics_range = if let Some(ref range) = epics_range_str {
        let parts: Vec<&str> = range.split("..").collect();
        if parts.len() != 2 {
            eprintln!(
                "Formato de rango inválido: '{}'. Use 'EPIC-001..EPIC-003'",
                range
            );
            std::process::exit(1);
        }
        Some((
            parts[0].trim().to_uppercase(),
            parts[1].trim().to_uppercase(),
        ))
    } else {
        None
    };

    let spec_path = Path::new(spec_path_str);
    // El directorio del proyecto es el dir padre del spec, o el actual
    let project_root = spec_path.parent().unwrap_or_else(|| Path::new("."));

    let config_path = config.map(Path::new);

    let mut cfg = match config::Config::load(project_root, config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error al cargar configuración: {e}");
            std::process::exit(1);
        }
    };

    // Aplicar override de provider (afecta tanto al groom como al pipeline)
    if let Some(ref provider) = provider_override {
        cfg.agents.provider = provider.clone();
    }

    // Configurar logging para el groom
    let env_filter = if quiet || json {
        tracing_subscriber::EnvFilter::new("error")
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("regista v{} — groom", env!("CARGO_PKG_VERSION"));
    tracing::info!("spec: {}", spec_path.display());
    tracing::info!("project: {}", project_root.display());

    match groom::run(project_root, spec_path, &cfg, max_stories, replace) {
        Ok(result) => {
            // En modo --run --json, suprimimos la salida legible del groom
            // para no contaminar el stdout (que lleva el JSON del pipeline)
            if !(run_after && json) {
                println!(
                    "\n✅ Groom completado en {} iteraciones.",
                    result.iterations
                );
                println!("   Historias generadas: {}", result.stories_created);
                println!("   Épicas generadas:    {}", result.epics_created);
                if result.dependencies_clean {
                    println!("   Grafo de dependencias: limpio ✅");
                } else {
                    println!("   Grafo de dependencias: con errores ⚠️");
                    println!("   Ejecuta `regista validate` para ver los detalles.");
                }
            }

            if run_after {
                // ── Lanzar pipeline automáticamente tras el groom ──────
                if result.stories_created == 0 {
                    tracing::warn!("⚠️  No hay historias que procesar. Omitiendo pipeline.");
                    return;
                }
                if !result.dependencies_clean {
                    tracing::warn!(
                        "⚠️  Grafo de dependencias con errores. Omitiendo pipeline.\n    Ejecuta `regista validate` y corrige las historias antes."
                    );
                    return;
                }

                // ── Validación completa pre-pipeline ──────────────────
                tracing::info!("🔍 Ejecutando validación completa...");
                let validation = validator::validate(project_root, config_path);

                if validation.errors > 0 {
                    tracing::warn!(
                        "⚠️  Validación encontró {} error(es). Omitiendo pipeline.",
                        validation.errors
                    );
                    for finding in &validation.findings {
                        if finding.severity == validator::Severity::Error {
                            tracing::warn!("  ❌ [{}] {}", finding.category, finding.message);
                        }
                    }
                    tracing::warn!("    Corrige los errores y vuelve a ejecutar.");
                    return;
                }

                if validation.warnings > 0 {
                    tracing::warn!(
                        "⚠️  Validación encontró {} warning(s). Continuando de todos modos.",
                        validation.warnings
                    );
                    for finding in &validation.findings {
                        if finding.severity == validator::Severity::Warning {
                            tracing::warn!("  ⚠️  [{}] {}", finding.category, finding.message);
                        }
                    }
                } else {
                    tracing::info!("✅ Validación completa: todo OK.");
                }

                tracing::info!("🚀 Iniciando pipeline automático tras groom...");

                let run_options = orchestrator::RunOptions {
                    once,
                    story_filter,
                    epic_filter,
                    epics_range,
                    dry_run,
                    quiet: quiet || json,
                };

                let resume_state = if resume {
                    checkpoint::OrchestratorState::load(project_root)
                } else {
                    None
                };

                match orchestrator::run(project_root, &cfg, &run_options, resume_state) {
                    Ok(report) => {
                        if json {
                            output_json_report(&report, &project_root.display().to_string());
                        } else {
                            if let Some(ref reason) = report.stop_reason {
                                tracing::info!("╔══════════════════════════════════╗");
                                tracing::info!("║  ⚠️  Pipeline detenido (límite)  ║");
                                tracing::info!("╠══════════════════════════════════╣");
                                tracing::info!("║ Razón: {:<23} ║", reason);
                            } else {
                                tracing::info!("╔══════════════════════════════════╗");
                                tracing::info!("║     🏁 Pipeline completado      ║");
                                tracing::info!("╠══════════════════════════════════╣");
                            }
                            tracing::info!("║ Historias totales:   {:>4}       ║", report.total);
                            tracing::info!("║ Done:                {:>4}       ║", report.done);
                            tracing::info!("║ Failed:              {:>4}       ║", report.failed);
                            tracing::info!("║ Blocked:             {:>4}       ║", report.blocked);
                            tracing::info!("║ Draft:               {:>4}       ║", report.draft);
                            tracing::info!(
                                "║ Iteraciones:         {:>4}       ║",
                                report.iterations
                            );
                            tracing::info!(
                                "║ Tiempo:              {:>4}s      ║",
                                report.elapsed.as_secs()
                            );
                            tracing::info!("╚══════════════════════════════════╝");
                        }
                        std::process::exit(exit_code_from_report(&report));
                    }
                    Err(e) => {
                        tracing::error!("❌ Pipeline falló: {e}");
                        if json {
                            output_json_error(&e.to_string());
                        }
                        std::process::exit(1);
                    }
                }
            } else if result.stories_created > 0 {
                println!("\n   🚀 Siguiente paso: regista --dry-run");
            }
        }
        Err(e) => {
            eprintln!("\n❌ Groom falló: {e}");
            std::process::exit(1);
        }
    }
}

/// Muestra la ayuda de regista.
fn run_help() {
    println!(
        "regista v{} — 🎬 AI agent director para pi",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("COMANDOS:");
    println!("  regista [DIR]             Ejecutar el pipeline completo");
    println!("  regista validate [DIR]    Validar proyecto (config, historias, dependencias)");
    println!("  regista init [DIR]        Crear estructura inicial del proyecto");
    println!("  regista groom <SPEC.md>   Generar historias desde especificación");
    println!("  regista help              Mostrar esta ayuda");
    println!();
    println!("FLAGS DEL PIPELINE:");
    println!("  --config <PATH>      Ruta al archivo .regista/config.toml");
    println!("  --story <ID>         Procesar solo una historia (STORY-001)");
    println!("  --epic <ID>          Filtrar por épica (EPIC-001)");
    println!("  --epics <RANGO>      Filtrar por rango (EPIC-001..EPIC-003)");
    println!("  --once               Una sola iteración y salir");
    println!("  --dry-run            Simular sin ejecutar agentes (sin coste)");
    println!("  --json               Salida JSON a stdout para CI/CD");
    println!("  --quiet              Suprimir logs, solo errores");
    println!("  --resume             Reanudar desde el último checkpoint");
    println!("  --clean-state        Borrar el checkpoint");
    println!();
    println!("FLAGS DEL DAEMON:");
    println!("  --detach             Lanzar en segundo plano");
    println!("  --log-file <PATH>    Archivo de log (default: .regista/daemon.log)");
    println!("  --follow             Ver log en vivo");
    println!("  --status             Consultar si el daemon sigue corriendo");
    println!("  --kill               Detener el daemon");
    println!();
    println!("ESTRUCTURA DEL PROYECTO (todo bajo .regista/):");
    println!("  .regista/config.toml   Configuración del pipeline");
    println!("  .regista/stories/      Historias de usuario (*.md)");
    println!("  .regista/epics/        Épicas");
    println!("  .regista/decisions/    Decisiones documentadas por los agentes");
    println!("  .regista/logs/         Logs del orquestador");
    println!("  .regista/state.toml    Checkpoint para --resume");
    println!("  .pi/skills/            Skills de pi (PO, QA, Dev, Reviewer)");
    println!();
    println!("Para empezar: regista init --with-example && regista --dry-run");
}

/// Vuelca el reporte en JSON a stdout.
fn output_json_report(report: &orchestrator::RunReport, project_dir: &str) {
    let json = serde_json::json!({
        "regista_version": env!("CARGO_PKG_VERSION"),
        "project_dir": project_dir,
        "result": if report.failed > 0 { "completed_with_failures" } else { "completed" },
        "exit_code": exit_code_from_report(report),
        "stopped_early": report.stop_reason.is_some(),
        "stop_reason": report.stop_reason,
        "summary": {
            "total": report.total,
            "done": report.done,
            "failed": report.failed,
            "blocked": report.blocked,
            "draft": report.draft,
            "iterations": report.iterations,
            "elapsed_seconds": report.elapsed_seconds
        },
        "stories": report.stories
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".into())
    );
}

/// Vuelca un error en formato JSON a stdout.
fn output_json_error(error: &str) {
    let json = serde_json::json!({
        "regista_version": env!("CARGO_PKG_VERSION"),
        "result": "error",
        "exit_code": 1,
        "error": error
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".into())
    );
}

/// Calcula el exit code según el reporte.
///
/// 0 = pipeline completo, 0 historias Failed
/// 2 = pipeline completo, ≥1 historias Failed
/// 3 = parada temprana (max_iterations o max_wall_time)
fn exit_code_from_report(report: &orchestrator::RunReport) -> i32 {
    if report.stop_reason.is_some() {
        3
    } else if report.failed > 0 {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_defaults() {
        let args = Cli::try_parse_from(["regista", "."]).unwrap();
        assert_eq!(args.project_dir, ".");
        assert!(!args.once);
        assert!(!args.detach);
        assert!(!args.daemon);
        assert!(args.config.is_none());
        assert!(args.story.is_none());
    }

    #[test]
    fn cli_with_story() {
        let args = Cli::try_parse_from(["regista", "/tmp/proj", "--story", "STORY-001"]).unwrap();
        assert_eq!(args.project_dir, "/tmp/proj");
        assert_eq!(args.story.unwrap(), "STORY-001");
    }

    #[test]
    fn cli_detach_conflicts_with_follow() {
        let err = Cli::try_parse_from(["regista", ".", "--detach", "--follow"]).unwrap_err();
        assert!(
            err.to_string().contains("--detach"),
            "expected conflict error, got: {err}"
        );
    }

    #[test]
    fn cli_daemon_flag_is_hidden() {
        let args = Cli::try_parse_from(["regista", ".", "--daemon"]).unwrap();
        assert!(args.daemon);
    }
}
