//! Detección de bloqueos (deadlock) en el pipeline.
//!
//! Cuando el loop normal del orquestador no encuentra historias accionables,
//! este módulo analiza el grafo de dependencias y estados para decidir
//! qué acción tomar (normalmente: invocar al PO para desatascar).

use crate::dependency_graph::DependencyGraph;
use crate::state::Status;
use crate::story::Story;
use std::collections::HashMap;

/// Resultado del análisis de deadlock.
#[derive(Debug, Clone)]
pub enum DeadlockResolution {
    /// No hay deadlock: al menos una historia es accionable por el loop normal.
    NoDeadlock,
    /// Hay historias stuck. Se debe disparar al PO para la historia indicada
    /// (la de mayor prioridad = la que desbloquea más historias).
    InvokePoFor {
        story_id: String,
        /// Cuántas historias desbloquearía si avanza.
        #[allow(dead_code)]
        unblocks: usize,
        /// Razón: por qué está stuck (Draft, dependencia circular, etc.).
        reason: String,
    },
    /// Todas las historias están en estados terminales (Done o Failed).
    PipelineComplete,
}

/// Analiza el conjunto de historias y decide la resolución.
pub fn analyze(stories: &[Story], graph: &DependencyGraph) -> DeadlockResolution {
    // 1. ¿Hay historias accionables por el loop normal?
    let actionable: Vec<&Story> = stories
        .iter()
        .filter(|s| s.status.is_actionable())
        .collect();

    if !actionable.is_empty() {
        return DeadlockResolution::NoDeadlock;
    }

    // 2. ¿Está todo en terminal?
    let non_terminal: Vec<&Story> = stories.iter().filter(|s| !s.status.is_terminal()).collect();

    if non_terminal.is_empty() {
        return DeadlockResolution::PipelineComplete;
    }

    // 3. Construir mapa status para consultas rápidas
    let status_map: HashMap<&str, Status> =
        stories.iter().map(|s| (s.id.as_str(), s.status)).collect();

    // 4. Encontrar candidatas stuck y puntuarlas
    struct Candidate {
        id: String,
        unblocks: usize,
        reason: String,
    }

    let mut candidates: Vec<Candidate> = vec![];

    for story in non_terminal {
        match story.status {
            // Caso A: Draft → necesita PO planning
            Status::Draft => {
                let unblocks = graph.blocks_count(&story.id);
                candidates.push(Candidate {
                    id: story.id.clone(),
                    unblocks,
                    reason: format!(
                        "en Draft — necesita refinamiento del PO (desbloquearía {unblocks} historias)"
                    ),
                });
            }

            // Caso B/C: Blocked → evaluar por qué
            Status::Blocked => {
                // B.1: ¿Algún bloqueador está en Draft?
                let draft_blockers: Vec<&str> = story
                    .blockers
                    .iter()
                    .filter(|b| {
                        status_map
                            .get(b.as_str())
                            .is_some_and(|s| *s == Status::Draft)
                    })
                    .map(|s| s.as_str())
                    .collect();

                if !draft_blockers.is_empty() {
                    // El bloqueador Draft es el candidato real, no esta historia
                    for draft_blocker in &draft_blockers {
                        let unblocks = graph.blocks_count(draft_blocker);
                        candidates.push(Candidate {
                            id: draft_blocker.to_string(),
                            unblocks,
                            reason: format!(
                                "en Draft, bloquea a {} — debe ser refinado por PO",
                                story.id
                            ),
                        });
                    }
                    continue;
                }

                // B.2: ¿Ciclo de dependencias?
                if graph.has_cycle_from(&story.id) {
                    let unblocks = graph.blocks_count(&story.id);
                    candidates.push(Candidate {
                        id: story.id.clone(),
                        unblocks,
                        reason: "en ciclo de dependencias — PO debe romper el ciclo".to_string(),
                    });
                    continue;
                }

                // B.3: Bloqueado pero los bloqueadores están en progreso normal
                // No está stuck realmente, solo esperando.
            }

            // Estados normales no accionables temporalmente (esperando reintento)
            _ => {}
        }
    }

    // 5. Seleccionar el mejor candidato
    if candidates.is_empty() {
        // No hay candidatos claros pero tampoco accionables.
        // Puede pasar si todo está Blocked esperando cosas en InProgress/InReview/etc.
        // En ese caso, no hay deadlock real, solo espera.
        return DeadlockResolution::NoDeadlock;
    }

    // Ordenar por: mayor unblocks, luego menor ID numérico
    candidates.sort_by(|a, b| {
        b.unblocks
            .cmp(&a.unblocks)
            .then_with(|| extract_numeric(&a.id).cmp(&extract_numeric(&b.id)))
    });

    let best = &candidates[0];
    DeadlockResolution::InvokePoFor {
        story_id: best.id.clone(),
        unblocks: best.unblocks,
        reason: best.reason.clone(),
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
    use crate::state::Status;
    use std::path::PathBuf;

    fn make_story(id: &str, status: Status, blockers: &[&str]) -> Story {
        Story {
            id: id.to_string(),
            path: PathBuf::from(format!("stories/{id}.md")),
            status,
            epic: None,
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            last_rejection: None,
            raw_content: String::new(),
        }
    }

    fn make_graph(stories: &[Story]) -> DependencyGraph {
        DependencyGraph::from_stories(stories)
    }

    #[test]
    fn all_draft_triggers_po() {
        let stories = vec![
            make_story("STORY-001", Status::Draft, &[]),
            make_story("STORY-002", Status::Draft, &[]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::InvokePoFor { story_id, .. } => {
                assert!(story_id == "STORY-001" || story_id == "STORY-002");
            }
            _ => panic!("Expected InvokePoFor, got {result:?}"),
        }
    }

    #[test]
    fn blocked_by_draft_triggers_po_for_draft() {
        let stories = vec![
            make_story("STORY-001", Status::Draft, &[]),
            make_story("STORY-002", Status::Blocked, &["STORY-001"]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::InvokePoFor { story_id, .. } => {
                assert_eq!(story_id, "STORY-001", "Debería planificar el Draft que bloquea");
            }
            _ => panic!("Expected InvokePoFor, got {result:?}"),
        }
    }

    #[test]
    fn actionable_story_means_no_deadlock() {
        let stories = vec![
            make_story("STORY-001", Status::Ready, &[]),
            make_story("STORY-002", Status::Draft, &[]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::NoDeadlock => {} // OK
            _ => panic!("Expected NoDeadlock, got {result:?}"),
        }
    }

    #[test]
    fn all_done_means_pipeline_complete() {
        let stories = vec![
            make_story("STORY-001", Status::Done, &[]),
            make_story("STORY-002", Status::Done, &[]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::PipelineComplete => {}
            _ => panic!("Expected PipelineComplete, got {result:?}"),
        }
    }

    #[test]
    fn mixed_done_and_failed_means_pipeline_complete() {
        let stories = vec![
            make_story("STORY-001", Status::Done, &[]),
            make_story("STORY-002", Status::Failed, &[]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::PipelineComplete => {}
            _ => panic!("Expected PipelineComplete, got {result:?}"),
        }
    }

    #[test]
    fn cycle_detection_triggers_po() {
        let stories = vec![
            make_story("STORY-001", Status::Blocked, &["STORY-002"]),
            make_story("STORY-002", Status::Blocked, &["STORY-001"]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::InvokePoFor {
                story_id, reason, ..
            } => {
                assert!(reason.contains("ciclo"));
                assert!(story_id == "STORY-001" || story_id == "STORY-002");
            }
            _ => panic!("Expected InvokePoFor, got {result:?}"),
        }
    }

    #[test]
    fn blocked_by_inprogress_is_not_deadlock() {
        let stories = vec![
            make_story("STORY-001", Status::InProgress, &[]),
            make_story("STORY-002", Status::Blocked, &["STORY-001"]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::NoDeadlock => {} // InProgress es accionable
            _ => panic!("Expected NoDeadlock, got {result:?}"),
        }
    }

    #[test]
    fn priority_goes_to_highest_unblocks() {
        // STORY-003 bloquea a 2 historias, STORY-001 bloquea a 1
        let stories = vec![
            make_story("STORY-001", Status::Draft, &[]),
            make_story("STORY-003", Status::Draft, &[]),
            make_story("STORY-002", Status::Blocked, &["STORY-001", "STORY-003"]),
            make_story("STORY-004", Status::Blocked, &["STORY-003"]),
        ];
        let graph = make_graph(&stories);
        let result = analyze(&stories, &graph);

        match result {
            DeadlockResolution::InvokePoFor {
                story_id, unblocks, ..
            } => {
                // STORY-003 bloquea 2 historias, STORY-001 solo 1
                assert_eq!(
                    story_id, "STORY-003",
                    "Debe priorizar la que más desbloquea"
                );
                assert_eq!(unblocks, 2);
            }
            _ => panic!("Expected InvokePoFor, got {result:?}"),
        }
    }
}
