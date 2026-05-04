//! Dashboard de historias (`regista board`).
//!
//! Muestra un tablero Kanban con el conteo de historias por estado,
//! y lista las que están bloqueadas o fallidas con detalle.
//!
//! Diseñado para ser resistente a #04 (workflows configurables):
//! trabaja con claves string (`status.to_string()`) en vez de acoplarse
//! a las variantes del enum `Status`. Cuando los estados pasen a ser
//! dinámicos, este módulo apenas necesitará cambios.

use crate::app::pipeline;
use crate::config::Config;
use crate::domain::story::Story;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Datos agregados del tablero de historias.
#[derive(Debug, Clone, Serialize)]
pub struct BoardData {
    /// Conteo de historias por estado (clave = representación string del estado).
    pub counts: HashMap<String, usize>,
    /// Total de historias cargadas.
    pub total: usize,
    /// Historias bloqueadas, con sus dependencias.
    pub blocked: Vec<BlockedStory>,
    /// Historias fallidas, con el motivo del último rechazo.
    pub failed: Vec<FailedStory>,
}

/// Una historia bloqueada y qué la bloquea.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedStory {
    pub id: String,
    /// IDs de las historias que bloquean a esta.
    pub blocked_by: Vec<String>,
}

/// Una historia fallida y el motivo.
#[derive(Debug, Clone, Serialize)]
pub struct FailedStory {
    pub id: String,
    /// Motivo del último rechazo (del Activity Log), si está disponible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Ejecuta el comando `board`.
///
/// Carga todas las historias, construye el `BoardData` y lo imprime
/// en formato humano o JSON.
pub fn run(
    project_root: &Path,
    json: bool,
    epic_filter: Option<&str>,
    config_path: Option<&Path>,
) -> anyhow::Result<()> {
    let cfg = Config::load(project_root, config_path)?;
    let mut stories = pipeline::load_all_stories(project_root, &cfg)?;

    // Filtrar por épica si se especifica
    if let Some(epic) = epic_filter {
        stories.retain(|s| s.epic.as_ref().is_some_and(|e| e == epic));
    }

    let data = build_board_data(&stories);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".into())
        );
    } else {
        print_human(&data);
    }

    Ok(())
}

/// Construye el `BoardData` a partir de una lista de historias.
fn build_board_data(stories: &[Story]) -> BoardData {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut blocked: Vec<BlockedStory> = Vec::new();
    let mut failed: Vec<FailedStory> = Vec::new();

    for story in stories {
        let status_key = story.status.to_string();
        *counts.entry(status_key.clone()).or_default() += 1;

        // Estados especiales: usamos la clave string, no el enum.
        // Cuando #04 llegue, estos literales se leerán de la config.
        match status_key.as_str() {
            "Blocked" => {
                blocked.push(BlockedStory {
                    id: story.id.clone(),
                    blocked_by: story.blockers.clone(),
                });
            }
            "Failed" => {
                failed.push(FailedStory {
                    id: story.id.clone(),
                    reason: story.last_rejection.clone(),
                });
            }
            _ => {}
        }
    }

    // Ordenar por ID numérico para salida predecible
    blocked.sort_by_key(|s| extract_numeric(&s.id));
    failed.sort_by_key(|s| extract_numeric(&s.id));

    BoardData {
        total: stories.len(),
        counts,
        blocked,
        failed,
    }
}

/// Imprime el tablero en formato legible por humanos.
fn print_human(data: &BoardData) {
    // Cabecera
    println!("📊 Story Board — regista");
    println!("==========================");
    println!();

    // Conteo por estado (en orden canónico para consistencia visual)
    let canonical_order = [
        "Draft",
        "Ready",
        "Tests Ready",
        "In Progress",
        "In Review",
        "Business Review",
        "Done",
        "Blocked",
        "Failed",
    ];

    for state in &canonical_order {
        let count = data.counts.get(*state).copied().unwrap_or(0);
        println!("  {state:<18} {count:>3}");
    }
    println!("  {}", "─".repeat(22));
    println!("  {:<18} {:>3}", "Total", data.total);
    println!();

    // Bloqueadas
    if !data.blocked.is_empty() {
        println!("🔴 Blocked ({}):", data.blocked.len());
        for bs in &data.blocked {
            let blockers = bs.blocked_by.join(", ");
            println!("  {} — blocked by: {}", bs.id, blockers);
        }
        println!();
    }

    // Fallidas
    if !data.failed.is_empty() {
        println!("❌ Failed ({}):", data.failed.len());
        for fs in &data.failed {
            match &fs.reason {
                Some(reason) => println!("  {} — {}", fs.id, reason),
                None => println!("  {} — (sin motivo registrado)", fs.id),
            }
        }
        println!();
    }
}

/// Extrae el número de un ID tipo "STORY-NNN".
fn extract_numeric(id: &str) -> u32 {
    id.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::state::Status;

    /// Construye una Story sintética para tests.
    fn fake_story(
        id: &str,
        status: Status,
        epic: Option<&str>,
        blockers: &[&str],
        last_rejection: Option<&str>,
    ) -> Story {
        Story {
            id: id.to_string(),
            path: format!("stories/{id}.md").into(),
            status,
            epic: epic.map(|s| s.to_string()),
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            last_rejection: last_rejection.map(|s| s.to_string()),
            raw_content: String::new(),
        }
    }

    #[test]
    fn build_board_counts_correctly() {
        let stories = vec![
            fake_story("STORY-001", Status::Draft, None, &[], None),
            fake_story("STORY-002", Status::Ready, None, &[], None),
            fake_story("STORY-003", Status::Done, None, &[], None),
            fake_story("STORY-004", Status::Done, None, &[], None),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.total, 4);
        assert_eq!(data.counts.get("Draft").copied().unwrap_or(0), 1);
        assert_eq!(data.counts.get("Ready").copied().unwrap_or(0), 1);
        assert_eq!(data.counts.get("Done").copied().unwrap_or(0), 2);
        assert!(data.blocked.is_empty());
        assert!(data.failed.is_empty());
    }

    #[test]
    fn build_board_lists_blocked_stories() {
        let stories = vec![
            fake_story("STORY-001", Status::Done, None, &[], None),
            fake_story("STORY-002", Status::Blocked, None, &["STORY-001"], None),
            fake_story(
                "STORY-003",
                Status::Blocked,
                None,
                &["STORY-001", "STORY-002"],
                None,
            ),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.blocked.len(), 2);
        assert_eq!(data.blocked[0].id, "STORY-002");
        assert_eq!(data.blocked[0].blocked_by, vec!["STORY-001"]);
        assert_eq!(data.blocked[1].id, "STORY-003");
        assert_eq!(data.blocked[1].blocked_by, vec!["STORY-001", "STORY-002"]);
        assert!(data.failed.is_empty());
    }

    #[test]
    fn build_board_lists_failed_stories_with_reason() {
        let stories = vec![
            fake_story(
                "STORY-001",
                Status::Failed,
                None,
                &[],
                Some("max reject cycles (8/8)"),
            ),
            fake_story("STORY-002", Status::Failed, None, &[], None),
            fake_story("STORY-003", Status::Done, None, &[], None),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.failed.len(), 2);
        assert_eq!(data.failed[0].id, "STORY-001");
        assert_eq!(
            data.failed[0].reason.as_deref(),
            Some("max reject cycles (8/8)")
        );
        assert_eq!(data.failed[1].id, "STORY-002");
        assert_eq!(data.failed[1].reason, None);
        assert!(data.blocked.is_empty());
    }

    #[test]
    fn build_board_handles_empty_list() {
        let stories: Vec<Story> = vec![];
        let data = build_board_data(&stories);

        assert_eq!(data.total, 0);
        assert!(data.counts.is_empty());
        assert!(data.blocked.is_empty());
        assert!(data.failed.is_empty());
    }

    #[test]
    fn build_board_sorts_by_numeric_id() {
        let stories = vec![
            fake_story("STORY-010", Status::Blocked, None, &["STORY-005"], None),
            fake_story("STORY-002", Status::Blocked, None, &["STORY-001"], None),
            fake_story("STORY-005", Status::Blocked, None, &["STORY-003"], None),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.blocked.len(), 3);
        assert_eq!(data.blocked[0].id, "STORY-002");
        assert_eq!(data.blocked[1].id, "STORY-005");
        assert_eq!(data.blocked[2].id, "STORY-010");
    }

    #[test]
    fn board_data_serializes_to_json() {
        let mut counts = HashMap::new();
        counts.insert("Done".into(), 3usize);
        counts.insert("Draft".into(), 2usize);

        let data = BoardData {
            counts,
            total: 5,
            blocked: vec![BlockedStory {
                id: "STORY-002".into(),
                blocked_by: vec!["STORY-001".into()],
            }],
            failed: vec![FailedStory {
                id: "STORY-005".into(),
                reason: Some("rechazada 3 veces".into()),
            }],
        };

        let json = serde_json::to_string_pretty(&data).unwrap();
        assert!(json.contains("\"Done\": 3"));
        assert!(json.contains("\"Draft\": 2"));
        assert!(json.contains("\"total\": 5"));
        assert!(json.contains("STORY-002"));
        assert!(json.contains("STORY-005"));
        assert!(json.contains("rechazada 3 veces"));
    }
}
