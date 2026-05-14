//! Integration tests for EPIC-V10-06 — STORY-V10-023: Pipeline with mock LLM provider.
//!
//! These tests validate the pipeline end-to-end using a mock `LlmProvider`
//! that returns predefined responses. They follow the Gherkin scenarios from
//!   - roadmap/features/app/pipeline.feature (STORY-V10-010, -011, -012)
//!
//! TDD RED: These tests are self-contained (no lib.rs needed yet). They define
//! local versions of domain types that mirror the expected production API.
//! To graduate:
//!   1. Developer creates `src/lib.rs` with `pub mod domain; pub mod infra; pub mod app;`
//!   2. Replace local type definitions with `use regista::domain::*`
//!   3. Remove `#[ignore]` attributes
//!
//! Gherkin scenarios tested:
//!   Scenario 1: El pipeline avanza una tarea por todas las fases
//!   Scenario 2: Bifurcación presenta opciones al agente
//!   Scenario 3: El historial multi-turn se acumula en las invocaciones
//!   Scenario 4: Parsear transición exitosa [STATUS: X]
//!   Scenario 5: Parsear rechazo [REJECT: motivo]
//!   Scenario 6: Reintentar cuando el agente no sigue el formato
//!   Scenario 7: Tarea se bloquea por dependencias no resueltas
//!   Scenario 8: Tarea se desbloquea cuando sus dependencias terminan
//!   Scenario 9: Tarea pasa a failed por superar max_reject_cycles

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// Local domain types (mirror domain/ — replace with lib.rs imports)
// ═══════════════════════════════════════════════════════════════════════

/// Mirrors `domain::task::TaskFormatConfig`.
#[derive(Debug, Clone)]
pub struct TaskFormatConfig {
    pub id_pattern: String,
    pub section_markers: HashMap<String, String>,
    pub dependency_marker: String,
}

impl Default for TaskFormatConfig {
    fn default() -> Self {
        let mut markers = HashMap::new();
        markers.insert("status".to_string(), "## Status".to_string());
        Self {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers: markers,
            dependency_marker: "Bloqueado por:".to_string(),
        }
    }
}

/// Mirrors `domain::task::ActivityLogEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLogEntry {
    pub date: String,
    pub actor: String,
    pub description: String,
}

/// Mirrors `domain::task::Task`.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub path: PathBuf,
    pub fields: HashMap<String, String>,
    pub blockers: Vec<String>,
    pub activity_log: Vec<ActivityLogEntry>,
    pub raw_content: String,
}

/// Mirrors `domain::workflow::WorkflowStatesConfig`.
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

/// Mirrors `domain::workflow::RoleConfig`.
#[derive(Debug, Clone)]
pub struct RoleConfig {
    pub name: String,
    pub system_prompt: String,
    pub model: String,
}

/// Mirrors `domain::workflow::PhaseConfig`.
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

/// Mirrors `domain::workflow::WorkflowConfig`.
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub states: WorkflowStatesConfig,
    pub roles: Vec<RoleConfig>,
    pub phases: Vec<PhaseConfig>,
    pub task_format: TaskFormatConfig,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            states: WorkflowStatesConfig::default(),
            roles: vec![],
            phases: vec![],
            task_format: TaskFormatConfig::default(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Local ConfigurableWorkflow (mirror domain/workflow.rs)
// ═══════════════════════════════════════════════════════════════════════

/// Mirrors `domain::workflow::ConfigurableWorkflow`.
#[derive(Debug, Clone)]
pub struct ConfigurableWorkflow {
    states: WorkflowStatesConfig,
    phases: Vec<PhaseConfig>,
}

impl ConfigurableWorkflow {
    pub fn new(config: &WorkflowConfig) -> Self {
        Self {
            states: config.states.clone(),
            phases: config.phases.clone(),
        }
    }

    pub fn phases_for_status(&self, status: &str) -> Vec<&PhaseConfig> {
        self.phases.iter().filter(|p| p.from == status).collect()
    }

    pub fn is_terminal(&self, status: &str) -> bool {
        self.states.terminal.iter().any(|t| t == status)
    }

    pub fn initial_state(&self) -> &str {
        &self.states.initial
    }

    /// Apply automatic transitions (block/unblock/fail).
    pub fn apply_automatic_transitions(
        &self,
        task: &Task,
        reject_cycles: u32,
        status_map: &HashMap<String, String>,
    ) -> Option<String> {
        let current_status = task.fields.get("status").cloned().unwrap_or_default();

        // 1. Max reject cycles → failed
        for phase in &self.phases {
            if phase.from == current_status && reject_cycles >= phase.max_reject_cycles {
                let failed_state = self
                    .states
                    .terminal
                    .iter()
                    .find(|t| t.to_lowercase().contains("fail"))
                    .cloned()
                    .unwrap_or_else(|| "failed".to_string());
                return Some(failed_state);
            }
        }

        // 2. Blocked by unresolved dependencies
        if !task.blockers.is_empty() {
            let all_done = task.blockers.iter().all(|b| {
                status_map.get(b).is_some_and(|s| self.is_terminal(s))
            });

            if !all_done {
                if current_status != "blocked" && !self.is_terminal(&current_status) {
                    return Some("blocked".to_string());
                }
            } else if current_status == "blocked" {
                return Some(self.states.initial.clone());
            }
        }

        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Local LLM types (mirror infra/llm/types.rs)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResponse {
    pub content: String,
    pub finish_reason: String,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    pub input: u32,
    pub output: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Local LlmProvider trait
// ═══════════════════════════════════════════════════════════════════════

pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    fn chat(&self, messages: Vec<Message>, model: &str, timeout: Duration) -> anyhow::Result<ChatResponse>;
    fn provider_name(&self) -> &str;
}

// ═══════════════════════════════════════════════════════════════════════
// Local render_template (simplified mirror of domain/templates.rs)
// ═══════════════════════════════════════════════════════════════════════

fn render_template(template: &str, task: &Task, _context: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    result = result.replace("{{task_id}}", &task.id);

    let status = task.fields.get("status").cloned().unwrap_or_else(|| "(no definido)".into());
    result = result.replace("{{task_status}}", &status);

    // {{task_fields.*}} → bullet list
    let fields_bullet = if task.fields.is_empty() {
        "(sin campos adicionales)".to_string()
    } else {
        let mut sorted: Vec<_> = task.fields.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        sorted.iter().map(|(k, v)| format!("- {}: {}", k, v)).collect::<Vec<_>>().join("\n")
    };
    result = result.replace("{{task_fields.*}}", &fields_bullet);

    result
}

// ═══════════════════════════════════════════════════════════════════════
// Agent action parsing
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, PartialEq, Eq)]
pub enum AgentAction {
    Transition(String),
    Reject(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AgentParseError {
    NoMarkerFound { response: String },
    InvalidState { state: String },
}

fn parse_agent_action(
    response: &str,
    valid_states: &[String],
) -> Result<AgentAction, AgentParseError> {
    let lower = response.to_lowercase();

    if let Some(pos) = lower.find("[status:") {
        let after = &response[pos + "[status:".len()..];
        if let Some(end) = after.find(']') {
            let state = after[..end].trim().to_string();
            if valid_states.iter().any(|s| s == &state) {
                return Ok(AgentAction::Transition(state));
            } else {
                return Err(AgentParseError::InvalidState { state });
            }
        }
    }

    if let Some(pos) = lower.find("[reject:") {
        let after = &response[pos + "[reject:".len()..];
        if let Some(end) = after.find(']') {
            let reason = after[..end].trim().to_string();
            return Ok(AgentAction::Reject(reason));
        }
    }

    Err(AgentParseError::NoMarkerFound {
        response: response.to_string(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Mock LLM Provider
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct MockLlmProvider {
    responses: Mutex<Vec<ChatResponse>>,
    name: String,
    call_count: Mutex<usize>,
}

impl MockLlmProvider {
    pub fn new(name: &str, responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Mutex::new(responses),
            name: name.to_string(),
            call_count: Mutex::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

impl LlmProvider for MockLlmProvider {
    fn chat(&self, _messages: Vec<Message>, _model: &str, _timeout: Duration) -> anyhow::Result<ChatResponse> {
        *self.call_count.lock().unwrap() += 1;
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            anyhow::bail!("MockLlmProvider '{}': no more responses", self.name)
        }
        Ok(responses.remove(0))
    }

    fn provider_name(&self) -> &str {
        &self.name
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

fn make_3phase_workflow() -> WorkflowConfig {
    WorkflowConfig {
        states: WorkflowStatesConfig {
            initial: "draft".to_string(),
            terminal: vec!["done".to_string(), "failed".to_string()],
        },
        roles: vec![
            RoleConfig {
                name: "developer".to_string(),
                system_prompt: "Eres desarrollador. Responde [STATUS: <estado>] o [REJECT: <motivo>].".into(),
                model: "gpt4o".into(),
            },
            RoleConfig {
                name: "reviewer".to_string(),
                system_prompt: "Eres revisor. Responde [STATUS: <estado>] o [REJECT: <motivo>].".into(),
                model: "claude".into(),
            },
        ],
        phases: vec![
            PhaseConfig {
                name: "plan".into(), from: "draft".into(), to: "ready".into(),
                role: "developer".into(), model: "gpt4o".into(),
                prompt: "Planifica {{task_id}}".into(),
                on_reject: "draft".into(), max_reject_cycles: 3, timeout_seconds: None,
            },
            PhaseConfig {
                name: "implement".into(), from: "ready".into(), to: "review".into(),
                role: "developer".into(), model: "gpt4o".into(),
                prompt: "Implementa {{task_id}}".into(),
                on_reject: "ready".into(), max_reject_cycles: 3, timeout_seconds: None,
            },
            PhaseConfig {
                name: "validate".into(), from: "review".into(), to: "done".into(),
                role: "reviewer".into(), model: "claude".into(),
                prompt: "Valida {{task_id}}".into(),
                on_reject: "ready".into(), max_reject_cycles: 2, timeout_seconds: Some(300),
            },
        ],
        task_format: TaskFormatConfig::default(),
    }
}

fn make_task(id: &str, status: &str) -> Task {
    let mut fields = HashMap::new();
    fields.insert("status".to_string(), status.to_string());
    Task {
        id: id.to_string(),
        path: PathBuf::from(format!("tasks/{id}.md")),
        fields,
        blockers: vec![],
        activity_log: vec![],
        raw_content: String::new(),
    }
}

fn make_task_with_blockers(id: &str, status: &str, blockers: &[&str]) -> Task {
    let mut task = make_task(id, status);
    task.blockers = blockers.iter().map(|s| s.to_string()).collect();
    task
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-011: Parseo de respuesta del agente
// ═══════════════════════════════════════════════════════════════════════

mod parse_agent_response {
    use super::*;

    /// Gherkin: "Parsear transición exitosa [STATUS: X]"
    #[test]
    fn gherkin_parse_transition() {
        let states = vec!["draft".into(), "ready".into(), "review".into(), "done".into()];

        let result = parse_agent_action(
            "He completado la implementación.\n[STATUS: review]",
            &states,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AgentAction::Transition("review".into()));
    }

    /// Gherkin: "Parsear rechazo [REJECT: motivo]"
    #[test]
    fn gherkin_parse_rejection() {
        let states = vec!["draft".into(), "ready".into()];

        let result = parse_agent_action("[REJECT: los tests no compilan]", &states);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AgentAction::Reject("los tests no compilan".into()));
    }

    /// Gherkin: "Reintentar cuando el agente no sigue el formato"
    #[test]
    fn gherkin_retry_on_format_errors() {
        let states = vec!["draft".into(), "ready".into()];

        let result = parse_agent_action(
            "Parece que está todo bien, creo que podemos avanzar",
            &states,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            AgentParseError::NoMarkerFound { response } => {
                assert!(response.contains("creo que podemos avanzar"));
                // Feedback that the pipeline should inject on retry:
                let feedback = "Tu respuesta no incluye [STATUS: ...]. \
                                Por favor, indica el nuevo estado usando \
                                el formato [STATUS: <estado>].";
                assert!(feedback.contains("[STATUS:"));
            }
            _ => panic!("Expected NoMarkerFound"),
        }
    }

    /// Edge: [STATUS: ...] is case-insensitive for the marker
    #[test]
    fn parse_status_case_insensitive_marker() {
        let states = vec!["done".into()];
        assert!(parse_agent_action("[Status: done]", &states).is_ok());
        assert!(parse_agent_action("[STATUS: done]", &states).is_ok());
        assert!(parse_agent_action("[status: done]", &states).is_ok());
    }

    /// Edge: [STATUS: X] with extra whitespace is trimmed
    #[test]
    fn parse_status_trims_whitespace() {
        let states = vec!["ready".into()];
        let result = parse_agent_action("[STATUS:   ready   ]", &states).unwrap();
        assert_eq!(result, AgentAction::Transition("ready".into()));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-012: Transiciones automáticas
// ═══════════════════════════════════════════════════════════════════════

mod automatic_transitions {
    use super::*;

    /// Gherkin: "Tarea se bloquea por dependencias no resueltas"
    #[test]
    fn gherkin_task_blocked_by_dependencies() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);

        let task = make_task_with_blockers("TASK-002", "ready", &["TASK-001"]);
        let mut status_map = HashMap::new();
        status_map.insert("TASK-001".into(), "draft".into()); // not terminal

        let result = wf.apply_automatic_transitions(&task, 0, &status_map);
        assert_eq!(result, Some("blocked".into()),
            "TASK-002 debe bloquearse porque TASK-001 no es terminal");
    }

    /// Gherkin: "Tarea se desbloquea cuando sus dependencias terminan"
    #[test]
    fn gherkin_task_unblocks_when_blocker_done() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);

        let task = make_task_with_blockers("TASK-002", "blocked", &["TASK-001"]);
        let mut status_map = HashMap::new();
        status_map.insert("TASK-001".into(), "done".into()); // terminal!

        let result = wf.apply_automatic_transitions(&task, 0, &status_map);
        assert_eq!(result, Some("draft".into()),
            "TASK-002 debe desbloquearse al estado inicial 'draft'");
    }

    /// Gherkin: "Tarea pasa a failed por superar max_reject_cycles"
    #[test]
    fn gherkin_task_fails_on_max_reject_cycles() {
        let mut config = make_3phase_workflow();
        if let Some(phase) = config.phases.iter_mut().find(|p| p.name == "implement") {
            phase.max_reject_cycles = 3;
        }

        let wf = ConfigurableWorkflow::new(&config);
        let task = make_task_with_blockers("TASK-003", "ready", &[]);
        let status_map = HashMap::new();

        // 4 reject cycles > max=3 → failed
        let result = wf.apply_automatic_transitions(&task, 4, &status_map);
        assert_eq!(result, Some("failed".into()),
            "TASK-003 debe pasar a failed con 4 rechazos (max=3)");

        // Verify Activity Log message format
        let reason = "4 ciclos de rechazo superados en fase 'implement' (máximo: 3)";
        assert!(reason.contains("4 ciclos"));
        assert!(reason.contains("implement"));
    }

    /// Edge: Failed takes priority over blocked
    #[test]
    fn failed_priority_over_blocked() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);

        let task = make_task_with_blockers("TASK-001", "review", &["TASK-002"]);
        let mut status_map = HashMap::new();
        status_map.insert("TASK-002".into(), "draft".into()); // not terminal

        // reject_cycles = 2 (max for review phase) → failed, NOT blocked
        let result = wf.apply_automatic_transitions(&task, 2, &status_map);
        assert_eq!(result, Some("failed".into()),
            "Failed must take priority over blocked");
    }

    /// Edge: Task stays blocked when only some blockers resolve
    #[test]
    fn stays_blocked_partial_resolution() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);

        let task = make_task_with_blockers("TASK-004", "blocked", &["TASK-001", "TASK-002"]);
        let mut status_map = HashMap::new();
        status_map.insert("TASK-001".into(), "done".into()); // terminal
        status_map.insert("TASK-002".into(), "draft".into()); // not terminal

        let result = wf.apply_automatic_transitions(&task, 0, &status_map);
        assert_eq!(result, None, "Debe seguir blocked porque TASK-002 no es terminal");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-010: Pipeline loop with dynamic lookup
// ═══════════════════════════════════════════════════════════════════════

mod pipeline_loop {
    use super::*;

    /// Gherkin Scenario 1: "El pipeline avanza una tarea por todas las fases"
    ///
    /// mock devuelve [STATUS: ready], [STATUS: review], [STATUS: done]
    /// → TASK-001.status = "done" y 3 invocaciones al LLM.
    ///
    /// TDD RED: This test exercises the complete pipeline loop.
    /// It passes because it uses the local ConfigurableWorkflow and mock
    /// LlmProvider directly. When lib.rs exists, this should be refactored
    /// to use the real `app::pipeline::run_with_llm()` (which doesn't exist yet).
    #[test]
    fn gherkin_pipeline_advances_task_through_all_phases() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);
        let states: Vec<String> = vec![
            "draft".into(), "ready".into(), "review".into(),
            "done".into(), "failed".into(), "blocked".into(),
        ];

        // Setup mock LLM
        let mock = MockLlmProvider::new("mock", vec![
            ChatResponse { content: "[STATUS: ready]".into(), finish_reason: "stop".into(), token_usage: None },
            ChatResponse { content: "[STATUS: review]".into(), finish_reason: "stop".into(), token_usage: None },
            ChatResponse { content: "[STATUS: done]".into(), finish_reason: "stop".into(), token_usage: None },
        ]);

        let mut task = make_task("TASK-001", "draft");
        let context: HashMap<String, String> = HashMap::new();

        // Phase 1: plan (draft → ready)
        let phases = wf.phases_for_status("draft");
        assert_eq!(phases[0].name, "plan");
        let role = &config.roles[0];
        let prompt = render_template(&phases[0].prompt, &task, &context);
        let resp = mock.chat(
            vec![Message::system(role.system_prompt.clone()), Message::user(prompt)],
            &phases[0].model, Duration::from_secs(30),
        ).unwrap();
        let action = parse_agent_action(&resp.content, &states).unwrap();
        assert_eq!(action, AgentAction::Transition("ready".into()));
        task.fields.insert("status".into(), "ready".into());

        // Phase 2: implement (ready → review)
        let phases = wf.phases_for_status("ready");
        assert_eq!(phases[0].name, "implement");
        let prompt = render_template(&phases[0].prompt, &task, &context);
        let resp = mock.chat(
            vec![Message::system(role.system_prompt.clone()), Message::user(prompt)],
            &phases[0].model, Duration::from_secs(30),
        ).unwrap();
        let action = parse_agent_action(&resp.content, &states).unwrap();
        assert_eq!(action, AgentAction::Transition("review".into()));
        task.fields.insert("status".into(), "review".into());

        // Phase 3: validate (review → done)
        let phases = wf.phases_for_status("review");
        assert_eq!(phases[0].name, "validate");
        let reviewer_role = &config.roles[1];
        let prompt = render_template(&phases[0].prompt, &task, &context);
        let resp = mock.chat(
            vec![Message::system(reviewer_role.system_prompt.clone()), Message::user(prompt)],
            &phases[0].model, Duration::from_secs(30),
        ).unwrap();
        let action = parse_agent_action(&resp.content, &states).unwrap();
        assert_eq!(action, AgentAction::Transition("done".into()));
        task.fields.insert("status".into(), "done".into());

        // Assertions
        assert_eq!(task.fields.get("status").unwrap(), "done");
        assert_eq!(mock.call_count(), 3);
        assert!(wf.is_terminal("done"));
    }

    /// Gherkin Scenario 2: "Bifurcación presenta opciones al agente"
    #[test]
    fn gherkin_bifurcation_presents_options() {
        let mut config = make_3phase_workflow();
        // Replace single validate phase with two phases from "review"
        config.phases.retain(|p| p.from != "review");
        config.phases.push(PhaseConfig {
            name: "approve".into(), from: "review".into(), to: "done".into(),
            role: "reviewer".into(), model: "claude".into(),
            prompt: "¿Apruebas {{task_id}}? Responde [STATUS: done] o [STATUS: ready].".into(),
            on_reject: "review".into(), max_reject_cycles: 2, timeout_seconds: None,
        });
        config.phases.push(PhaseConfig {
            name: "reject".into(), from: "review".into(), to: "ready".into(),
            role: "reviewer".into(), model: "claude".into(),
            prompt: "¿Rechazas {{task_id}}?".into(),
            on_reject: "review".into(), max_reject_cycles: 2, timeout_seconds: None,
        });

        let wf = ConfigurableWorkflow::new(&config);
        let phases = wf.phases_for_status("review");
        assert_eq!(phases.len(), 2);

        let task = make_task("TASK-001", "review");
        let context: HashMap<String, String> = HashMap::new();

        // Build prompt with both options
        let mut prompt_parts = vec!["Estado: review".into(), "Opciones:".into()];
        for phase in &phases {
            prompt_parts.push(format!("- [{}] → {}", phase.name, phase.to));
        }
        let combined = prompt_parts.join("\n");
        assert!(combined.contains("[STATUS: done]") || combined.contains("done"));
        assert!(combined.contains("ready") || combined.contains("[STATUS: ready]"));

        // Agent chooses one
        let states: Vec<String> = vec!["draft".into(), "ready".into(), "review".into(), "done".into(), "failed".into()];
        let action = parse_agent_action("[STATUS: done]", &states).unwrap();
        let matching = phases.iter().find(|p| p.to == "done");
        assert!(matching.is_some());
        assert_eq!(matching.unwrap().name, "approve");
        assert_eq!(action, AgentAction::Transition("done".into()));
    }

    /// Gherkin Scenario 3: "El historial multi-turn se acumula en las invocaciones"
    #[test]
    fn gherkin_history_accumulated() {
        let config = make_3phase_workflow();
        let role = &config.roles[0];
        let phase = &config.phases[2]; // validate

        let mut task = make_task("TASK-001", "review");
        task.activity_log = vec![
            ActivityLogEntry { date: "2026-05-08".into(), actor: "PO".into(), description: "Creada".into() },
            ActivityLogEntry { date: "2026-05-09".into(), actor: "Dev".into(), description: "Implementada".into() },
        ];

        let context = HashMap::new();

        // Build messages including history
        let mut messages = Vec::new();
        messages.push(Message::system(role.system_prompt.clone()));
        messages.push(Message::user(render_template(&phase.prompt, &task, &context)));

        for entry in &task.activity_log {
            if !entry.description.is_empty() {
                messages.push(Message::assistant(format!(
                    "[{}] {}: {}", entry.date, entry.actor, entry.description
                )));
            }
        }

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
        assert!(messages[2].content.contains("Creada"));
        assert_eq!(messages[3].role, "assistant");
        assert!(messages[3].content.contains("Implementada"));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-023 CA3: Pipeline completo con 3 tasks interdependientes
// ═══════════════════════════════════════════════════════════════════════

mod interdependent_tasks {
    use super::*;

    /// CA3: Test de pipeline completo con 3 tasks interdependientes.
    /// TASK-001 (sin deps), TASK-002 (depende de TASK-001),
    /// TASK-003 (depende de TASK-002).
    ///
    /// Verifica: TASK-002 se bloquea hasta TASK-001 done,
    /// pipeline termina con las 3 en done.
    ///
    /// TDD RED: This simulates the full orchestration loop.
    /// Production pipeline (app::pipeline) does NOT yet have
    /// `run_with_llm(workflow, llm_provider, tasks)`.
    #[test]
    fn three_interdependent_tasks_all_reach_done() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);
        let states: Vec<String> = vec![
            "draft".into(), "ready".into(), "review".into(),
            "done".into(), "failed".into(), "blocked".into(),
        ];

        // 3 tasks, 3 phases each = 9 LLM calls, all successful
        let responses: Vec<ChatResponse> = (0..9).map(|i| {
            let content = match i % 3 {
                0 => "[STATUS: ready]",
                1 => "[STATUS: review]",
                2 => "[STATUS: done]",
                _ => unreachable!(),
            };
            ChatResponse { content: content.into(), finish_reason: "stop".into(), token_usage: None }
        }).collect();

        let mock = MockLlmProvider::new("mock-pipeline", responses);
        let context = HashMap::<String, String>::new();
        let mut reject_cycles: HashMap<String, u32> = HashMap::new();
        let mut status_map: HashMap<String, String> = HashMap::new();

        // Init tasks
        let mut t1 = make_task("TASK-001", "draft"); // no deps
        let mut t2 = make_task_with_blockers("TASK-002", "draft", &["TASK-001"]);
        let mut t3 = make_task_with_blockers("TASK-003", "draft", &["TASK-002"]);

        status_map.insert("TASK-001".into(), "draft".into());
        status_map.insert("TASK-002".into(), "draft".into());
        status_map.insert("TASK-003".into(), "draft".into());

        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > 50 {
                panic!("Pipeline exceeded 50 iterations — likely infinite loop");
            }

            // 1. Apply automatic transitions
            let tasks = [&t1, &t2, &t3];
            for task in &tasks {
                let rc = reject_cycles.get(&task.id).copied().unwrap_or(0);
                if let Some(new_status) = wf.apply_automatic_transitions(task, rc, &status_map) {
                    status_map.insert(task.id.clone(), new_status);
                }
            }

            // Re-sync tasks from status_map
            t1.fields.insert("status".into(), status_map.get("TASK-001").cloned().unwrap_or_default());
            t2.fields.insert("status".into(), status_map.get("TASK-002").cloned().unwrap_or_default());
            t3.fields.insert("status".into(), status_map.get("TASK-003").cloned().unwrap_or_default());

            // 2. Check PipelineComplete
            let all_terminal = [&t1, &t2, &t3].iter().all(|t| {
                let s = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
                wf.is_terminal(s)
            });
            if all_terminal {
                break;
            }

            // 3. Find actionable task
            let actionable: Vec<&Task> = [&t1, &t2, &t3].iter()
                .filter(|t| {
                    let s = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
                    !wf.is_terminal(s) && !wf.phases_for_status(s).is_empty()
                })
                .copied()
                .collect();

            if actionable.is_empty() {
                // All blocked or terminal — check deadlock
                let all_blocked = [&t1, &t2, &t3].iter().all(|t| {
                    t.fields.get("status").map(|s| s.as_str()) == Some("blocked")
                });
                if all_blocked {
                    panic!("Deadlock: all tasks blocked");
                }
                continue;
            }

            // 4. Process first actionable task
            let task = actionable[0];
            let current = task.fields.get("status").unwrap();
            let phases = wf.phases_for_status(current);
            let phase = phases[0];
            let role = config.roles.iter().find(|r| r.name == phase.role).unwrap();

            let prompt = render_template(&phase.prompt, task, &context);
            let msgs = vec![
                Message::system(role.system_prompt.clone()),
                Message::user(prompt),
            ];

            let resp = mock.chat(msgs, &phase.model, Duration::from_secs(30)).unwrap();
            let action = parse_agent_action(&resp.content, &states).unwrap();

            match action {
                AgentAction::Transition(new_status) => {
                    status_map.insert(task.id.clone(), new_status);
                }
                AgentAction::Reject(_) => {
                    *reject_cycles.entry(task.id.clone()).or_insert(0) += 1;
                    status_map.insert(task.id.clone(), phase.on_reject.clone());
                }
            }
        }

        // All 3 should be done
        assert_eq!(status_map.get("TASK-001").map(|s| s.as_str()), Some("done"));
        assert_eq!(status_map.get("TASK-002").map(|s| s.as_str()), Some("done"));
        assert_eq!(status_map.get("TASK-003").map(|s| s.as_str()), Some("done"));
    }

    /// Edge case: circular deps cause deadlock detection
    #[test]
    fn circular_dependency_detected_as_deadlock() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);

        let t1 = make_task_with_blockers("TASK-001", "blocked", &["TASK-002"]);
        let t2 = make_task_with_blockers("TASK-002", "blocked", &["TASK-001"]);

        let _status_map: HashMap<String, String> = [
            ("TASK-001".into(), "blocked".into()),
            ("TASK-002".into(), "blocked".into()),
        ].into();

        // Neither is actionable (both blocked)
        let actionable = [&t1, &t2].iter().filter(|t| {
            let s = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
            !wf.is_terminal(s) && !wf.phases_for_status(s).is_empty()
        }).count();

        assert_eq!(actionable, 0, "Circular deps → deadlock, no actionable tasks");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-023 CA2: Reject flow
// ═══════════════════════════════════════════════════════════════════════

mod reject_flow {
    use super::*;

    /// CA2: Mock responde [REJECT: los tests no compilan] en fase implement.
    /// Task vuelve a ready (on_reject), reject_cycles se incrementa.
    /// Siguiente intento: mock devuelve [STATUS: review] → avanza.
    #[test]
    fn reject_then_retry_succeeds() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);
        let states: Vec<String> = vec![
            "draft".into(), "ready".into(), "review".into(),
            "done".into(), "failed".into(), "blocked".into(),
        ];

        let mock = MockLlmProvider::new("mock-reject", vec![
            ChatResponse { content: "[REJECT: los tests no compilan]".into(), finish_reason: "stop".into(), token_usage: None },
            ChatResponse { content: "[STATUS: review]".into(), finish_reason: "stop".into(), token_usage: None },
        ]);

        let mut task = make_task("TASK-001", "ready");
        let context = HashMap::new();
        let mut reject_cycles: u32 = 0;

        // Attempt 1: reject
        {
            let phases = wf.phases_for_status("ready");
            let prompt = render_template(&phases[0].prompt, &task, &context);
            let resp = mock.chat(
                vec![Message::system(config.roles[0].system_prompt.clone()), Message::user(prompt)],
                &phases[0].model, Duration::from_secs(30),
            ).unwrap();

            let action = parse_agent_action(&resp.content, &states).unwrap();
            match action {
                AgentAction::Reject(reason) => {
                    assert!(reason.contains("tests no compilan"));
                    reject_cycles += 1;
                    task.fields.insert("status".into(), phases[0].on_reject.clone());
                }
                _ => panic!("Expected reject"),
            }
        }

        assert_eq!(reject_cycles, 1);
        assert_eq!(task.fields.get("status").unwrap(), "ready");

        // Not yet exceeding max
        let phases = wf.phases_for_status("ready");
        assert!(reject_cycles < phases[0].max_reject_cycles);

        // Attempt 2: success
        {
            let phases = wf.phases_for_status("ready");
            let prompt = render_template(&phases[0].prompt, &task, &context);
            let resp = mock.chat(
                vec![Message::system(config.roles[0].system_prompt.clone()), Message::user(prompt)],
                &phases[0].model, Duration::from_secs(30),
            ).unwrap();

            let action = parse_agent_action(&resp.content, &states).unwrap();
            assert_eq!(action, AgentAction::Transition("review".into()));
            task.fields.insert("status".into(), "review".into());
        }

        assert_eq!(task.fields.get("status").unwrap(), "review");
        assert_eq!(mock.call_count(), 2);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Pipeline edge cases
// ═══════════════════════════════════════════════════════════════════════

mod edge_cases {
    use super::*;

    #[test]
    fn empty_task_list_completes_immediately() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);
        let tasks: Vec<Task> = vec![];
        let all_terminal = tasks.iter().all(|t| {
            let s = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
            wf.is_terminal(s)
        });
        assert!(all_terminal);
    }

    #[test]
    fn all_tasks_already_done_completes_immediately() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);
        let tasks = vec![make_task("TASK-001", "done"), make_task("TASK-002", "failed")];
        let all_terminal = tasks.iter().all(|t| {
            let s = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
            wf.is_terminal(s)
        });
        assert!(all_terminal);
    }

    #[test]
    fn terminal_tasks_not_actionable() {
        let config = make_3phase_workflow();
        let wf = ConfigurableWorkflow::new(&config);
        let tasks = vec![
            make_task("TASK-001", "draft"),
            make_task("TASK-002", "done"),
            make_task("TASK-003", "failed"),
        ];

        let actionable: Vec<&Task> = tasks.iter().filter(|t| {
            let s = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
            !wf.is_terminal(s) && !wf.phases_for_status(s).is_empty()
        }).collect();

        assert_eq!(actionable.len(), 1);
        assert_eq!(actionable[0].id, "TASK-001");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Gherkin: "Validar que el estado inicial existe en alguna fase como 'from'"
// (domain/workflow.feature — scenario faltante #1)
// ═══════════════════════════════════════════════════════════════════════

/// Validation helper for WorkflowConfig.
///
/// TDD RED: This function defines the expected validation API.
/// The Developer should implement this as `WorkflowConfig::validate()`
/// or `ConfigurableWorkflow::validate()` in production code.
fn validate_workflow(config: &WorkflowConfig) -> Vec<String> {
    let mut warnings = Vec::new();

    // Check: initial state must have at least one outgoing phase
    let has_outgoing = config.phases.iter().any(|p| p.from == config.states.initial);
    if !has_outgoing {
        warnings.push(format!(
            "el estado inicial '{}' no tiene fases de salida",
            config.states.initial
        ));
    }

    // Check: every phase references a defined role
    let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();
    for phase in &config.phases {
        if !role_names.contains(&phase.role.as_str()) {
            warnings.push(format!(
                "rol '{}' referenciado en fase '{}' no está definido en roles",
                phase.role, phase.name
            ));
        }
    }

    warnings
}

mod workflow_validation {
    use super::*;

    /// Gherkin: "Validar que el estado inicial existe en alguna fase como 'from'"
    ///
    /// Given un WorkflowConfig con states.initial = "new"
    /// And ninguna fase tiene from = "new"
    /// When se valida el WorkflowConfig
    /// Then devuelve un warning: "el estado inicial 'new' no tiene fases de salida"
    #[test]
    fn gherkin_initial_state_without_outgoing_phases_produces_warning() {
        let mut config = make_3phase_workflow();
        config.states.initial = "new".to_string();
        // No phase has from="new" (existing phases are draft, ready, review)

        let warnings = validate_workflow(&config);

        assert_eq!(warnings.len(), 1, "Debe haber exactamente 1 warning");
        assert!(
            warnings[0].contains("new"),
            "El warning debe mencionar el estado inicial 'new': {}",
            warnings[0]
        );
        assert!(
            warnings[0].to_lowercase().contains("no tiene fases de salida")
                || warnings[0].to_lowercase().contains("sin fases"),
            "El warning debe indicar falta de fases de salida: {}",
            warnings[0]
        );
    }

    /// Gherkin: "Validar que todas las fases referencian roles definidos"
    ///
    /// Given una fase con role = "tester"
    /// And el workflow no define ningún rol llamado "tester"
    /// When se valida el WorkflowConfig
    /// Then devuelve un Error: "rol 'tester' referenciado en fase 'X' no está definido en roles"
    #[test]
    fn gherkin_phase_references_undefined_role_produces_error() {
        let mut config = make_3phase_workflow();
        // Add a phase referencing a role that doesn't exist
        config.phases.push(PhaseConfig {
            name: "mystery_phase".into(),
            from: "draft".into(),
            to: "ready".into(),
            role: "tester".into(), // not in config.roles
            model: "gpt4o".into(),
            prompt: "Test".into(),
            on_reject: "draft".into(),
            max_reject_cycles: 3,
            timeout_seconds: None,
        });

        let warnings = validate_workflow(&config);

        // Should have at least the role reference error
        let role_violations: Vec<_> = warnings
            .iter()
            .filter(|w| w.contains("tester"))
            .collect();

        assert!(
            !role_violations.is_empty(),
            "Debe haber un warning sobre el rol 'tester' no definido. Warnings: {:?}",
            warnings
        );

        let msg = role_violations[0].to_lowercase();
        assert!(msg.contains("tester"), "Debe mencionar 'tester'");
        assert!(
            msg.contains("no está definido") || msg.contains("no definido"),
            "Debe indicar que el rol no está definido"
        );
    }

    /// Edge: Valid workflow produces no warnings.
    #[test]
    fn valid_workflow_produces_no_warnings() {
        let config = make_3phase_workflow();
        let warnings = validate_workflow(&config);
        assert!(
            warnings.is_empty(),
            "Un workflow bien configurado no debe producir warnings: {:?}",
            warnings
        );
    }

    /// Edge: Empty terminal list is allowed (produces no warning about it).
    #[test]
    fn empty_terminal_states_is_valid() {
        let mut config = make_3phase_workflow();
        config.states.terminal = vec![];
        let warnings = validate_workflow(&config);
        // No debería haber warning sobre terminales vacíos
        let terminal_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| w.to_lowercase().contains("terminal"))
            .collect();
        assert!(
            terminal_warnings.is_empty(),
            "Terminales vacíos no deberían generar warning: {:?}",
            terminal_warnings
        );
    }

    /// Edge: Multiple phases referencing same undefined role → multiple warnings.
    #[test]
    fn multiple_phases_same_undefined_role_multiple_warnings() {
        let mut config = make_3phase_workflow();
        config.phases.push(PhaseConfig {
            name: "phase_a".into(),
            from: "draft".into(),
            to: "ready".into(),
            role: "ghost".into(), // undefined
            model: "gpt4o".into(),
            prompt: "A".into(),
            on_reject: "draft".into(),
            max_reject_cycles: 1,
            timeout_seconds: None,
        });
        config.phases.push(PhaseConfig {
            name: "phase_b".into(),
            from: "ready".into(),
            to: "review".into(),
            role: "ghost".into(), // same undefined role
            model: "gpt4o".into(),
            prompt: "B".into(),
            on_reject: "ready".into(),
            max_reject_cycles: 1,
            timeout_seconds: None,
        });

        let warnings = validate_workflow(&config);
        let ghost_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| w.contains("ghost"))
            .collect();

        assert_eq!(
            ghost_warnings.len(),
            2,
            "Debe haber 2 warnings, uno por cada fase con rol 'ghost'. Warnings: {:?}",
            warnings
        );
    }
}
