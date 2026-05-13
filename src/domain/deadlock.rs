//! Detección de bloqueos (deadlock) en el pipeline.
//!
//! Cuando el loop normal del orquestador no encuentra tareas accionables,
//! este módulo analiza el grafo de dependencias y estados para decidir
//! qué acción tomar (normalmente: invocar al agente para desatascar).
//!
//! Soporta tanto `Story` (v0.x) como `Task` (v1.0) mediante funciones separadas.

use crate::domain::graph::DependencyGraph;
use crate::domain::state::Status;
use crate::domain::story::Story;
use crate::domain::task::Task;
use crate::domain::workflow::ConfigurableWorkflow;
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

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-009: analyze_deadlock() genérico para Task
// ═══════════════════════════════════════════════════════════════════════

/// Resultado del análisis de deadlock genérico (v1.0).
#[derive(Debug, Clone)]
pub enum DeadlockResolutionV10 {
    /// No hay deadlock: al menos una tarea es accionable.
    NoDeadlock,
    /// Hay tareas stuck. Se debe disparar al agente correspondiente.
    InvokeAgentFor {
        task_id: String,
        unblocks: usize,
        reason: String,
    },
    /// Todas las tareas están en estados terminales.
    PipelineComplete,
}

/// Analiza el conjunto de tareas genéricas y decide la resolución.
///
/// A diferencia de `analyze()` que usa `Story` y `Status` enums,
/// esta función trabaja con `Task` y estados como `String`.
pub fn analyze_deadlock(
    tasks: &[Task],
    graph: &DependencyGraph,
    workflow: &ConfigurableWorkflow,
) -> DeadlockResolutionV10 {
    // 1. ¿Hay tareas accionables? (no Draft, no Blocked, no terminal)
    let actionable: Vec<&Task> = tasks
        .iter()
        .filter(|t| {
            let status = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
            !workflow.is_terminal(status)
                && status != "draft"
                && status != "blocked"
                && !status.is_empty()
        })
        .collect();

    if !actionable.is_empty() {
        return DeadlockResolutionV10::NoDeadlock;
    }

    // 2. ¿Está todo en terminal?
    let non_terminal: Vec<&Task> = tasks
        .iter()
        .filter(|t| {
            let status = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
            !workflow.is_terminal(status)
        })
        .collect();

    if non_terminal.is_empty() {
        return DeadlockResolutionV10::PipelineComplete;
    }

    // 3. Construir mapa status para consultas rápidas
    let status_map: HashMap<&str, &str> = tasks
        .iter()
        .map(|t| {
            (
                t.id.as_str(),
                t.fields.get("status").map(|s| s.as_str()).unwrap_or(""),
            )
        })
        .collect();

    // 4. Encontrar candidatas stuck y puntuarlas
    struct Candidate {
        id: String,
        unblocks: usize,
        reason: String,
    }

    let mut candidates: Vec<Candidate> = vec![];

    for task in non_terminal {
        let status = task.fields.get("status").map(|s| s.as_str()).unwrap_or("");

        match status {
            // Caso A: Draft → necesita refinamiento
            "draft" => {
                let unblocks = graph.blocks_count(&task.id);
                candidates.push(Candidate {
                    id: task.id.clone(),
                    unblocks,
                    reason: format!(
                        "{} en Draft — necesita refinamiento (desbloquearía {unblocks} tareas)",
                        task.id
                    ),
                });
            }

            // Caso B: Blocked → evaluar por qué
            "blocked" => {
                // B.1: ¿Algún bloqueador está en Draft?
                let draft_blockers: Vec<&str> = task
                    .blockers
                    .iter()
                    .filter(|b| status_map.get(b.as_str()).is_some_and(|s| *s == "draft"))
                    .map(|s| s.as_str())
                    .collect();

                if !draft_blockers.is_empty() {
                    for draft_blocker in &draft_blockers {
                        let unblocks = graph.blocks_count(draft_blocker);
                        candidates.push(Candidate {
                            id: draft_blocker.to_string(),
                            unblocks,
                            reason: format!(
                                "{} en Draft, bloquea a {} — debe ser refinado",
                                draft_blocker, task.id
                            ),
                        });
                    }
                    continue;
                }

                // B.2: ¿Ciclo de dependencias?
                if graph.has_cycle_from(&task.id) {
                    let unblocks = graph.blocks_count(&task.id);
                    candidates.push(Candidate {
                        id: task.id.clone(),
                        unblocks,
                        reason: format!(
                            "{} en ciclo de dependencias — el agente debe romper el ciclo",
                            task.id
                        ),
                    });
                    continue;
                }
            }

            _ => {}
        }
    }

    // 5. Seleccionar el mejor candidato
    if candidates.is_empty() {
        return DeadlockResolutionV10::NoDeadlock;
    }

    // Ordenar por: mayor unblocks, luego menor ID numérico
    candidates.sort_by(|a, b| {
        b.unblocks
            .cmp(&a.unblocks)
            .then_with(|| extract_numeric(&a.id).cmp(&extract_numeric(&b.id)))
    });

    let best = &candidates[0];
    DeadlockResolutionV10::InvokeAgentFor {
        task_id: best.id.clone(),
        unblocks: best.unblocks,
        reason: best.reason.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::state::Status;
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
                assert_eq!(
                    story_id, "STORY-001",
                    "Debería planificar el Draft que bloquea"
                );
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-V10-009: analyze_deadlock() con Task genérico
    // ═══════════════════════════════════════════════════════════════

    use crate::domain::task::Task;
    use crate::domain::workflow::{
        ConfigurableWorkflow, PhaseConfig, RoleConfig, WorkflowConfig, WorkflowStatesConfig,
    };

    fn make_config() -> WorkflowConfig {
        WorkflowConfig {
            states: WorkflowStatesConfig {
                initial: "draft".to_string(),
                terminal: vec!["done".to_string(), "failed".to_string()],
            },
            roles: vec![RoleConfig {
                name: "agent".to_string(),
                system_prompt: "Eres un agente.".to_string(),
                model: "gpt4o".to_string(),
            }],
            phases: vec![PhaseConfig {
                name: "execute".to_string(),
                from: "ready".to_string(),
                to: "done".to_string(),
                role: "agent".to_string(),
                model: "gpt4o".to_string(),
                prompt: "Ejecuta {{task_id}}".to_string(),
                on_reject: "draft".to_string(),
                max_reject_cycles: 3,
                timeout_seconds: None,
            }],
            task_format: crate::domain::task::TaskFormatConfig::default(),
        }
    }

    fn make_task_v10(id: &str, status: &str, blockers: &[&str]) -> Task {
        let mut fields = std::collections::HashMap::new();
        fields.insert("status".to_string(), status.to_string());
        Task {
            id: id.to_string(),
            path: PathBuf::from(format!("tasks/{id}.md")),
            fields,
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            activity_log: vec![],
            raw_content: String::new(),
        }
    }

    #[test]
    fn v10_all_draft_triggers_agent() {
        let tasks = vec![
            make_task_v10("TASK-001", "draft", &[]),
            make_task_v10("TASK-002", "draft", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor { task_id, .. } => {
                assert!(task_id == "TASK-001" || task_id == "TASK-002");
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_blocked_by_draft_triggers_agent_for_draft() {
        let tasks = vec![
            make_task_v10("TASK-001", "draft", &[]),
            make_task_v10("TASK-002", "blocked", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor { task_id, .. } => {
                assert_eq!(task_id, "TASK-001", "Debería planificar el Draft que bloquea");
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_actionable_task_means_no_deadlock() {
        let tasks = vec![
            make_task_v10("TASK-001", "ready", &[]),
            make_task_v10("TASK-002", "draft", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::NoDeadlock => {}
            _ => panic!("Expected NoDeadlock, got {result:?}"),
        }
    }

    #[test]
    fn v10_all_done_means_pipeline_complete() {
        let tasks = vec![
            make_task_v10("TASK-001", "done", &[]),
            make_task_v10("TASK-002", "done", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::PipelineComplete => {}
            _ => panic!("Expected PipelineComplete, got {result:?}"),
        }
    }

    #[test]
    fn v10_mixed_done_and_failed_means_pipeline_complete() {
        let tasks = vec![
            make_task_v10("TASK-001", "done", &[]),
            make_task_v10("TASK-002", "failed", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::PipelineComplete => {}
            _ => panic!("Expected PipelineComplete, got {result:?}"),
        }
    }

    #[test]
    fn v10_cycle_detection_triggers_agent() {
        let tasks = vec![
            make_task_v10("TASK-006", "blocked", &["TASK-007"]),
            make_task_v10("TASK-007", "blocked", &["TASK-006"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor { task_id, reason, .. } => {
                assert!(reason.contains("ciclo"));
                assert!(task_id == "TASK-006" || task_id == "TASK-007");
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_blocked_by_active_task_is_not_deadlock() {
        let tasks = vec![
            make_task_v10("TASK-001", "in_progress", &[]),
            make_task_v10("TASK-002", "blocked", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::NoDeadlock => {}
            _ => panic!("Expected NoDeadlock, got {result:?}"),
        }
    }

    #[test]
    fn v10_priority_goes_to_highest_unblocks() {
        // TASK-003 bloquea a 2 tareas, TASK-001 bloquea a 1
        let tasks = vec![
            make_task_v10("TASK-001", "draft", &[]),
            make_task_v10("TASK-003", "draft", &[]),
            make_task_v10("TASK-002", "blocked", &["TASK-001", "TASK-003"]),
            make_task_v10("TASK-004", "blocked", &["TASK-003"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor {
                task_id, unblocks, ..
            } => {
                assert_eq!(task_id, "TASK-003", "Debe priorizar la que más desbloquea");
                assert_eq!(unblocks, 2);
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_tiebreaker_by_numeric_id() {
        let tasks = vec![
            make_task_v10("TASK-020", "draft", &[]),
            make_task_v10("TASK-010", "draft", &[]),
            make_task_v10("TASK-001", "blocked", &["TASK-010", "TASK-020"]),
            make_task_v10("TASK-002", "blocked", &["TASK-010", "TASK-020"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor { task_id, .. } => {
                assert_eq!(
                    task_id, "TASK-010",
                    "En empate de unblocks, debe elegir el menor ID numérico"
                );
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_reason_messages_do_not_hardcode_story() {
        let tasks = vec![
            make_task_v10("ISSUE-042", "draft", &[]),
            make_task_v10("ISSUE-043", "blocked", &["ISSUE-042"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor { reason, .. } => {
                assert!(
                    reason.contains("ISSUE-042"),
                    "El mensaje debe contener el ID real: {reason}"
                );
                assert!(
                    !reason.contains("STORY"),
                    "El mensaje NO debe hardcodear 'STORY': {reason}"
                );
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_analyze_deadlock_with_issue_ids() {
        let tasks = vec![
            make_task_v10("ISSUE-001", "draft", &[]),
            make_task_v10("ISSUE-002", "draft", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::InvokeAgentFor { task_id, .. } => {
                assert!(
                    task_id.starts_with("ISSUE-"),
                    "Debe preservar el formato ISSUE-NNN"
                );
            }
            _ => panic!("Expected InvokeAgentFor, got {result:?}"),
        }
    }

    #[test]
    fn v10_no_deadlock_when_all_blocked_waiting_for_active() {
        let tasks = vec![
            make_task_v10("TASK-001", "review", &[]),
            make_task_v10("TASK-002", "blocked", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::NoDeadlock => {}
            _ => panic!("Expected NoDeadlock, got {result:?}"),
        }
    }

    #[test]
    fn v10_no_candidates_returns_no_deadlock() {
        // Todo está Blocked esperando cosas en Done (que son terminales)
        // pero como Done es terminal, no hay nadie a quien invocar
        let tasks = vec![
            make_task_v10("TASK-001", "done", &[]),
            make_task_v10("TASK-002", "blocked", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let wf = ConfigurableWorkflow::new(&make_config());
        let result = analyze_deadlock(&tasks, &graph, &wf);

        match result {
            DeadlockResolutionV10::NoDeadlock => {}
            _ => panic!(
                "Expected NoDeadlock (blocked esperando done que es terminal, no hay deadlock): got {result:?}"
            ),
        }
    }
}
