//! Tests de integración para EPIC-V10-04: Presets de fábrica.
//!
//! Cubre STORY-V10-013 (software-dev) y STORY-V10-014 (research, single-agent).
//!
//! NOTA TDD: Estos tests referencian el módulo `app::presets` y sus structs
//! (`SoftwareDevPreset`, `ResearchPreset`, `SingleAgentPreset`, `Preset` trait)
//! que aún no existen. El Developer debe implementarlos para que compilen.
//!
//! Una vez compilen, verifican los criterios de aceptación definidos en las historias.

// ── NOTA TDD: descomenta estos imports cuando el Developer implemente app::presets ──
// use regista::app::presets::{Preset, SoftwareDevPreset, ResearchPreset, SingleAgentPreset};
// use regista::app::presets::PresetRegistry;
// use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-013: Preset software-dev
// ═══════════════════════════════════════════════════════════════════════

// TODO: Descomenta estos tests cuando app::presets esté implementado.
// Actualmente fallan en compilación porque el módulo no existe.
// El Developer debe:
//   1. Crear src/app/presets/mod.rs con el trait Preset
//   2. Crear src/app/presets/software_dev.rs con SoftwareDevPreset
//   3. Hacer públicos los structs en lib.rs o main.rs

/*
#[test]
fn software_dev_preset_defines_three_phases() {
    // CA1: El preset define 3 fases encadenadas
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.phases.len(), 3, "Debe tener exactamente 3 fases");

    // Fase 1: plan (draft → ready, rol=product_owner)
    let plan_phase = &config.phases[0];
    assert_eq!(plan_phase.name, "plan");
    assert_eq!(plan_phase.from, "draft");
    assert_eq!(plan_phase.to, "ready");
    assert_eq!(plan_phase.role, "product_owner");

    // Fase 2: implement (ready → review, rol=developer)
    let impl_phase = &config.phases[1];
    assert_eq!(impl_phase.name, "implement");
    assert_eq!(impl_phase.from, "ready");
    assert_eq!(impl_phase.to, "review");
    assert_eq!(impl_phase.role, "developer");

    // Fase 3: validate (review → done, rol=reviewer)
    let val_phase = &config.phases[2];
    assert_eq!(val_phase.name, "validate");
    assert_eq!(val_phase.from, "review");
    assert_eq!(val_phase.to, "done");
    assert_eq!(val_phase.role, "reviewer");
}

#[test]
fn software_dev_initial_and_terminal_states() {
    // CA1: Estado inicial correcto, terminales correctos
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.states.initial, "draft");
    assert!(config.states.terminal.contains(&"done".to_string()));
    assert!(config.states.terminal.contains(&"failed".to_string()));
    // max_reject_cycles por defecto es 8
    for phase in &config.phases {
        assert_eq!(phase.max_reject_cycles, 8,
            "Cada fase debe tener max_reject_cycles=8 por defecto");
    }
}

#[test]
fn software_dev_on_reject_goes_to_previous_state() {
    // CA1: on_reject retorna al estado anterior
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    // implement: on_reject → ready (estado anterior)
    assert_eq!(config.phases[1].on_reject, "ready");

    // validate: on_reject → review (estado anterior)
    assert_eq!(config.phases[2].on_reject, "review");
}

#[test]
fn software_dev_task_format_compatible_with_v0x() {
    // CA2: task_format compatible con formato STORY-NNN de v0.x
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.task_format.id_pattern, r"STORY-\d+");
    assert!(config.task_format.section_markers.contains_key("status"),
        "Debe tener section_marker para 'status'");
    assert!(config.task_format.section_markers.contains_key("epic"),
        "Debe tener section_marker para 'epic'");
    assert!(config.task_format.section_markers.contains_key("descripcion"),
        "Debe tener section_marker para 'descripcion'");
    assert!(config.task_format.section_markers.contains_key("criterios"),
        "Debe tener section_marker para 'criterios'");
    assert!(config.task_format.section_markers.contains_key("dependencias"),
        "Debe tener section_marker para 'dependencias'");
    assert_eq!(config.task_format.dependency_marker, "Bloqueado por:");
}

#[test]
fn software_dev_system_prompts_include_format_instructions() {
    // CA3: Los system prompts incluyen instrucciones de formato
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    // Cada rol debe tener instrucciones [STATUS: X] y [REJECT: Y]
    for role in &config.roles {
        let prompt_lower = role.system_prompt.to_lowercase();
        assert!(
            prompt_lower.contains("[status:") || prompt_lower.contains("responde con"),
            "El rol '{}' debe incluir instrucción de formato STATUS",
            role.name
        );
        assert!(
            prompt_lower.contains("[reject:") || prompt_lower.contains("rechaz"),
            "El rol '{}' debe incluir instrucción de formato REJECT",
            role.name
        );
    }
}

#[test]
fn software_dev_developer_prompt_references_criterios_aceptacion() {
    // CA3: El prompt del developer referencia el formato de criterios de aceptación
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    let dev_role = config.roles.iter()
        .find(|r| r.name == "developer")
        .expect("El preset debe tener un rol 'developer'");

    assert!(
        dev_role.system_prompt.to_lowercase().contains("criterios de aceptación")
            || dev_role.system_prompt.to_lowercase().contains("ca"),
        "El system prompt del developer debe referenciar los criterios de aceptación"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// STORY-V10-014: Presets research y single-agent
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn research_preset_defines_two_phases() {
    // CA1: Preset research: 2 fases
    let preset = ResearchPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.phases.len(), 2, "Research debe tener 2 fases");

    // Fase 1: research (pending → draft, rol=researcher)
    let research_phase = &config.phases[0];
    assert_eq!(research_phase.name, "research");
    assert_eq!(research_phase.from, "pending");
    assert_eq!(research_phase.to, "draft");
    assert_eq!(research_phase.role, "researcher");

    // Fase 2: report (draft → done, rol=analyst)
    let report_phase = &config.phases[1];
    assert_eq!(report_phase.name, "report");
    assert_eq!(report_phase.from, "draft");
    assert_eq!(report_phase.to, "done");
    assert_eq!(report_phase.role, "analyst");
}

#[test]
fn research_preset_task_format_has_topic_depth_sources() {
    // CA1: task_format con campos topic, depth, sources
    let preset = ResearchPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.task_format.id_pattern, r"TASK-\d+");
    assert!(config.task_format.section_markers.contains_key("topic"),
        "Research debe tener campo 'topic'");
    assert!(config.task_format.section_markers.contains_key("depth"),
        "Research debe tener campo 'depth'");
    assert!(config.task_format.section_markers.contains_key("sources"),
        "Research debe tener campo 'sources'");
}

#[test]
fn single_agent_preset_defines_minimal_pipeline() {
    // CA2: Preset single-agent: 1 fase
    let preset = SingleAgentPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.phases.len(), 1, "Single-agent debe tener exactamente 1 fase");

    let phase = &config.phases[0];
    assert_eq!(phase.name, "execute");
    assert_eq!(phase.from, "pending");
    assert_eq!(phase.to, "done");
    assert_eq!(phase.role, "agent");
}

#[test]
fn single_agent_preset_task_format_is_minimal() {
    // CA2: task_format mínimo con description y priority
    let preset = SingleAgentPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.task_format.id_pattern, r"TASK-\d+");
    assert!(config.task_format.section_markers.contains_key("description"),
        "Single-agent debe tener campo 'description'");
    assert!(config.task_format.section_markers.contains_key("priority"),
        "Single-agent debe tener campo 'priority'");

    // Debe ser mínimo: no debe tener campos de software-dev
    assert!(!config.task_format.section_markers.contains_key("criterios"),
        "Single-agent NO debe tener campo de criterios");
    assert!(!config.task_format.section_markers.contains_key("epic"),
        "Single-agent NO debe tener campo de epic");
}

// ═══════════════════════════════════════════════════════════════════════
// CA3: Presets seleccionables desde el registry
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn preset_registry_contains_all_three_presets() {
    // CA3: El registry contiene los 3 presets
    let registry = PresetRegistry::new();

    let names: Vec<&str> = registry.preset_names();
    assert!(names.contains(&"software-dev"), "Registry debe contener software-dev");
    assert!(names.contains(&"research"), "Registry debe contener research");
    assert!(names.contains(&"single-agent"), "Registry debe contener single-agent");
}

#[test]
fn preset_registry_get_returns_correct_preset() {
    // CA3: Se puede obtener cada preset por nombre
    let registry = PresetRegistry::new();

    assert!(registry.get("software-dev").is_some(), "Debe existir software-dev");
    assert!(registry.get("research").is_some(), "Debe existir research");
    assert!(registry.get("single-agent").is_some(), "Debe existir single-agent");
    assert!(registry.get("nonexistent").is_none(), "Presets inexistentes retornan None");
}

#[test]
fn preset_registry_get_is_case_insensitive() {
    // CA3 (borde): El registry debería ser case-insensitive para usabilidad
    let registry = PresetRegistry::new();

    assert!(registry.get("SOFTWARE-DEV").is_some(), "software-dev en mayúsculas debe funcionar");
    assert!(registry.get("Software-Dev").is_some(), "software-dev mixto debe funcionar");
    assert!(registry.get("RESEARCH").is_some(), "research en mayúsculas debe funcionar");
    assert!(registry.get("Single-Agent").is_some(), "single-agent mixto debe funcionar");
}

#[test]
fn each_preset_provides_workflow_config() {
    // Verificar que cada preset implementa el trait Preset y devuelve WorkflowConfig
    let presets: Vec<Box<dyn Preset>> = vec![
        Box::new(SoftwareDevPreset::new()),
        Box::new(ResearchPreset::new()),
        Box::new(SingleAgentPreset::new()),
    ];

    for preset in &presets {
        let config = preset.workflow_config();

        // Todos deben tener estados definidos
        assert!(!config.states.initial.is_empty(), "El preset debe tener estado inicial");
        assert!(!config.states.terminal.is_empty(), "El preset debe tener estados terminales");

        // Todos deben tener al menos una fase
        assert!(!config.phases.is_empty(), "El preset debe tener al menos una fase");

        // Cada fase debe referenciar un rol definido
        let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();
        for phase in &config.phases {
            assert!(
                role_names.contains(&phase.role.as_str()),
                "La fase '{}' referencia el rol '{}' que no está definido en roles",
                phase.name,
                phase.role
            );
        }

        // task_format debe tener id_pattern no vacío
        assert!(!config.task_format.id_pattern.is_empty(),
            "El preset debe tener id_pattern definido");
    }
}

#[test]
fn software_dev_preset_roles_include_product_owner_developer_reviewer() {
    // CA1 + CA3: software-dev tiene los 3 roles esperados
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();
    assert!(role_names.contains(&"product_owner"), "software-dev debe tener product_owner");
    assert!(role_names.contains(&"developer"), "software-dev debe tener developer");
    assert!(role_names.contains(&"reviewer"), "software-dev debe tener reviewer");
    assert_eq!(config.roles.len(), 3, "software-dev debe tener exactamente 3 roles");
}

#[test]
fn research_preset_roles_include_researcher_and_analyst() {
    // CA1: research tiene researcher y analyst
    let preset = ResearchPreset::new();
    let config = preset.workflow_config();

    let role_names: Vec<&str> = config.roles.iter().map(|r| r.name.as_str()).collect();
    assert!(role_names.contains(&"researcher"), "research debe tener researcher");
    assert!(role_names.contains(&"analyst"), "research debe tener analyst");
    assert_eq!(config.roles.len(), 2, "research debe tener exactamente 2 roles");
}

#[test]
fn single_agent_preset_has_only_agent_role() {
    // CA2: single-agent tiene solo un rol
    let preset = SingleAgentPreset::new();
    let config = preset.workflow_config();

    assert_eq!(config.roles.len(), 1, "single-agent debe tener exactamente 1 rol");
    assert_eq!(config.roles[0].name, "agent");
}

#[test]
fn software_dev_plan_phase_is_decomposition() {
    // CA1: La fase "plan" es la fase de descomposición (from="_init_" o decomposition=true)
    let preset = SoftwareDevPreset::new();
    let config = preset.workflow_config();

    let plan_phase = &config.phases[0];
    assert_eq!(plan_phase.name, "plan");
    // La fase de plan debe ser identificable como fase de descomposición
    // (el Developer decide si usa from="_init_" o un flag decomposition)
    assert!(
        plan_phase.from == "_init_" || plan_phase.from == "draft",
        "La fase de plan debe ser la fase de descomposición (from=_init_ o from=draft)"
    );
}
*/
