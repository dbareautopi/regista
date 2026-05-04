//! regista — 🎬 AI agent director.
//!
//! Orquestador genérico de agentes para pi, Claude Code, Codex y OpenCode.
//! Pipeline con 3 modos: plan, auto (plan + pipeline), run (pipeline).
//! Toda ejecución es en modo daemon (background). Usa --logs para ver el progreso.

mod app;
mod cli;
mod config;
mod domain;
mod infra;

use clap::Parser;

fn main() {
    let cli = cli::args::Cli::parse();
    cli::handlers::dispatch(cli);
}
