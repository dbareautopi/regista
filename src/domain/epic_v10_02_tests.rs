//! Tests de integración para EPIC-V10-02: Dominio Genérico.
//!
//! Verifica la interacción entre todos los módulos del dominio:
//! - `task.rs` — Task genérico con parseo configurable
//! - `workflow.rs` — ConfigurableWorkflow desde TOML
//! - `templates.rs` — Templates con {{variables}}
//! - `deadlock.rs` — analyze_deadlock() con Task
//! - `graph.rs` — DependencyGraph::from_tasks()
//!
//! Estos tests simulan flujos completos: carga de tareas → grafo →
//! workflow → deadlock → templates, sin depender de I/O real.

use crate::domain::deadlock::{analyze_deadlock, DeadlockResolutionV10};
use crate::domain::graph::DependencyGraph;
use crate::domain::task::Task;
use crate::domain::templates::render_template;
use crate::domain::workflow::{
    ConfigurableWorkflow, PhaseConfig, RoleConfig, WorkflowConfig, WorkflowStatesConfig,
};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Helpers ────────────────────────────────────────────────────────────

/// Crea una Task sintética con campos mínimos.
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

/// Crea una Task con campos adicionales (para tests de templates).
fn make_task_with_fields(
    id: &str,
    status: &str,
    extra_fields: &[(&str, &str)],
    blockers: &[&str],
) -> Task {
    let mut fields = HashMap::new();
    fields.insert("status".to_string(), status.to_string());
    for (k, v) in extra_fields {
        fields.insert(k.to_string(), v.to_string());
    }
    Task {
        id: id.to_string(),
        path: PathBuf::from(format!("tasks/{id}.md")),
        fields,
        blockers: blockers.iter().map(|s| s.to_string()).collect(),
        activity_log: vec![],
        raw_content: String::new(),
    }
}

/// Workflow canónico de desarrollo software (similar al Background de workflow.feature).
fn make_dev_workflow_config() -> WorkflowConfig {
    WorkflowConfig {
        states: WorkflowStatesConfig {
            initial: "draft".to_string(),
            terminal: vec!["done".to_string(), "failed".to_string()],
        },
        roles: vec![
            RoleConfig {
                name: "product_owner".to_string(),
                system_prompt: "Eres {{role_name}}. Refina la tarea {{task_id}}.".to_string(),
                model: "gpt4o".to_string(),
            },
            RoleConfig {
                name: "qa_engineer".to_string(),
                system_prompt: "Eres {{role_name}}. Escribe tests para {{task_id}}.".to_string(),
                model: "gpt4o".to_string(),
            },
            RoleConfig {
                name: "developer".to_string(),
                system_prompt:
                    "Eres {{role_name}}. Implementa {{task_id}} [{{task_status}}].".to_string(),
                model: "gpt4o".to_string(),
            },
            RoleConfig {
                name: "reviewer".to_string(),
                system_prompt: "Eres {{role_name}}. Revisa {{task_id}}.".to_string(),
                model: "claude".to_string(),
            },
        ],
        phases: vec![
            PhaseConfig {
                name: "plan".to_string(),
                from: "draft".to_string(),
                to: "ready".to_string(),
                role: "product_owner".to_string(),
                model: "gpt4o".to_string(),
                prompt:
                    "Refina {{task_id}}. Estado actual: {{task_status}}. Descripción: {{task_fields.description}}"
                        .to_string(),
                on_reject: "draft".to_string(),
                max_reject_cycles: 3,
                timeout_seconds: None,
            },
            PhaseConfig {
                name: "test".to_string(),
                from: "ready".to_string(),
                to: "tests_ready".to_string(),
                role: "qa_engineer".to_string(),
                model: "gpt4o".to_string(),
                prompt: "Escribe tests para {{task_id}}. Prioridad: {{task_fields.priority}}"
                    .to_string(),
                on_reject: "ready".to_string(),
                max_reject_cycles: 2,
                timeout_seconds: None,
            },
            PhaseConfig {
                name: "implement".to_string(),
                from: "tests_ready".to_string(),
                to: "in_review".to_string(),
                role: "developer".to_string(),
                model: "gpt4o".to_string(),
                prompt: "Implementa {{task_id}}. {{last_rejection}}".to_string(),
                on_reject: "tests_ready".to_string(),
                max_reject_cycles: 3,
                timeout_seconds: Some(1800),
            },
            PhaseConfig {
                name: "review".to_string(),
                from: "in_review".to_string(),
                to: "done".to_string(),
                role: "reviewer".to_string(),
                model: "claude".to_string(),
                prompt: "Revisa {{task_id}}. Bloqueantes: {{blockers}}".to_string(),
                on_reject: "tests_ready".to_string(),
                max_reject_cycles: 2,
                timeout_seconds: Some(600),
            },
        ],
        task_format: crate::domain::task::TaskFormatConfig::default(),
    }
}

/// Workflow alternativo tipo "research" con estados y roles distintos.
fn make_research_workflow_config() -> WorkflowConfig {
    WorkflowConfig {
        states: WorkflowStatesConfig {
            initial: "backlog".to_string(),
            terminal: vec!["published".to_string(), "rejected".to_string()],
        },
        roles: vec![
            RoleConfig {
                name: "researcher".to_string(),
                system_prompt: "Eres un investigador. Tema: {{task_fields.topic}}".to_string(),
                model: "claude".to_string(),
            },
            RoleConfig {
                name: "reviewer".to_string(),
                system_prompt: "Eres un revisor académico.".to_string(),
                model: "gpt4o".to_string(),
            },
        ],
        phases: vec![
            PhaseConfig {
                name: "investigate".to_string(),
                from: "backlog".to_string(),
                to: "draft".to_string(),
                role: "researcher".to_string(),
                model: "claude".to_string(),
                prompt: "Investiga {{task_id}}: {{task_fields.topic}}".to_string(),
                on_reject: "backlog".to_string(),
                max_reject_cycles: 3,
                timeout_seconds: None,
            },
            PhaseConfig {
                name: "peer_review".to_string(),
                from: "draft".to_string(),
                to: "published".to_string(),
                role: "reviewer".to_string(),
                model: "gpt4o".to_string(),
                prompt:
                    "Revisa {{task_id}}. Tema: {{task_fields.topic}}. Notas: {{task_fields.notes}}"
                        .to_string(),
                on_reject: "draft".to_string(),
                max_reject_cycles: 2,
                timeout_seconds: Some(900),
            },
        ],
        task_format: crate::domain::task::TaskFormatConfig {
            id_pattern: r"RESEARCH-\d+".to_string(),
            section_markers: {
                let mut m = HashMap::new();
                m.insert("status".to_string(), "## Status".to_string());
                m.insert("topic".to_string(), "## Topic".to_string());
                m.insert("notes".to_string(), "## Notes".to_string());
                m
            },
            dependency_marker: "Requires:".to_string(),
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sección 1: Integración Task + Templates
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn task_with_fields_rendered_in_template() {
    let task = make_task_with_fields(
        "TASK-042",
        "in_progress",
        &[("priority", "high"), ("description", "Parser genérico de tareas")],
        &["TASK-001", "TASK-002"],
    );

    let template =
        "Tarea {{task_id}}: {{task_fields.description}} ({{task_status}}) [{{task_fields.priority}}]";
    let result = render_template(template, &task, &HashMap::new());

    assert_eq!(
        result,
        "Tarea TASK-042: Parser genérico de tareas (in_progress) [high]"
    );
}

#[test]
fn template_with_context_and_task_fields() {
    let task = make_task_with_fields(
        "TASK-005",
        "pending",
        &[("effort", "8"), ("assignee", "Bob")],
        &[],
    );

    let mut ctx = HashMap::new();
    ctx.insert("sprint".to_string(), "Sprint 42".to_string());
    ctx.insert("role_name".to_string(), "Developer".to_string());

    let template = "{{role_name}}: Trabaja en {{task_id}} para {{context.sprint}}. Esfuerzo: {{task_fields.effort}}. Asignado: {{task_fields.assignee}}";
    let result = render_template(template, &task, &ctx);

    assert_eq!(
        result,
        "Developer: Trabaja en TASK-005 para Sprint 42. Esfuerzo: 8. Asignado: Bob"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Sección 2: Integración Task + DependencyGraph + Workflow
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn graph_and_workflow_detect_blocked_tasks() {
    let workflow = ConfigurableWorkflow::new(&make_dev_workflow_config());

    let tasks = vec![
        make_task("TASK-001", "done", &[]),
        make_task("TASK-002", "draft", &[]),
        make_task("TASK-003", "ready", &["TASK-002"]),
        make_task("TASK-004", "tests_ready", &["TASK-001"]),
        make_task("TASK-005", "in_review", &[]),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);

    // Verificar estructura del grafo
    assert_eq!(graph.blocks_count("TASK-001"), 1);
    assert_eq!(graph.blocks_count("TASK-002"), 1);
    assert_eq!(graph.blocks_count("TASK-004"), 0);

    // Verificar que TASK-003 (ready) tiene dependencia no resuelta (TASK-002 draft → no terminal)
    let status_map: HashMap<String, String> = tasks
        .iter()
        .map(|t| {
            (
                t.id.clone(),
                t.fields.get("status").cloned().unwrap_or_default(),
            )
        })
        .collect();

    let auto = workflow.apply_automatic_transitions(&tasks[2], &graph, 0, &status_map);
    assert_eq!(
        auto,
        Some("blocked".to_string()),
        "TASK-003 (ready) con blocker TASK-002 (draft) debe ser bloqueada"
    );

    // TASK-004 (tests_ready) con blocker TASK-001 (done) → sin transición automática
    let auto2 = workflow.apply_automatic_transitions(&tasks[3], &graph, 0, &status_map);
    assert_eq!(
        auto2, None,
        "TASK-004 con blocker done (terminal) no debe ser bloqueada"
    );
}

#[test]
fn workflow_phase_prompt_renders_with_task() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    let task = make_task_with_fields(
        "TASK-001",
        "draft",
        &[
            ("description", "Sistema de login OAuth2"),
            ("priority", "critical"),
        ],
        &[],
    );

    let phases = workflow.phases_for_status("draft");
    assert_eq!(phases.len(), 1);
    assert_eq!(phases[0].name, "plan");

    // Renderizar el prompt de la fase con la task
    let rendered = render_template(&phases[0].prompt, &task, &HashMap::new());
    assert!(rendered.contains("TASK-001"));
    assert!(rendered.contains("draft"));
    assert!(rendered.contains("Sistema de login OAuth2"));
}

#[test]
fn phases_for_status_with_correct_from_matching() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    // Verificar que cada estado devuelve las fases correctas
    assert_eq!(workflow.phases_for_status("draft").len(), 1);
    assert_eq!(workflow.phases_for_status("ready").len(), 1);
    assert_eq!(workflow.phases_for_status("tests_ready").len(), 1);
    assert_eq!(workflow.phases_for_status("in_review").len(), 1);
    assert_eq!(workflow.phases_for_status("done").len(), 0);
    assert_eq!(workflow.phases_for_status("blocked").len(), 0);

    // Verificar nombres de fases
    assert_eq!(workflow.phases_for_status("draft")[0].name, "plan");
    assert_eq!(workflow.phases_for_status("ready")[0].name, "test");
    assert_eq!(workflow.phases_for_status("tests_ready")[0].name, "implement");
    assert_eq!(workflow.phases_for_status("in_review")[0].name, "review");
}

// ═══════════════════════════════════════════════════════════════════════
// Sección 3: Integración Deadlock + Workflow + Graph + Task
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn deadlock_analysis_integrates_with_workflow_and_graph() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    // Escenario: 5 tareas, algunas en draft, algunas bloqueadas
    let tasks = vec![
        make_task("TASK-001", "done", &[]),
        make_task("TASK-002", "draft", &[]),
        make_task("TASK-003", "draft", &[]),
        make_task("TASK-004", "blocked", &["TASK-002"]),
        make_task("TASK-005", "blocked", &["TASK-002", "TASK-003"]),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    let result = analyze_deadlock(&tasks, &graph, &workflow);

    match result {
        DeadlockResolutionV10::InvokeAgentFor {
            task_id, unblocks, ..
        } => {
            // TASK-002 bloquea a TASK-004 y TASK-005 (2 tareas)
            // TASK-003 bloquea a TASK-005 (1 tarea)
            assert_eq!(task_id, "TASK-002", "Debe priorizar la que más desbloquea");
            assert_eq!(unblocks, 2);
        }
        _ => panic!("Expected InvokeAgentFor, got {result:?}"),
    }
}

#[test]
fn deadlock_with_custom_research_workflow() {
    let config = make_research_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    // Usar el estado "draft" que sí es reconocido por analyze_deadlock
    let tasks = vec![
        make_task_with_fields(
            "RESEARCH-001",
            "draft",
            &[
                ("topic", "Evolución de LLMs en 2025"),
                ("notes", "Revisar papers de arXiv"),
            ],
            &[],
        ),
        make_task_with_fields(
            "RESEARCH-002",
            "draft",
            &[("topic", "Fine-tuning eficiente"), ("notes", "LoRA y QLoRA")],
            &[],
        ),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    let result = analyze_deadlock(&tasks, &graph, &workflow);

    match result {
        DeadlockResolutionV10::InvokeAgentFor { task_id, reason, .. } => {
            assert!(
                task_id == "RESEARCH-001" || task_id == "RESEARCH-002",
                "Debe seleccionar una de las tareas en draft: {task_id}"
            );
            assert!(
                reason.contains("Draft"),
                "El motivo debe mencionar el estado: {reason}"
            );
            assert!(
                !reason.contains("STORY"),
                "No debe hardcodear 'STORY': {reason}"
            );
        }
        _ => panic!("Expected InvokeAgentFor for research tasks, got {result:?}"),
    }
}

#[test]
fn deadlock_pipeline_complete_with_mixed_terminal() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    let tasks = vec![
        make_task("TASK-001", "done", &[]),
        make_task("TASK-002", "failed", &[]),
        make_task("TASK-003", "done", &[]),
        make_task("TASK-004", "done", &["TASK-001"]),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    let result = analyze_deadlock(&tasks, &graph, &workflow);

    match result {
        DeadlockResolutionV10::PipelineComplete => {}
        _ => panic!("Expected PipelineComplete for mixed terminal states, got {result:?}"),
    }
}

#[test]
fn deadlock_no_deadlock_when_actionable() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    // TASK-001 está en "in_review" (accionable según el workflow)
    let tasks = vec![
        make_task("TASK-001", "in_review", &[]),
        make_task("TASK-002", "draft", &[]),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    let result = analyze_deadlock(&tasks, &graph, &workflow);

    match result {
        DeadlockResolutionV10::NoDeadlock => {}
        _ => panic!("Expected NoDeadlock with actionable task, got {result:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sección 4: Integración completa de flujo (Task → Graph → Workflow → Deadlock)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn full_pipeline_simulation_dev_workflow() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    // ── Fase 1: Cargar tareas ──────────────────────────────────────
    let mut tasks = vec![
        make_task_with_fields(
            "TASK-001",
            "draft",
            &[
                ("description", "API REST de usuarios"),
                ("priority", "high"),
            ],
            &[],
        ),
        make_task_with_fields(
            "TASK-002",
            "draft",
            &[("description", "Autenticación JWT"), ("priority", "medium")],
            &["TASK-001"],
        ),
        make_task_with_fields(
            "TASK-003",
            "draft",
            &[("description", "Dashboard admin"), ("priority", "low")],
            &["TASK-001", "TASK-002"],
        ),
    ];

    // ── Fase 2: Construir grafo ────────────────────────────────────
    let graph = DependencyGraph::from_tasks(&tasks);
    assert_eq!(graph.blocks_count("TASK-001"), 2);
    assert_eq!(graph.blocks_count("TASK-002"), 1);
    assert_eq!(graph.blocks_count("TASK-003"), 0);

    // ── Fase 3: Aplicar transiciones automáticas ───────────────────
    let status_map_before: HashMap<String, String> = tasks
        .iter()
        .map(|t| {
            (
                t.id.clone(),
                t.fields.get("status").cloned().unwrap_or_default(),
            )
        })
        .collect();

    // TASK-002 (draft) depende de TASK-001 (draft) → se bloquea automáticamente
    let auto_t2 = workflow.apply_automatic_transitions(&tasks[1], &graph, 0, &status_map_before);
    assert_eq!(
        auto_t2,
        Some("blocked".to_string()),
        "TASK-002 (draft) depende de TASK-001 (draft, no terminal) → debe bloquearse"
    );

    // TASK-001 (draft) sin dependencias → no transiciona automáticamente
    let auto_t1 = workflow.apply_automatic_transitions(&tasks[0], &graph, 0, &status_map_before);
    assert_eq!(
        auto_t1, None,
        "TASK-001 draft without blockers should not auto-transition"
    );

    // ── Fase 4: Deadlock ───────────────────────────────────────────
    // Con todas en draft, se debe detectar deadlock
    let deadlock = analyze_deadlock(&tasks, &graph, &workflow);
    match deadlock {
        DeadlockResolutionV10::InvokeAgentFor {
            task_id, unblocks, ..
        } => {
            assert_eq!(
                task_id, "TASK-001",
                "Debe priorizar TASK-001 (bloquea 2)"
            );
            assert_eq!(unblocks, 2);
        }
        _ => panic!("Expected InvokeAgentFor, got {deadlock:?}"),
    }

    // ── Fase 5: Simular avance de TASK-001 a ready ─────────────────
    tasks[0]
        .fields
        .insert("status".to_string(), "ready".to_string());

    let status_map_after: HashMap<String, String> = tasks
        .iter()
        .map(|t| {
            (
                t.id.clone(),
                t.fields.get("status").cloned().unwrap_or_default(),
            )
        })
        .collect();

    // TASK-002 está en draft, depende de TASK-001 (ready → no terminal)
    // → El sistema bloquea automáticamente las tareas con dependencias no resueltas,
    //   incluso en draft (porque draft no es terminal y tiene blockers no terminal)
    let auto2 = workflow.apply_automatic_transitions(&tasks[1], &graph, 0, &status_map_after);
    assert_eq!(
        auto2,
        Some("blocked".to_string()),
        "TASK-002 (draft) with blocker TASK-001 (ready, not terminal) should be auto-blocked"
    );

    // ── Fase 6: Plantillas de fase ─────────────────────────────────
    let phases = workflow.phases_for_status("ready");
    assert_eq!(phases.len(), 1);
    assert_eq!(phases[0].name, "test");

    let mut ctx = HashMap::new();
    ctx.insert("role_name".to_string(), "QA Engineer".to_string());
    let prompt = render_template(&phases[0].prompt, &tasks[0], &ctx);
    assert!(prompt.contains("TASK-001"));
    assert!(prompt.contains("high"));
}

#[test]
fn full_pipeline_simulation_research_workflow() {
    let config = make_research_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    // ── Fase 1: Cargar tareas de investigación en estado "draft" ──
    // (analyze_deadlock solo reconoce "draft" y "blocked" como stuck)
    let mut tasks = vec![
        make_task_with_fields(
            "RESEARCH-001",
            "draft",
            &[
                ("topic", "Mecanismos de atención en transformers"),
                ("notes", "Vaswani et al. 2017"),
            ],
            &[],
        ),
        make_task_with_fields(
            "RESEARCH-002",
            "draft",
            &[
                ("topic", "Modelos de difusión para texto"),
                ("notes", "Revisar estado del arte"),
            ],
            &["RESEARCH-001"],
        ),
        make_task_with_fields(
            "RESEARCH-003",
            "draft",
            &[
                ("topic", "RLHF y alineación"),
                ("notes", "InstructGPT, Anthropic's CAI"),
            ],
            &[],
        ),
    ];

    // ── Fase 2: Construir grafo ────────────────────────────────────
    let graph = DependencyGraph::from_tasks(&tasks);
    assert_eq!(graph.blocks_count("RESEARCH-001"), 1);
    assert_eq!(graph.blocks_count("RESEARCH-003"), 0);
    assert!(!graph.has_any_cycle());

    // ── Fase 3: Deadlock en draft ─────────────────────────────────
    let deadlock = analyze_deadlock(&tasks, &graph, &workflow);
    match deadlock {
        DeadlockResolutionV10::InvokeAgentFor {
            task_id, unblocks, ..
        } => {
            // RESEARCH-001 bloquea a RESEARCH-002 (1 tarea)
            assert_eq!(
                task_id, "RESEARCH-001",
                "Debe priorizar RESEARCH-001 (bloquea 1)"
            );
            assert_eq!(unblocks, 1);
        }
        _ => panic!("Expected InvokeAgentFor in research deadlock, got {deadlock:?}"),
    }

    // ── Fase 4: Simular avance a published ─────────────────────────
    tasks[0]
        .fields
        .insert("status".to_string(), "published".to_string());

    // ── Fase 5: Verificar que "published" es terminal ──────────────
    assert!(workflow.is_terminal("published"));

    // ── Fase 6: Renderizar template de peer review ─────────────────
    let phases = workflow.phases_for_status("draft");
    assert_eq!(phases.len(), 1);
    assert_eq!(phases[0].name, "peer_review");
    assert_eq!(phases[0].to, "published");

    let mut ctx = HashMap::new();
    ctx.insert("role_name".to_string(), "Academic Reviewer".to_string());
    let prompt = render_template(&phases[0].prompt, &tasks[1], &ctx);
    assert!(prompt.contains("RESEARCH-002"));
    assert!(prompt.contains("Modelos de difusión para texto"));
    assert!(prompt.contains("Revisar estado del arte"));
}

// ═══════════════════════════════════════════════════════════════════════
// Sección 5: Edge cases de integración
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn graph_with_circular_dependencies_integrates_with_deadlock() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    let tasks = vec![
        make_task("TASK-001", "blocked", &["TASK-002"]),
        make_task("TASK-002", "blocked", &["TASK-001"]),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    assert!(graph.has_any_cycle());
    assert!(graph.has_cycle_from("TASK-001"));
    assert!(graph.has_cycle_from("TASK-002"));

    let result = analyze_deadlock(&tasks, &graph, &workflow);
    match result {
        DeadlockResolutionV10::InvokeAgentFor { reason, .. } => {
            assert!(reason.contains("ciclo"), "Debe detectar ciclo: {reason}");
        }
        _ => panic!("Expected InvokeAgentFor for circular deps, got {result:?}"),
    }
}

#[test]
fn reject_cycles_across_multiple_tasks() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    let tasks = vec![
        make_task("TASK-001", "in_review", &[]),
        make_task("TASK-002", "in_review", &[]),
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    let status_map: HashMap<String, String> = tasks
        .iter()
        .map(|t| {
            (
                t.id.clone(),
                t.fields.get("status").cloned().unwrap_or_default(),
            )
        })
        .collect();

    // review phase: max_reject_cycles = 2
    // TASK-001 con reject_cycles = 1 → no failed
    let r1 = workflow.apply_automatic_transitions(&tasks[0], &graph, 1, &status_map);
    assert_eq!(r1, None);

    // TASK-001 con reject_cycles = 2 → failed
    let r2 = workflow.apply_automatic_transitions(&tasks[0], &graph, 2, &status_map);
    assert_eq!(r2, Some("failed".to_string()));

    // TASK-002 con reject_cycles = 2 → failed
    let r3 = workflow.apply_automatic_transitions(&tasks[1], &graph, 2, &status_map);
    assert_eq!(r3, Some("failed".to_string()));
}

#[test]
fn template_rendering_with_all_variable_types_in_realistic_prompt() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    let task = make_task_with_fields(
        "TASK-099",
        "in_review",
        &[
            ("description", "Sistema de caché distribuido"),
            ("priority", "critical"),
            ("effort", "13"),
        ],
        &["TASK-050", "TASK-051"],
    );

    let phases = workflow.phases_for_status("in_review");
    let phase_prompt = &phases[0].prompt;

    let mut ctx = HashMap::new();
    ctx.insert("role_name".to_string(), "Senior Reviewer".to_string());
    ctx.insert("sprint".to_string(), "Sprint 8".to_string());

    let result = render_template(phase_prompt, &task, &ctx);

    assert!(result.contains("TASK-099"), "Should contain task ID");
    assert!(result.contains("TASK-050"), "Should contain blocker 1");
    assert!(result.contains("TASK-051"), "Should contain blocker 2");
    assert!(
        !result.contains("{{"),
        "No unresolved braces should remain: {result}"
    );
}

#[test]
fn empty_tasks_list_is_pipeline_complete() {
    let config = make_dev_workflow_config();
    let workflow = ConfigurableWorkflow::new(&config);

    let tasks: Vec<Task> = vec![];
    let graph = DependencyGraph::from_tasks(&tasks);
    let result = analyze_deadlock(&tasks, &graph, &workflow);

    match result {
        DeadlockResolutionV10::PipelineComplete => {}
        _ => panic!("Expected PipelineComplete for empty list, got {result:?}"),
    }
}

#[test]
fn task_format_config_isolation_between_workflows() {
    let dev_config = make_dev_workflow_config();
    let research_config = make_research_workflow_config();

    assert_eq!(dev_config.task_format.id_pattern, r"TASK-\d+");
    assert_eq!(research_config.task_format.id_pattern, r"RESEARCH-\d+");

    assert_ne!(
        dev_config.task_format.dependency_marker,
        research_config.task_format.dependency_marker
    );

    // Cada workflow tiene su propia TaskFormatConfig aislada
    let dev_wf = ConfigurableWorkflow::new(&dev_config);
    let research_wf = ConfigurableWorkflow::new(&research_config);

    assert_eq!(dev_wf.initial_state(), "draft");
    assert_eq!(research_wf.initial_state(), "backlog");
}

#[test]
fn workflow_terminal_states_match_deadlock_pipeline_complete() {
    let mut config = make_dev_workflow_config();
    config.states.terminal = vec!["completed".to_string(), "aborted".to_string()];

    let workflow = ConfigurableWorkflow::new(&config);

    let tasks = vec![
        {
            let mut t = make_task("TASK-001", "completed", &[]);
            t.fields.insert("status".to_string(), "completed".to_string());
            t
        },
        {
            let mut t = make_task("TASK-002", "aborted", &[]);
            t.fields.insert("status".to_string(), "aborted".to_string());
            t
        },
    ];

    let graph = DependencyGraph::from_tasks(&tasks);
    let result = analyze_deadlock(&tasks, &graph, &workflow);

    match result {
        DeadlockResolutionV10::PipelineComplete => {}
        _ => panic!(
            "Expected PipelineComplete with custom terminal states, got {result:?}"
        ),
    }
}
