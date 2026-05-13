//! Trait `Workflow` e implementación canónica.
//!
//! Encapsula las decisiones de la máquina de estados:
//! - `next_status`: el estado esperado tras la intervención del agente
//! - `map_status_to_role`: el rol canónico que procesa un estado
//! - `canonical_column_order`: orden visual de columnas en el board
//!
//! La implementación `CanonicalWorkflow` replica el comportamiento actual
//! hardcodeado en `pipeline.rs` y `board.rs`.

use crate::domain::state::Status;

/// Trait que define el comportamiento del workflow.
///
/// Cada método recibe `&self` (no `&mut self`) porque los workflows
/// son inmutables durante la ejecución.
/// `Sync` is required so that `&dyn Workflow` can be held across `.await`
/// points in async functions (the resulting future must be `Send` for
/// multi-threaded tokio runtimes and potential `tokio::spawn` usage in #01).
#[allow(dead_code)]
pub trait Workflow: Sync {
    /// Infiere el estado esperado tras la intervención del agente
    /// para el estado `current`.
    fn next_status(&self, current: Status) -> Status;

    /// Mapea un estado al rol canónico que lo procesa.
    /// Retorna nombres como `"product_owner"`, `"qa_engineer"`, etc.
    fn map_status_to_role(&self, status: Status) -> &'static str;

    /// Orden canónico de columnas para visualización (board / dashboard).
    fn canonical_column_order(&self) -> &[&'static str];
}

/// Implementación del workflow canónico con las 14 transiciones fijas.
///
/// Replica exactamente el comportamiento hardcodeado en `pipeline.rs`:
/// - `next_status()` ≡ `pipeline::next_status()`
/// - `map_status_to_role()` ≡ `pipeline::map_status_to_role()`
/// - `canonical_column_order()` ≡ orden usado en `board.rs`
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalWorkflow;

impl Workflow for CanonicalWorkflow {
    fn next_status(&self, current: Status) -> Status {
        match current {
            Status::Draft => Status::Ready,
            Status::Ready => Status::TestsReady,
            Status::TestsReady => Status::InReview,
            Status::InProgress => Status::InReview,
            Status::InReview => Status::BusinessReview,
            Status::BusinessReview => Status::Done,
            Status::Blocked => Status::Ready,
            _ => current,
        }
    }

    fn map_status_to_role(&self, status: Status) -> &'static str {
        match status {
            Status::Draft | Status::BusinessReview => "product_owner",
            Status::Ready => "qa_engineer",
            Status::TestsReady | Status::InProgress => "developer",
            Status::InReview => "reviewer",
            _ => "product_owner",
        }
    }

    fn canonical_column_order(&self) -> &[&'static str] {
        &[
            "Draft",
            "Ready",
            "Tests Ready",
            "In Progress",
            "In Review",
            "Business Review",
            "Done",
            "Blocked",
            "Failed",
        ]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // CA1: El trait Workflow existe con los 3 métodos requeridos
    // ====================================================================
    // Estos tests verifican implícitamente CA1 al compilar:
    // si el trait no existiera o le faltara un método, no compilarían.

    /// Verifica que se puede usar `CanonicalWorkflow` como `&dyn Workflow`.
    /// Esto prueba que el trait existe (CA1) y que el struct lo implementa (CA2).
    ///
    /// NOTA TDD: este test falla ahora (todo!() panic) y pasará cuando
    /// el Developer implemente `next_status`, `map_status_to_role` y
    /// `canonical_column_order` correctamente.
    #[test]
    fn canonical_workflow_can_be_used_as_trait_object() {
        let wf: &dyn Workflow = &CanonicalWorkflow;
        // CA1: el trait existe y expone next_status
        assert_eq!(wf.next_status(Status::Draft), Status::Ready);
        // CA1: el trait existe y expone map_status_to_role
        assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
        // CA1: el trait existe y expone canonical_column_order (no vacío)
        assert!(!wf.canonical_column_order().is_empty());
    }

    // ====================================================================
    // CA2: CanonicalWorkflow implementa Workflow
    // ====================================================================
    // Se verifica en los tests concretos de next_status / map_status_to_role.

    /// CA2: CanonicalWorkflow se puede construir sin argumentos.
    #[test]
    fn canonical_workflow_can_be_constructed() {
        let _wf = CanonicalWorkflow;
        let _wf2 = CanonicalWorkflow::default();
    }

    // ====================================================================
    // CA3: CanonicalWorkflow::next_status() ≡ pipeline::next_status()
    // ====================================================================
    // Se comparan contra los outputs esperados de la función actual.
    // (El Developer debe hacer que estos tests pasen.)

    mod next_status {
        use super::*;

        /// Happy path: Draft → Ready → TestsReady → InReview → BusinessReview → Done
        #[test]
        fn happy_path() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.next_status(Status::Ready), Status::TestsReady);
            assert_eq!(wf.next_status(Status::TestsReady), Status::InReview);
            assert_eq!(wf.next_status(Status::InReview), Status::BusinessReview);
            assert_eq!(wf.next_status(Status::BusinessReview), Status::Done);
        }

        /// Fix path: InProgress → InReview
        #[test]
        fn fix_path_in_progress_to_in_review() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::InProgress), Status::InReview);
        }

        /// Estados terminales: Done y Failed se quedan como están
        #[test]
        fn terminal_states_stay() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::Done), Status::Done);
            assert_eq!(wf.next_status(Status::Failed), Status::Failed);
        }

        /// Blocked desbloquea a Ready (el orquestador usa el workflow para determinar el target)
        #[test]
        fn blocked_unblocks_to_ready() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::Blocked), Status::Ready);
        }

        /// Verifica todos los estados posibles contra la salida esperada
        #[test]
        fn all_states_have_expected_output() {
            let wf = CanonicalWorkflow;
            // Mapa completo de todos los estados → expected next_status
            let expected: &[(Status, Status)] = &[
                (Status::Draft, Status::Ready),
                (Status::Ready, Status::TestsReady),
                (Status::TestsReady, Status::InReview),
                (Status::InProgress, Status::InReview),
                (Status::InReview, Status::BusinessReview),
                (Status::BusinessReview, Status::Done),
                (Status::Done, Status::Done),
                (Status::Blocked, Status::Ready),
                (Status::Failed, Status::Failed),
            ];

            for (current, expected_next) in expected {
                assert_eq!(
                    wf.next_status(*current),
                    *expected_next,
                    "next_status({current}) debería ser {expected_next}"
                );
            }
        }
    }

    // ====================================================================
    // CA4: CanonicalWorkflow::map_status_to_role() ≡ pipeline::map_status_to_role()
    // ====================================================================

    mod map_status_to_role {
        use super::*;

        #[test]
        fn product_owner_states() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::Draft), "product_owner");
            assert_eq!(
                wf.map_status_to_role(Status::BusinessReview),
                "product_owner"
            );
        }

        #[test]
        fn qa_engineer_state() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
        }

        #[test]
        fn developer_states() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::TestsReady), "developer");
            assert_eq!(wf.map_status_to_role(Status::InProgress), "developer");
        }

        #[test]
        fn reviewer_state() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::InReview), "reviewer");
        }

        /// Estados terminales y otros: fallback seguro a "product_owner"
        #[test]
        fn fallback_to_product_owner() {
            let wf = CanonicalWorkflow;
            // Done, Blocked, Failed → "product_owner" (fallback seguro)
            assert_eq!(wf.map_status_to_role(Status::Done), "product_owner");
            assert_eq!(wf.map_status_to_role(Status::Blocked), "product_owner");
            assert_eq!(wf.map_status_to_role(Status::Failed), "product_owner");
        }

        /// Verifica todos los estados contra la salida esperada
        #[test]
        fn all_states_have_expected_role() {
            let wf = CanonicalWorkflow;
            let expected: &[(Status, &str)] = &[
                (Status::Draft, "product_owner"),
                (Status::Ready, "qa_engineer"),
                (Status::TestsReady, "developer"),
                (Status::InProgress, "developer"),
                (Status::InReview, "reviewer"),
                (Status::BusinessReview, "product_owner"),
                (Status::Done, "product_owner"),
                (Status::Blocked, "product_owner"),
                (Status::Failed, "product_owner"),
            ];

            for (status, expected_role) in expected {
                assert_eq!(
                    wf.map_status_to_role(*status),
                    *expected_role,
                    "map_status_to_role({status}) debería ser {expected_role}"
                );
            }
        }
    }

    // ====================================================================
    // CA5: canonical_column_order() devuelve las 9 columnas en orden
    // ====================================================================

    mod canonical_column_order {
        use super::*;

        #[test]
        fn returns_nine_columns_in_correct_order() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            assert_eq!(
                order,
                &[
                    "Draft",
                    "Ready",
                    "Tests Ready",
                    "In Progress",
                    "In Review",
                    "Business Review",
                    "Done",
                    "Blocked",
                    "Failed",
                ]
            );
        }

        #[test]
        fn has_exactly_nine_columns() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.canonical_column_order().len(), 9);
        }

        #[test]
        fn columns_are_in_priority_order() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            // Done está antes que Blocked/Failed (estados terminales exitosos primero)
            let done_idx = order.iter().position(|&c| c == "Done").unwrap();
            let blocked_idx = order.iter().position(|&c| c == "Blocked").unwrap();
            let failed_idx = order.iter().position(|&c| c == "Failed").unwrap();
            assert!(
                done_idx < blocked_idx,
                "Done debería aparecer antes que Blocked en el orden canónico"
            );
            assert!(
                done_idx < failed_idx,
                "Done debería aparecer antes que Failed en el orden canónico"
            );
        }

        #[test]
        fn draft_is_first_column() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            assert_eq!(order[0], "Draft");
        }

        #[test]
        fn failed_is_last_column() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            assert_eq!(order[order.len() - 1], "Failed");
        }
    }

    // ====================================================================
    // CA7: El trait usa &self (no &mut self)
    // ====================================================================
    // Este test es compilación: si algún método pidiera &mut self,
    // no podríamos llamarlo sobre una referencia compartida.

    /// CA7: Se puede llamar sobre referencia compartida (&CanonicalWorkflow).
    /// Si el trait usara `&mut self`, este test no compilaría.
    #[test]
    fn workflow_methods_accept_immutable_reference() {
        let wf = CanonicalWorkflow;
        let shared: &CanonicalWorkflow = &wf;

        // Llamar a los tres métodos vía referencia compartida.
        // Si compila, CA7 está satisfecho.
        let _ns = Workflow::next_status(shared, Status::Draft);
        let _role = Workflow::map_status_to_role(shared, Status::Ready);
        let _cols = Workflow::canonical_column_order(shared);
    }

    // ====================================================================
    // CA6: cargo test --lib domain pasa
    // ====================================================================
    // CA6 se verifica ejecutando los tests; no es un test en sí mismo.
    // El Developer comprobará: cargo test --lib state
    // y cargo test --lib workflow

    // ====================================================================
    // Test adicional: el workflow es determinista
    // ====================================================================

    #[test]
    fn workflow_is_deterministic() {
        let wf = CanonicalWorkflow;
        // Múltiples invocaciones devuelven lo mismo
        for _ in 0..5 {
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
            assert_eq!(wf.canonical_column_order().len(), 9);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-007: ConfigurableWorkflow desde TOML
// ═══════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

/// Configuración de estados del workflow.
///
/// Vive en `config/workflow.rs` en producción. Definido aquí temporalmente
/// para que el dominio pueda consumirlo sin depender de la capa config.
#[derive(Debug, Clone)]
pub struct WorkflowStatesConfig {
    pub initial: String,
    pub terminal: Vec<String>,
}

impl Default for WorkflowStatesConfig {
    fn default() -> Self {
        Self {
            initial: "draft".to_string(),
            terminal: vec!["done".to_string(), "failed".to_string()],
        }
    }
}

/// Configuración de un rol en el workflow.
#[derive(Debug, Clone)]
pub struct RoleConfig {
    pub name: String,
    pub system_prompt: String,
    pub model: String,
}

/// Configuración de una fase (transición) del workflow.
#[derive(Debug, Clone)]
pub struct PhaseConfig {
    pub name: String,
    pub from: String,
    pub to: String,
    pub role: String,
    pub model: String,
    pub prompt: String,
    pub on_reject: String,
    pub max_reject_cycles: u32,
    pub timeout_seconds: Option<u64>,
}

/// Configuración completa del workflow cargable desde TOML.
#[derive(Debug, Clone, Default)]
pub struct WorkflowConfig {
    pub states: WorkflowStatesConfig,
    pub roles: Vec<RoleConfig>,
    pub phases: Vec<PhaseConfig>,
    pub task_format: crate::domain::task::TaskFormatConfig,
}

/// Workflow configurable en runtime.
///
/// Recibe un `&WorkflowConfig` (que vive en la capa `config/`) y expone
/// consultas sobre estados, fases, roles y transiciones automáticas.
#[derive(Debug, Clone)]
pub struct ConfigurableWorkflow {
    states: WorkflowStatesConfig,
    phases: Vec<PhaseConfig>,
}

impl ConfigurableWorkflow {
    /// Construye un workflow configurable a partir de la configuración.
    pub fn new(config: &WorkflowConfig) -> Self {
        Self {
            states: config.states.clone(),
            phases: config.phases.clone(),
        }
    }

    /// Devuelve todas las fases cuyo `from` coincide con el estado actual.
    /// Si hay más de una, hay bifurcación y el agente elige.
    pub fn phases_for_status(&self, status: &str) -> Vec<&PhaseConfig> {
        self.phases.iter().filter(|p| p.from == status).collect()
    }

    /// ¿Es este estado terminal? (el pipeline no vuelve a tocar la task).
    pub fn is_terminal(&self, status: &str) -> bool {
        self.states.terminal.iter().any(|t| t == status)
    }

    /// Estado inicial del workflow.
    pub fn initial_state(&self) -> &str {
        &self.states.initial
    }

    /// Aplica transiciones automáticas a una task según su estado y dependencias.
    ///
    /// Retorna `Some(nuevo_estado)` si se debe hacer una transición automática,
    /// o `None` si no corresponde.
    pub fn apply_automatic_transitions(
        &self,
        task: &crate::domain::task::Task,
        graph: &crate::domain::graph::DependencyGraph,
        reject_cycles: u32,
        status_map: &HashMap<String, String>,
    ) -> Option<String> {
        let current_status = task.fields.get("status").cloned().unwrap_or_default();

        // 1. ¿max_reject_cycles agotado?
        for phase in &self.phases {
            if phase.from == current_status && reject_cycles >= phase.max_reject_cycles {
                // Encontrar el estado "failed" configurado
                let failed_state = self
                    .states
                    .terminal
                    .iter()
                    .find(|t| {
                        t.to_lowercase().contains("fail") || t.to_lowercase().contains("reject")
                    })
                    .cloned()
                    .unwrap_or_else(|| "failed".to_string());
                return Some(failed_state);
            }
        }

        // 2. Bloquear por dependencias no resueltas
        if !task.blockers.is_empty() {
            let all_blockers_done = task.blockers.iter().all(|blocker| {
                status_map
                    .get(blocker)
                    .is_some_and(|s| self.is_terminal(s))
            });

            if !all_blockers_done {
                // Solo bloquear si no es ya blocked y no es terminal
                if current_status != "blocked" && !self.is_terminal(&current_status) {
                    return Some("blocked".to_string());
                }
            } else if current_status == "blocked" {
                // Desbloquear: todos los blockers están en terminal
                return Some(self.states.initial.clone());
            }
        }

        None
    }
}

#[cfg(test)]
mod configurable_workflow_tests {
    use super::*;
    use crate::domain::graph::DependencyGraph;
    use crate::domain::task::{ActivityLogEntry, Task, TaskFormatConfig};
    use std::collections::HashMap;
    use std::path::PathBuf;

    // ── Helpers ──────────────────────────────────────────────────────

    fn make_workflow_config() -> WorkflowConfig {
        WorkflowConfig {
            states: WorkflowStatesConfig {
                initial: "draft".to_string(),
                terminal: vec!["done".to_string(), "failed".to_string()],
            },
            roles: vec![
                RoleConfig {
                    name: "developer".to_string(),
                    system_prompt: "Eres un desarrollador senior.".to_string(),
                    model: "gpt4o".to_string(),
                },
                RoleConfig {
                    name: "reviewer".to_string(),
                    system_prompt: "Eres un revisor de código.".to_string(),
                    model: "claude".to_string(),
                },
            ],
            phases: vec![
                PhaseConfig {
                    name: "implement".to_string(),
                    from: "ready".to_string(),
                    to: "review".to_string(),
                    role: "developer".to_string(),
                    model: "gpt4o".to_string(),
                    prompt: "Implementa {{task_id}}".to_string(),
                    on_reject: "ready".to_string(),
                    max_reject_cycles: 3,
                    timeout_seconds: None,
                },
                PhaseConfig {
                    name: "review".to_string(),
                    from: "review".to_string(),
                    to: "done".to_string(),
                    role: "reviewer".to_string(),
                    model: "claude".to_string(),
                    prompt: "Revisa {{task_id}}".to_string(),
                    on_reject: "ready".to_string(),
                    max_reject_cycles: 2,
                    timeout_seconds: Some(300),
                },
            ],
            task_format: TaskFormatConfig::default(),
        }
    }

    fn make_task(id: &str, status: &str, blockers: &[&str]) -> Task {
        let mut fields = HashMap::new();
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

    // ═══════════════════════════════════════════════════════════════
    // CA1: WorkflowConfig contiene todos los campos
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn workflow_config_has_states_roles_phases() {
        let config = make_workflow_config();
        assert_eq!(config.states.initial, "draft");
        assert!(config.states.terminal.contains(&"done".to_string()));
        assert_eq!(config.roles.len(), 2);
        assert_eq!(config.roles[0].name, "developer");
        assert_eq!(config.phases.len(), 2);
        assert_eq!(config.phases[0].name, "implement");
    }

    #[test]
    fn phase_config_includes_optional_timeout() {
        let config = make_workflow_config();
        assert!(config.phases[0].timeout_seconds.is_none());
        assert_eq!(config.phases[1].timeout_seconds, Some(300));
    }

    #[test]
    fn task_format_is_included_in_workflow_config() {
        let config = make_workflow_config();
        assert_eq!(config.task_format.id_pattern, r"TASK-\d+");
        assert!(config.task_format.section_markers.contains_key("status"));
    }

    // ═══════════════════════════════════════════════════════════════
    // CA2: phases_for_status
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn phases_for_status_returns_matching_phases() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        let phases = wf.phases_for_status("ready");
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].name, "implement");
        assert_eq!(phases[0].from, "ready");
        assert_eq!(phases[0].to, "review");
    }

    #[test]
    fn phases_for_status_with_bifurcation() {
        let mut config = make_workflow_config();
        // Añadir una segunda fase desde "in_review"
        config.phases.push(PhaseConfig {
            name: "approve".to_string(),
            from: "in_review".to_string(),
            to: "done".to_string(),
            role: "reviewer".to_string(),
            model: "claude".to_string(),
            prompt: "Aprueba".to_string(),
            on_reject: "in_progress".to_string(),
            max_reject_cycles: 2,
            timeout_seconds: None,
        });
        config.phases.push(PhaseConfig {
            name: "request_changes".to_string(),
            from: "in_review".to_string(),
            to: "in_progress".to_string(),
            role: "reviewer".to_string(),
            model: "claude".to_string(),
            prompt: "Solicita cambios".to_string(),
            on_reject: "in_review".to_string(),
            max_reject_cycles: 2,
            timeout_seconds: None,
        });

        let wf = ConfigurableWorkflow::new(&config);
        let phases = wf.phases_for_status("in_review");
        assert_eq!(phases.len(), 2);
        let names: Vec<&str> = phases.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"approve"));
        assert!(names.contains(&"request_changes"));
    }

    #[test]
    fn phases_for_status_returns_empty_for_terminal_state() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        let phases = wf.phases_for_status("done");
        assert!(phases.is_empty());
    }

    #[test]
    fn phases_for_status_returns_empty_for_unknown_state() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        let phases = wf.phases_for_status("unknown_state");
        assert!(phases.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════
    // CA3: is_terminal
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn is_terminal_returns_true_for_done() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        assert!(wf.is_terminal("done"));
        assert!(wf.is_terminal("failed"));
    }

    #[test]
    fn is_terminal_returns_false_for_non_terminal() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        assert!(!wf.is_terminal("draft"));
        assert!(!wf.is_terminal("ready"));
        assert!(!wf.is_terminal("in_progress"));
    }

    #[test]
    fn is_terminal_returns_false_for_unknown_state() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        assert!(!wf.is_terminal("unknown"));
    }

    #[test]
    fn is_terminal_with_custom_terminal_states() {
        let mut config = make_workflow_config();
        config.states.terminal = vec!["completed".to_string(), "cancelled".to_string()];
        let wf = ConfigurableWorkflow::new(&config);
        assert!(wf.is_terminal("completed"));
        assert!(wf.is_terminal("cancelled"));
        assert!(!wf.is_terminal("done"), "done no debería ser terminal si no está en la lista");
    }

    #[test]
    fn is_terminal_with_empty_terminal_list() {
        let mut config = make_workflow_config();
        config.states.terminal = vec![];
        let wf = ConfigurableWorkflow::new(&config);
        assert!(!wf.is_terminal("done"));
        assert!(!wf.is_terminal("failed"));
    }

    // ═══════════════════════════════════════════════════════════════
    // Transiciones automáticas
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn automatic_transition_to_blocked_for_unresolved_dependencies() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task("TASK-004", "ready", &["TASK-003"]);
        let graph = DependencyGraph::default(); // simplificado
        let mut status_map = HashMap::new();
        status_map.insert("TASK-003".to_string(), "draft".to_string());

        let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
        assert_eq!(result, Some("blocked".to_string()));
    }

    #[test]
    fn automatic_transition_to_unblocked_when_all_blockers_terminal() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task("TASK-004", "blocked", &["TASK-003"]);
        let graph = DependencyGraph::default();
        let mut status_map = HashMap::new();
        status_map.insert("TASK-003".to_string(), "done".to_string());

        let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
        assert_eq!(result, Some("draft".to_string())); // initial state
    }

    #[test]
    fn automatic_transition_stays_blocked_if_not_all_blockers_terminal() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task("TASK-004", "blocked", &["TASK-002", "TASK-003"]);
        let graph = DependencyGraph::default();
        let mut status_map = HashMap::new();
        status_map.insert("TASK-002".to_string(), "done".to_string());
        status_map.insert("TASK-003".to_string(), "draft".to_string());

        let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
        assert_eq!(result, None);
    }

    #[test]
    fn automatic_transition_to_failed_on_max_reject_cycles() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task("TASK-001", "review", &[]);
        let graph = DependencyGraph::default();
        let status_map = HashMap::new();

        // max_reject_cycles para la fase "review" es 2
        let result = wf.apply_automatic_transitions(&task, &graph, 2, &status_map);
        assert_eq!(result, Some("failed".to_string()));
    }

    #[test]
    fn no_automatic_transition_below_max_reject_cycles() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task("TASK-001", "review", &[]);
        let graph = DependencyGraph::default();
        let status_map = HashMap::new();

        // max_reject_cycles para "review" es 2, llevamos 1
        let result = wf.apply_automatic_transitions(&task, &graph, 1, &status_map);
        assert_eq!(result, None);
    }

    #[test]
    fn automatic_transition_no_change_for_terminal_task() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task("TASK-001", "done", &["TASK-002"]);
        let graph = DependencyGraph::default();
        let mut status_map = HashMap::new();
        status_map.insert("TASK-002".to_string(), "draft".to_string());

        // Ya es terminal, no debería cambiar
        let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
        assert_eq!(result, None);
    }

    // ═══════════════════════════════════════════════════════════════
    // initial_state
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn initial_state_returns_configured_value() {
        let wf = ConfigurableWorkflow::new(&make_workflow_config());
        assert_eq!(wf.initial_state(), "draft");
    }

    // ═══════════════════════════════════════════════════════════════
    // ConfigurableWorkflow recibe &WorkflowConfig (referencia)
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn configurable_workflow_does_not_take_ownership() {
        let config = make_workflow_config();
        let wf = ConfigurableWorkflow::new(&config);
        // La config original sigue siendo accesible
        assert_eq!(config.states.initial, "draft");
        assert!(wf.is_terminal("done"));
    }
}
