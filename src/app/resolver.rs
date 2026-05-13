//! Resolution logic for agent configuration.
//!
//! Resolves provider-specific paths (skill files) and model names from
//! `AgentsConfig`. Lives in `app/` because it depends on `infra/providers`.

use crate::config::AgentsConfig;
use crate::infra::providers;
use std::path::Path;

/// Resolves the instruction file (skill) path for a role.
pub fn skill_path(agents: &AgentsConfig, role: &str) -> String {
    let config = match role {
        "product_owner" => &agents.product_owner,
        "qa_engineer" => &agents.qa_engineer,
        "developer" => &agents.developer,
        "reviewer" => &agents.reviewer,
        _ => return String::new(),
    };

    if let Some(ref skill) = config.skill {
        return skill.clone();
    }

    let provider_name = agents.provider_for_role(role);
    let provider = providers::from_name(&provider_name)
        .expect("provider inválido en configuración — ejecuta 'regista validate' para diagnosticar");
    provider.instruction_dir(role)
}

/// Resolves the LLM model for a role with the priority:
/// 1. `AgentRoleConfig.model` of the role
/// 2. `AgentsConfig.model` (global)
/// 3. `model` field in the YAML frontmatter of the skill
/// 4. `"desconocido"`
///
/// Does not panic if `skill_path` does not exist — treats the error as fallback to step 3.
pub fn model(agents: &AgentsConfig, role: &str, skill_path: &Path) -> String {
    let role_config = match role {
        "product_owner" => Some(&agents.product_owner),
        "qa_engineer" => Some(&agents.qa_engineer),
        "developer" => Some(&agents.developer),
        "reviewer" => Some(&agents.reviewer),
        _ => None,
    };

    if let Some(config) = role_config {
        if let Some(ref model) = config.model {
            return model.clone();
        }
    }

    if let Some(ref model) = agents.model {
        return model.clone();
    }

    if let Some(model) = providers::read_yaml_field(skill_path, "model") {
        return model;
    }

    "desconocido".to_string()
}
