//! Generación de prompts para cada agente del workflow.
//!
//! Los prompts son genéricos: indican al agente qué historia trabajar,
//! qué transición de estado se espera, y dónde documentar decisiones.
//! El skill del agente ya sabe *cómo* hacer su trabajo.

use crate::state::Status;

/// Contexto necesario para generar cualquier prompt.
pub struct PromptContext {
    /// ID de la historia (STORY-NNN).
    pub story_id: String,
    /// Ruta al directorio de historias (relativa, se incluye en el prompt).
    pub stories_dir: String,
    /// Ruta al directorio de decisiones (relativa, se incluye en el prompt).
    pub decisions_dir: String,
    /// Último motivo de rechazo (extraído del Activity Log), si existe.
    pub last_rejection: Option<String>,
    /// Transición esperada: de qué estado a qué estado.
    pub from: Status,
    pub to: Status,
}

impl PromptContext {
    /// Prompt para el Product Owner — Refinamiento (Draft → Ready).
    pub fn po_plan(&self) -> String {
        format!(
            "Refina {id}. Lee {dir}/{id}.md.\n\
             Valídala contra el Definition of Ready. Si está lista, muévela de {from} → {to}.\n\
             Si necesitas tomar decisiones, documéntalas en {dec}/.\n\
             Añade entrada en Activity Log con el formato: YYYY-MM-DD | PO | descripción.\n\
             NO preguntes nada al usuario. Trabaja de forma 100% autónoma.",
            id = self.story_id,
            dir = self.stories_dir,
            dec = self.decisions_dir,
            from = self.from,
            to = self.to,
        )
    }

    /// Prompt para el Product Owner — Validación (Business Review → Done).
    pub fn po_validate(&self) -> String {
        format!(
            "Valida {id} para Done. Lee {dir}/{id}.md.\n\
             Verifica que el valor de negocio se cumple. Si OK → {to}.\n\
             Si no: rechaza a In Review o In Progress según gravedad. Detalla el motivo.\n\
             Documenta la decisión en {dec}/.\n\
             NO preguntes. 100% autónomo.",
            id = self.story_id,
            dir = self.stories_dir,
            dec = self.decisions_dir,
            to = self.to,
        )
    }

    /// Prompt para QA — Escribir tests (Ready → Tests Ready).
    pub fn qa_tests(&self) -> String {
        format!(
            "Escribe tests para {id}. Lee {dir}/{id}.md.\n\
             Escribe los tests necesarios según los criterios de aceptación.\n\
             Mueve el estado de {from} → {to}.\n\
             Añade entrada en Activity Log: YYYY-MM-DD | QA | descripción.\n\
             Si necesitas crear placeholders en src/ para que los tests compilen, hazlo.\n\
             Documenta decisiones de diseño en {dec}/.\n\
             NO preguntes. 100% autónomo.",
            id = self.story_id,
            dir = self.stories_dir,
            dec = self.decisions_dir,
            from = self.from,
            to = self.to,
        )
    }

    /// Prompt para QA — Corregir tests (Tests Ready → Tests Ready).
    pub fn qa_fix_tests(&self) -> String {
        format!(
            "Corrige los tests de {id}. El Developer reportó problemas con los tests actuales.\n\
             Lee {dir}/{id}.md, especialmente el Activity Log para entender el feedback.\n\
             Corrige los tests. El estado se mantiene en {to}.\n\
             Añade entrada en Activity Log: YYYY-MM-DD | QA | descripción de la corrección.\n\
             NO preguntes. 100% autónomo.",
            id = self.story_id,
            dir = self.stories_dir,
            to = self.to,
        )
    }

    /// Prompt para Developer — Implementar (Tests Ready → In Review).
    pub fn dev_implement(&self) -> String {
        format!(
            "Implementa {id}. Lee {dir}/{id}.md.\n\
             Los tests ya existen (QA los escribió). Búscalos y haz que pasen.\n\
             Implementa en el código fuente. Ejecuta build + tests.\n\
             Mueve de {from} → {to}.\n\
             Añade entrada en Activity Log: YYYY-MM-DD | Dev | descripción.\n\
             Documenta decisiones de arquitectura en {dec}/.\n\
             NO preguntes. 100% autónomo.",
            id = self.story_id,
            dir = self.stories_dir,
            dec = self.decisions_dir,
            from = self.from,
            to = self.to,
        )
    }

    /// Prompt para Developer — Corregir tras rechazo (In Progress → In Review).
    pub fn dev_fix(&self) -> String {
        let rejection = self
            .last_rejection
            .as_deref()
            .unwrap_or("(revisa el Activity Log para los detalles)");
        format!(
            "Corrige {id}. El Reviewer/PO rechazó la implementación anterior:\n\
             \n  {rejection}\n\
             \n\
             Lee {dir}/{id}.md, especialmente el Activity Log, para el contexto completo.\n\
             Corrige la implementación. Ejecuta build + tests.\n\
             Mueve de {from} → {to}.\n\
             Añade entrada en Activity Log: YYYY-MM-DD | Dev | qué corregiste y por qué.\n\
             NO preguntes. 100% autónomo.",
            id = self.story_id,
            dir = self.stories_dir,
            from = self.from,
            to = self.to,
        )
    }

    /// Prompt para Reviewer — Revisión técnica (In Review → Business Review / In Progress).
    pub fn reviewer(&self) -> String {
        format!(
            "Revisa {id}. Lee {dir}/{id}.md.\n\
             Verifica el DoD técnico. Ejecuta cargo test, clippy, fmt.\n\
             Si TODO OK → Business Review.\n\
             Si algo falla → In Progress, con detalles CONCRETOS de archivo, línea y problema.\n\
             Añade entrada en Activity Log: YYYY-MM-DD | Reviewer | resultado.\n\
             Documenta observaciones en {dec}/.\n\
             NO preguntes. 100% autónomo.",
            id = self.story_id,
            dir = self.stories_dir,
            dec = self.decisions_dir,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> PromptContext {
        PromptContext {
            story_id: "STORY-042".into(),
            stories_dir: "product/stories".into(),
            decisions_dir: "product/decisions".into(),
            last_rejection: Some("Falta test para edge case CA3".into()),
            from: Status::InProgress,
            to: Status::InReview,
        }
    }

    #[test]
    fn po_plan_contains_story_id() {
        let plan_ctx = PromptContext {
            from: Status::Draft,
            to: Status::Ready,
            ..ctx()
        };
        let prompt = plan_ctx.po_plan();
        assert!(prompt.contains("STORY-042"));
        assert!(prompt.contains("Draft"));
        assert!(prompt.contains("Ready"));
    }

    #[test]
    fn dev_fix_includes_rejection() {
        let prompt = ctx().dev_fix();
        assert!(prompt.contains("Falta test para edge case CA3"));
        assert!(prompt.contains("In Progress"));
        assert!(prompt.contains("In Review"));
    }

    #[test]
    fn all_prompts_contain_story_id() {
        for prompt in [
            ctx().po_plan(),
            ctx().po_validate(),
            ctx().qa_tests(),
            ctx().qa_fix_tests(),
            ctx().dev_implement(),
            ctx().dev_fix(),
            ctx().reviewer(),
        ] {
            assert!(
                prompt.contains("STORY-042"),
                "prompt should mention STORY-042"
            );
        }
    }

    #[test]
    fn all_prompts_contain_no_preguntes() {
        for prompt in [
            ctx().po_plan(),
            ctx().po_validate(),
            ctx().qa_tests(),
            ctx().qa_fix_tests(),
            ctx().dev_implement(),
            ctx().dev_fix(),
            ctx().reviewer(),
        ] {
            assert!(
                prompt.contains("NO preguntes"),
                "prompt should tell agent not to ask user"
            );
        }
    }
}
