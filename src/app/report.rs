//! Report types and builder for pipeline execution results.
//!
//! Extracted from pipeline.rs to keep the orchestrator module focused.

use crate::domain::state::Status;
use crate::domain::story::Story;
use serde::Serialize;
use std::collections::HashMap;

/// Reporte final de la ejecución del orquestador.
#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub blocked: usize,
    pub draft: usize,
    pub iterations: u32,
    #[serde(skip)]
    pub elapsed: std::time::Duration,
    pub elapsed_seconds: u64,
    pub stories: Vec<StoryRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Registro individual de una historia para el reporte JSON.
#[derive(Debug, Clone, Serialize)]
pub struct StoryRecord {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    pub iterations: u32,
    pub reject_cycles: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Construye el RunReport final a partir del estado de las historias.
pub fn build(
    stories: &[Story],
    iterations: u32,
    elapsed: std::time::Duration,
    story_iterations: &HashMap<String, u32>,
    reject_cycles: &HashMap<String, u32>,
    story_errors: &HashMap<String, String>,
    stop_reason: Option<String>,
) -> anyhow::Result<RunReport> {
    let done = stories.iter().filter(|s| s.status == Status::Done).count();
    let failed = stories
        .iter()
        .filter(|s| s.status == Status::Failed)
        .count();
    let blocked = stories
        .iter()
        .filter(|s| s.status == Status::Blocked)
        .count();
    let draft = stories.iter().filter(|s| s.status == Status::Draft).count();
    let total = stories.len();

    let story_records: Vec<StoryRecord> = stories
        .iter()
        .map(|s| {
            let iter_count = story_iterations.get(&s.id).copied().unwrap_or(0);
            let rej_count = reject_cycles.get(&s.id).copied().unwrap_or(0);
            let error = story_errors.get(&s.id).cloned();
            StoryRecord {
                id: s.id.clone(),
                status: s.status.to_string(),
                epic: s.epic.clone(),
                iterations: iter_count,
                reject_cycles: rej_count,
                error,
            }
        })
        .collect();

    Ok(RunReport {
        total,
        done,
        failed,
        blocked,
        draft,
        iterations,
        elapsed,
        elapsed_seconds: elapsed.as_secs(),
        stories: story_records,
        stop_reason,
    })
}
