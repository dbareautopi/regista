//! regista — 🎬 AI agent director for pi.
//!
//! Escanea historias en un directorio configurable y dispara agentes
//! según una máquina de estados fija: Draft → Ready → Tests Ready →
//! In Review → Business Review → Done, con detección de deadlocks.
//!
//! Totalmente agnóstico al proyecto anfitrión: se configura mediante
//! un archivo `.regista.toml` en la raíz del proyecto.

mod agent;
mod config;
mod daemon;
mod deadlock;
mod dependency_graph;
mod git;
mod hooks;
mod orchestrator;
mod prompts;
mod state;
mod story;

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

    /// Ruta específica para el archivo de log.
    #[arg(long)]
    pub log_file: Option<String>,

    /// Flag interno: indica que este proceso es el hijo daemon lanzado por --detach.
    #[arg(long, hide = true)]
    pub daemon: bool,
}

fn main() {
    let cli = Cli::parse();
    let project_root = Path::new(&cli.project_dir);

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
                println!("Log: {}", cli.log_file.as_deref().unwrap_or(".regista.log"));
                println!("Usa --status para consultar, --follow para ver el log, --kill para detener.");
            }
            Err(e) => {
                eprintln!("Error al lanzar daemon: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // ── Configurar logging ──────────────────────────────────────────────

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter);

    if let Some(ref log_file) = cli.log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .expect("No se pudo crear/abrir el archivo de log");
        subscriber.with_writer(std::sync::Mutex::new(file)).init();
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

    let cfg = match config::Config::load(project_root, config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Error al cargar configuración: {e}");
            std::process::exit(1);
        }
    };

    tracing::info!(
        "Configuración cargada: stories_dir={}, agents={{ PO={}, QA={}, Dev={}, Reviewer={} }}",
        cfg.project.stories_dir, cfg.agents.product_owner, cfg.agents.qa_engineer,
        cfg.agents.developer, cfg.agents.reviewer
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
        Some((parts[0].trim().to_uppercase(), parts[1].trim().to_uppercase()))
    } else {
        None
    };

    let run_options = orchestrator::RunOptions {
        once: cli.once,
        story_filter: cli.story.clone(),
        epic_filter: cli.epic.clone(),
        epics_range,
    };

    tracing::info!(
        "Filtros: story={:?}, epic={:?}, epics_range={:?}, once={}",
        run_options.story_filter, run_options.epic_filter,
        run_options.epics_range, run_options.once
    );

    // ── Ejecutar pipeline ───────────────────────────────────────────────

    tracing::info!("🚀 Iniciando pipeline...");

    match orchestrator::run(project_root, &cfg, &run_options) {
        Ok(report) => {
            tracing::info!("╔══════════════════════════════════╗");
            tracing::info!("║     🏁 Pipeline completado      ║");
            tracing::info!("╠══════════════════════════════════╣");
            tracing::info!("║ Historias totales:   {:>4}       ║", report.total);
            tracing::info!("║ Done:                {:>4}       ║", report.done);
            tracing::info!("║ Failed:              {:>4}       ║", report.failed);
            tracing::info!("║ Iteraciones:         {:>4}       ║", report.iterations);
            tracing::info!("║ Tiempo:              {:>4}s      ║", report.elapsed.as_secs());
            tracing::info!("╚══════════════════════════════════╝");
        }
        Err(e) => {
            tracing::error!("❌ Pipeline falló: {e}");
            std::process::exit(1);
        }
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
        let args =
            Cli::try_parse_from(["regista", "/tmp/proj", "--story", "STORY-001"]).unwrap();
        assert_eq!(args.project_dir, "/tmp/proj");
        assert_eq!(args.story.unwrap(), "STORY-001");
    }

    #[test]
    fn cli_detach_conflicts_with_follow() {
        let err = Cli::try_parse_from([
            "regista",
            ".",
            "--detach",
            "--follow",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("--detach"), "expected conflict error, got: {err}");
    }

    #[test]
    fn cli_daemon_flag_is_hidden() {
        let args = Cli::try_parse_from(["regista", ".", "--daemon"]).unwrap();
        assert!(args.daemon);
    }
}
