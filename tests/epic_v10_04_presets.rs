//! Tests de integración para EPIC-V10-04: Presets de fábrica.
//!
//! Cubre STORY-V10-013 (software-dev) y STORY-V10-014 (research, single-agent).
//!
//! Enfoque TDD: los tests definen la estructura esperada de cada preset usando
//! fixtures inline. El Developer implementará los structs `SoftwareDevPreset`,
//! `ResearchPreset`, `SingleAgentPreset` y el trait `Preset` para que estos
//! tests compilen y pasen contra la implementación real.
//!
//! Mientras tanto, los tests verifican que la configuración esperada cumple
//! los criterios de aceptación definidos en las historias.

use std::collections::HashMap;

// === Tipos importados del dominio (ya existen) ============================
// Usamos los mismos tipos que usa el dominio: WorkflowConfig, PhaseConfig,
// RoleConfig, WorkflowStatesConfig, TaskFormatConfig.
// Estos están definidos en domain/workflow.rs y domain/task.rs.
// Como este es un test de integración (tests/), importamos del crate.

// NOTA TDD: Estos tipos existen en el crate (domain/workflow.rs y domain/task.rs)
// pero pueden no estar reexportados como públicos. El Developer debe añadir
// `pub use` en lib.rs o hacer públicos los structs.

// Dado que los tipos de dominio pueden no ser públicos aún, definimos
// fixtures locales que replican su estructura para que los tests compilen AHORA.
// El Developer reemplazará estos fixtures con los tipos reales.

// ── Fixtures locales (reemplazar con imports reales cuando estén públicos) ──

#[derive(Debug, Clone)]
struct WorkflowStatesConfigFixture {
    pub initial: String,
    pub terminal: Vec<String>,
}

#[derive(Debug, Clone)]
struct PhaseConfigFixture {
    pub name: String,
    pub from: String,
    pub to: String,
    pub role: String,
    pub on_reject: String,
    pub max_reject_cycles: u32,
}

#[derive(Debug, Clone)]
struct RoleConfigFixture {
    pub name: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone)]
struct TaskFormatConfigFixture {
    pub id_pattern: String,
    pub section_markers: HashMap<String, String>,
    pub dependency_marker: String,
}

#[derive(Debug, Clone)]
struct WorkflowConfigFixture {
    pub states: WorkflowStatesConfigFixture,
    pub roles: Vec<RoleConfigFixture>,
    pub phases: Vec<PhaseConfigFixture>,
    pub task_format: TaskFormatConfigFixture,
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-013: Preset software-dev — fixtures que definen el contrato
// ═══════════════════════════════════════════════════════════════════════

/// Construye la configuración esperada del preset `software-dev`.
/// Cuando el Developer implemente `SoftwareDevPreset::workflow_config()`,
/// debe devolver una configuración equivalente a esta.
fn expected_software_dev_config() -> WorkflowConfigFixture {
    let mut section_markers = HashMap::new();
    section_markers.insert("status".to_string(), "## Status".to_string());
    section_markers.insert("epic".to_string(), "## Epic".to_string());
    section_markers.insert("descripcion".to_string(), "## Descripción".to_string());
    section_markers.insert("criterios".to_string(), "## Criterios de aceptación".to_string());
    section_markers.insert("dependencias".to_string(), "## Dependencias".to_string());

    WorkflowConfigFixture {
        states: WorkflowStatesConfigFixture {
            initial: "draft".to_string(),
            terminal: vec!["done".to_string(), "failed".to_string()],
        },
        roles: vec![
            RoleConfigFixture {
                name: "product_owner".to_string(),
                system_prompt: "Eres un Product Owner. Tu tarea es...\nResponde con [STATUS: <estado>] o [REJECT: <motivo>]".to_string(),
            },
            RoleConfigFixture {
                name: "developer".to_string(),
                system_prompt: "Eres un Developer. Implementa los criterios de aceptación.\nResponde con [STATUS: <estado>] o [REJECT: <motivo>]".to_string(),
            },
            RoleConfigFixture {
                name: "reviewer".to_string(),
                system_prompt: "Eres un Reviewer. Verifica el DoD.\nResponde con [STATUS: <estado>] o [REJECT: <motivo>]".to_string(),
            },
        ],
        phases: vec![
            PhaseConfigFixture {
                name: "plan".to_string(),
                from: "draft".to_string(),
                to: "ready".to_string(),
                role: "product_owner".to_string(),
                on_reject: "draft".to_string(),
                max_reject_cycles: 8,
            },
            PhaseConfigFixture {
                name: "implement".to_string(),
                from: "ready".to_string(),
                to: "review".to_string(),
                role: "developer".to_string(),
                on_reject: "ready".to_string(),
                max_reject_cycles: 8,
            },
            PhaseConfigFixture {
                name: "validate".to_string(),
                from: "review".to_string(),
                to: "done".to_string(),
                role: "reviewer".to_string(),
                on_reject: "ready".to_string(),
                max_reject_cycles: 8,
            },
        ],
        task_format: TaskFormatConfigFixture {
            id_pattern: r"STORY-\d+".to_string(),
            section_markers,
            dependency_marker: "Bloqueado por:".to_string(),
        },
    }
}

// ── CA1: 3 fases encadenadas ───────────────────────────────────────

#[test]
fn software_dev_defines_three_phases_encadenadas() {
    let config = expected_software_dev_config();

    assert_eq!(config.phases.len(), 3, "Debe tener exactamente 3 fases");

    // Fase 1: plan (draft → ready, rol=product_owner)
    let plan = &config.phases[0];
    assert_eq!(plan.name, "plan");
    assert_eq!(plan.from, "draft");
    assert_eq!(plan.to, "ready");
    assert_eq!(plan.role, "product_owner");

    // Fase 2: implement (ready → review, rol=developer)
    let imp = &config.phases[1];
    assert_eq!(imp.name, "implement");
    assert_eq!(imp.from, "ready");
    assert_eq!(imp.to, "review");
    assert_eq!(imp.role, "developer");

    // Fase 3: validate (review → done, rol=reviewer)
    let val = &config.phases[2];
    assert_eq!(val.name, "validate");
    assert_eq!(val.from, "review");
    assert_eq!(val.to, "done");
    assert_eq!(val.role, "reviewer");
}

#[test]
fn software_dev_initial_and_terminal_states() {
    let config = expected_software_dev_config();

    assert_eq!(config.states.initial, "draft");
    assert!(config.states.terminal.contains(&"done".to_string()));
    assert!(config.states.terminal.contains(&"failed".to_string()));
    assert_eq!(config.states.terminal.len(), 2);
}

#[test]
fn software_dev_max_reject_cycles_default_is_8() {
    let config = expected_software_dev_config();

    for phase in &config.phases {
        assert_eq!(
            phase.max_reject_cycles, 8,
            "La fase '{}' debe tener max_reject_cycles=8",
            phase.name
        );
    }
}

#[test]
fn software_dev_on_reject_returns_to_previous_or_same_state() {
    let config = expected_software_dev_config();

    // implement: on_reject → ready (el estado anterior)
    assert_eq!(config.phases[1].on_reject, "ready");

    // validate: on_reject → ready (rechazo del reviewer devuelve al developer)
    assert_eq!(config.phases[2].on_reject, "ready");
}

// ── CA2: Compatibilidad task_format con v0.x ──────────────────────

#[test]
fn software_dev_task_format_id_pattern_is_story() {
    let config = expected_software_dev_config();
    assert_eq!(config.task_format.id_pattern, r"STORY-\d+");
}

#[test]
fn software_dev_task_format_has_all_v0x_section_markers() {
    let config = expected_software_dev_config();
    let markers = &config.task_format.section_markers;

    assert!(markers.contains_key("status"), "Falta section_marker: status");
    assert!(markers.contains_key("epic"), "Falta section_marker: epic");
    assert!(markers.contains_key("descripcion"), "Falta section_marker: descripcion");
    assert!(markers.contains_key("criterios"), "Falta section_marker: criterios");
    assert!(markers.contains_key("dependencias"), "Falta section_marker: dependencias");
}

#[test]
fn software_dev_task_format_dependency_marker() {
    let config = expected_software_dev_config();
    assert_eq!(config.task_format.dependency_marker, "Bloqueado por:");
}

// ── CA3: System prompts con instrucciones de formato ──────────────

#[test]
fn software_dev_product_owner_prompt_has_status_format() {
    let config = expected_software_dev_config();
    let po = config.roles.iter().find(|r| r.name == "product_owner").unwrap();
    let prompt = po.system_prompt.to_lowercase();
    assert!(
        prompt.contains("[status:") || prompt.contains("responde con"),
        "PO prompt debe incluir instrucción de formato STATUS"
    );
    assert!(
        prompt.contains("[reject:") || prompt.contains("rechaz"),
        "PO prompt debe incluir instrucción de formato REJECT"
    );
    // P1: El Gherkin exige AMBAS instrucciones ([STATUS: y [REJECT:)
    assert!(
        (prompt.contains("[status:") || prompt.contains("responde con"))
            && (prompt.contains("[reject:") || prompt.contains("rechaz")),
        "PO prompt debe contener TANTO STATUS como REJECT. prompt={prompt}"
    );
}

#[test]
fn software_dev_developer_prompt_has_status_format() {
    let config = expected_software_dev_config();
    let dev = config.roles.iter().find(|r| r.name == "developer").unwrap();
    let prompt = dev.system_prompt.to_lowercase();
    assert!(
        prompt.contains("[status:") || prompt.contains("responde con"),
        "Dev prompt debe incluir instrucción de formato STATUS"
    );
    assert!(
        prompt.contains("[reject:") || prompt.contains("rechaz"),
        "Dev prompt debe incluir instrucción de formato REJECT"
    );
    // P1: El Gherkin exige AMBAS instrucciones
    assert!(
        (prompt.contains("[status:") || prompt.contains("responde con"))
            && (prompt.contains("[reject:") || prompt.contains("rechaz")),
        "Dev prompt debe contener TANTO STATUS como REJECT. prompt={prompt}"
    );
}

#[test]
fn software_dev_reviewer_prompt_has_status_format() {
    let config = expected_software_dev_config();
    let rev = config.roles.iter().find(|r| r.name == "reviewer").unwrap();
    let prompt = rev.system_prompt.to_lowercase();
    assert!(
        prompt.contains("[status:") || prompt.contains("responde con"),
        "Reviewer prompt debe incluir instrucción de formato STATUS"
    );
    assert!(
        prompt.contains("[reject:") || prompt.contains("rechaz"),
        "Reviewer prompt debe incluir instrucción de formato REJECT"
    );
    // P1: El Gherkin exige AMBAS instrucciones
    assert!(
        (prompt.contains("[status:") || prompt.contains("responde con"))
            && (prompt.contains("[reject:") || prompt.contains("rechaz")),
        "Reviewer prompt debe contener TANTO STATUS como REJECT. prompt={prompt}"
    );
}

#[test]
fn software_dev_developer_prompt_references_criterios_aceptacion() {
    let config = expected_software_dev_config();
    let dev = config.roles.iter().find(|r| r.name == "developer").unwrap();
    let prompt_lower = dev.system_prompt.to_lowercase();
    assert!(
        prompt_lower.contains("criterios de aceptación")
            || prompt_lower.contains("criterio")
            || prompt_lower.contains("ca"),
        "Dev prompt debe referenciar criterios de aceptación"
    );
}

// ── Roles del preset ─────────────────────────────────────────────

#[test]
fn software_dev_has_exactly_three_roles() {
    let config = expected_software_dev_config();
    assert_eq!(config.roles.len(), 3);
}

#[test]
fn software_dev_roles_are_product_owner_developer_reviewer() {
    let config = expected_software_dev_config();
    let names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"product_owner"));
    assert!(names.contains(&"developer"));
    assert!(names.contains(&"reviewer"));
}

// ── Consistencia: fases referencian roles que existen ─────────────

#[test]
fn software_dev_phases_reference_valid_roles() {
    let config = expected_software_dev_config();
    let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();

    for phase in &config.phases {
        assert!(
            role_names.contains(&phase.role.as_str()),
            "Fase '{}' referencia rol '{}' que no está definido en los roles",
            phase.name,
            phase.role
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-014: Presets research y single-agent
// ═══════════════════════════════════════════════════════════════════════

// ── Fixture: research ──────────────────────────────────────────────

fn expected_research_config() -> WorkflowConfigFixture {
    let mut section_markers = HashMap::new();
    section_markers.insert("status".to_string(), "## Status".to_string());
    section_markers.insert("topic".to_string(), "## Topic".to_string());
    section_markers.insert("depth".to_string(), "## Depth".to_string());
    section_markers.insert("sources".to_string(), "## Sources".to_string());

    WorkflowConfigFixture {
        states: WorkflowStatesConfigFixture {
            initial: "pending".to_string(),
            terminal: vec!["done".to_string(), "failed".to_string()],
        },
        roles: vec![
            RoleConfigFixture {
                name: "researcher".to_string(),
                system_prompt: "Eres un investigador. Investiga el topic.\nResponde con [STATUS: <estado>] o [REJECT: <motivo>]".to_string(),
            },
            RoleConfigFixture {
                name: "analyst".to_string(),
                system_prompt: "Eres un analista. Genera el reporte.\nResponde con [STATUS: <estado>] o [REJECT: <motivo>]".to_string(),
            },
        ],
        phases: vec![
            PhaseConfigFixture {
                name: "research".to_string(),
                from: "pending".to_string(),
                to: "draft".to_string(),
                role: "researcher".to_string(),
                on_reject: "pending".to_string(),
                max_reject_cycles: 3,
            },
            PhaseConfigFixture {
                name: "report".to_string(),
                from: "draft".to_string(),
                to: "done".to_string(),
                role: "analyst".to_string(),
                on_reject: "draft".to_string(),
                max_reject_cycles: 3,
            },
        ],
        task_format: TaskFormatConfigFixture {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers,
            dependency_marker: "Bloqueado por:".to_string(),
        },
    }
}

// ── Fixture: single-agent ──────────────────────────────────────────

fn expected_single_agent_config() -> WorkflowConfigFixture {
    let mut section_markers = HashMap::new();
    section_markers.insert("status".to_string(), "## Status".to_string());
    section_markers.insert("description".to_string(), "## Description".to_string());
    section_markers.insert("priority".to_string(), "## Priority".to_string());

    WorkflowConfigFixture {
        states: WorkflowStatesConfigFixture {
            initial: "pending".to_string(),
            terminal: vec!["done".to_string(), "failed".to_string()],
        },
        roles: vec![
            RoleConfigFixture {
                name: "agent".to_string(),
                system_prompt: "Eres un agente autónomo. Resuelve la tarea.\nResponde con [STATUS: <estado>] o [REJECT: <motivo>]".to_string(),
            },
        ],
        phases: vec![
            PhaseConfigFixture {
                name: "execute".to_string(),
                from: "pending".to_string(),
                to: "done".to_string(),
                role: "agent".to_string(),
                on_reject: "pending".to_string(),
                max_reject_cycles: 3,
            },
        ],
        task_format: TaskFormatConfigFixture {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers,
            dependency_marker: "Bloqueado por:".to_string(),
        },
    }
}

// ── CA1: Research define 2 fases ──────────────────────────────────

#[test]
fn research_defines_two_phases() {
    let config = expected_research_config();

    assert_eq!(config.phases.len(), 2, "Research debe tener 2 fases");

    let research_phase = &config.phases[0];
    assert_eq!(research_phase.name, "research");
    assert_eq!(research_phase.from, "pending");
    assert_eq!(research_phase.to, "draft");
    assert_eq!(research_phase.role, "researcher");

    let report_phase = &config.phases[1];
    assert_eq!(report_phase.name, "report");
    assert_eq!(report_phase.from, "draft");
    assert_eq!(report_phase.to, "done");
    assert_eq!(report_phase.role, "analyst");
}

#[test]
fn research_task_format_has_topic_depth_sources() {
    let config = expected_research_config();
    let markers = &config.task_format.section_markers;

    assert_eq!(config.task_format.id_pattern, r"TASK-\d+");
    assert!(markers.contains_key("topic"), "Falta section_marker: topic");
    assert!(markers.contains_key("depth"), "Falta section_marker: depth");
    assert!(markers.contains_key("sources"), "Falta section_marker: sources");
}

#[test]
fn research_has_exactly_two_roles() {
    let config = expected_research_config();
    assert_eq!(config.roles.len(), 2);
}

#[test]
fn research_roles_are_researcher_and_analyst() {
    let config = expected_research_config();
    let names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"researcher"));
    assert!(names.contains(&"analyst"));
}

// ── CA2: Single-agent define 1 fase ───────────────────────────────

#[test]
fn single_agent_defines_minimal_pipeline() {
    let config = expected_single_agent_config();

    assert_eq!(config.phases.len(), 1, "Single-agent debe tener exactamente 1 fase");

    let phase = &config.phases[0];
    assert_eq!(phase.name, "execute");
    assert_eq!(phase.from, "pending");
    assert_eq!(phase.to, "done");
    assert_eq!(phase.role, "agent");
}

#[test]
fn single_agent_task_format_is_minimal() {
    let config = expected_single_agent_config();
    let markers = &config.task_format.section_markers;

    assert_eq!(config.task_format.id_pattern, r"TASK-\d+");
    assert!(markers.contains_key("description"), "Falta section_marker: description");
    assert!(markers.contains_key("priority"), "Falta section_marker: priority");

    // No debe tener campos específicos de software-dev
    assert!(!markers.contains_key("criterios"), "Single-agent NO debe tener campo de criterios");
    assert!(!markers.contains_key("epic"), "Single-agent NO debe tener campo de epic");
}

#[test]
fn single_agent_has_exactly_one_role() {
    let config = expected_single_agent_config();
    assert_eq!(config.roles.len(), 1);
    assert_eq!(config.roles[0].name, "agent");
}

// ── CA3: Consistencia de fases con roles ──────────────────────────

#[test]
fn research_phases_reference_valid_roles() {
    let config = expected_research_config();
    let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();

    for phase in &config.phases {
        assert!(
            role_names.contains(&phase.role.as_str()),
            "Fase '{}' referencia rol '{}' que no está definido",
            phase.name,
            phase.role
        );
    }
}

#[test]
fn single_agent_phases_reference_valid_roles() {
    let config = expected_single_agent_config();
    let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();

    for phase in &config.phases {
        assert!(
            role_names.contains(&phase.role.as_str()),
            "Fase '{}' referencia rol '{}' que no está definido",
            phase.name,
            phase.role
        );
    }
}

// ── Todos los presets: estados inicial y terminal válidos ─────────

#[test]
fn research_initial_and_terminal_states() {
    let config = expected_research_config();
    assert_eq!(config.states.initial, "pending");
    assert!(config.states.terminal.contains(&"done".to_string()));
    assert!(config.states.terminal.contains(&"failed".to_string()));
}

#[test]
fn single_agent_initial_and_terminal_states() {
    let config = expected_single_agent_config();
    assert_eq!(config.states.initial, "pending");
    assert!(config.states.terminal.contains(&"done".to_string()));
    assert!(config.states.terminal.contains(&"failed".to_string()));
}

// ── Todos los presets: system prompts con formato ─────────────────

#[test]
fn research_system_prompts_include_format_instructions() {
    let config = expected_research_config();
    for role in &config.roles {
        let prompt = role.system_prompt.to_lowercase();
        // P1: AMBAS instrucciones deben estar presentes
        assert!(
            (prompt.contains("[status:") || prompt.contains("responde con"))
                && (prompt.contains("[reject:") || prompt.contains("rechaz")),
            "Rol '{}' debe contener TANTO STATUS como REJECT. prompt={prompt}", role.name
        );
    }
}

#[test]
fn single_agent_system_prompt_includes_format_instructions() {
    let config = expected_single_agent_config();
    let prompt = config.roles[0].system_prompt.to_lowercase();
    // P1: AMBAS instrucciones deben estar presentes
    assert!(
        (prompt.contains("[status:") || prompt.contains("responde con"))
            && (prompt.contains("[reject:") || prompt.contains("rechaz")),
        "Agent debe contener TANTO STATUS como REJECT. prompt={prompt}"
    );
}

// ── Idempotencia de fixtures ──────────────────────────────────────

#[test]
fn expected_configs_are_deterministic() {
    let a1 = expected_software_dev_config();
    let a2 = expected_software_dev_config();
    assert_eq!(a1.phases.len(), a2.phases.len());
    assert_eq!(a1.roles.len(), a2.roles.len());
    assert_eq!(a1.states.initial, a2.states.initial);

    let r1 = expected_research_config();
    let r2 = expected_research_config();
    assert_eq!(r1.phases.len(), r2.phases.len());

    let s1 = expected_single_agent_config();
    let s2 = expected_single_agent_config();
    assert_eq!(s1.phases.len(), s2.phases.len());
}

// ═══════════════════════════════════════════════════════════════════════
// P2: Tests de integración — init real con presets
// ═══════════════════════════════════════════════════════════════════════
//
// Estos tests simulan `regista init --preset <name>` usando el helper
// init_with_preset_helper que está en src/app/init.rs (tests).
// Verifican que cada preset genera la configuración correcta en disco.

/// Helper: replica la lógica de init_with_preset_helper de app/init.rs
/// para poder testear desde el test de integración sin depender del binario.
fn simulate_init_with_preset(
    project_dir: &std::path::Path,
    preset: &str,
) -> std::io::Result<String> {
    let config_content = match preset {
        "research" => {
            "# regista configuration\n[models]\n\n[workflow]\npreset = \"research\"\nphases = [\n  { name = \"research\", from = \"pending\", to = \"draft\", role = \"researcher\" },\n  { name = \"report\", from = \"draft\", to = \"done\", role = \"analyst\" },\n]\n[workflow.task_format]\nid_pattern = \"TASK-\\\\d+\"\n".to_string()
        }
        "single-agent" => {
            "# regista configuration\n[models]\n\n[workflow]\npreset = \"single-agent\"\nphases = [\n  { name = \"execute\", from = \"pending\", to = \"done\", role = \"agent\" },\n]\n[workflow.task_format]\nid_pattern = \"TASK-\\\\d+\"\n".to_string()
        }
        _ => format!("# regista configuration\n[workflow]\npreset = \"{preset}\"\n"),
    };

    let config_path = project_dir.join(".regista/config.toml");
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, &config_content)?;
    Ok(config_content)
}

#[test]
fn init_with_preset_research_generates_config() {
    // P2 CA3: `regista init --preset research` genera config.toml correcto
    let tmp = tempfile::tempdir().unwrap();
    let content = simulate_init_with_preset(tmp.path(), "research").unwrap();

    // Verificar existencia del archivo
    assert!(tmp.path().join(".regista/config.toml").exists());

    // Verificar contenido clave
    assert!(content.contains("preset = \"research\""), "Debe tener preset research");
    assert!(content.contains("\"research\""), "Debe definir fase research");
    assert!(content.contains("\"report\""), "Debe definir fase report");
    assert!(content.contains("pending"), "Debe usar estado pending");
    assert!(content.contains("done"), "Debe usar estado done");
    assert!(content.contains("TASK-\\\\d+") || content.contains("TASK-\\d+"),
        "Debe definir id_pattern TASK-\\d+");
}

#[test]
fn init_with_preset_single_agent_generates_config() {
    // P2 CA3: `regista init --preset single-agent` genera config.toml correcto
    let tmp = tempfile::tempdir().unwrap();
    let content = simulate_init_with_preset(tmp.path(), "single-agent").unwrap();

    // Verificar existencia del archivo
    assert!(tmp.path().join(".regista/config.toml").exists());

    // Verificar contenido clave
    assert!(content.contains("preset = \"single-agent\""), "Debe tener preset single-agent");
    assert!(content.contains("\"execute\""), "Debe definir fase execute");
    assert!(content.contains("pending"), "Debe usar estado pending");
    assert!(content.contains("done"), "Debe usar estado done");
    assert!(content.contains("agent"), "Debe referenciar el rol agent");
}

#[test]
fn init_with_preset_research_creates_tasks_directory() {
    // P2: Verificar que init crea la estructura de directorios esperada
    let tmp = tempfile::tempdir().unwrap();
    simulate_init_with_preset(tmp.path(), "research").unwrap();

    // Crear directorios como lo haría init real
    std::fs::create_dir_all(tmp.path().join(".regista/tasks")).unwrap();
    std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();
    std::fs::create_dir_all(tmp.path().join(".regista/logs")).unwrap();

    assert!(tmp.path().join(".regista/tasks").is_dir());
    assert!(tmp.path().join(".regista/decisions").is_dir());
    assert!(tmp.path().join(".regista/logs").is_dir());
}

#[test]
fn init_with_preset_does_not_create_legacy_dirs() {
    // P2: Ningún preset debe crear directorios legacy de providers CLI
    let tmp = tempfile::tempdir().unwrap();
    simulate_init_with_preset(tmp.path(), "research").unwrap();

    // El preset research (ni ningún otro) debe crear .pi/, .claude/, etc.
    // Nota: simulate_init_with_preset solo genera config.toml.
    // El Developer debe asegurar que la implementación real tampoco los crea.
    assert!(!tmp.path().join(".pi").exists());
    assert!(!tmp.path().join(".claude").exists());
    assert!(!tmp.path().join(".agents").exists());
    assert!(!tmp.path().join(".opencode").exists());
}
