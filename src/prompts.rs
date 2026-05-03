//! Generación de prompts para cada agente del workflow.
//!
//! Los prompts son agnósticos al stack tecnológico del proyecto anfitrión:
//! los comandos de build, test, lint y formato se configuran en
//! `[stack]` de `.regista/config.toml`. Si no se definen, el prompt usa
//! instrucciones genéricas y el skill del agente interpreta el stack.

use crate::config::StackConfig;
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
    /// Comandos del stack tecnológico (build, test, lint, fmt, src_dir).
    pub stack: StackConfig,
}

impl StackConfig {
    /// Renderiza el bloque de stack para inyectar en el prompt.
    ///
    /// Si hay comandos definidos, los lista como instrucciones concretas.
    /// Si no hay ninguno, devuelve una instrucción genérica para que el
    /// skill del agente interprete el stack del proyecto.
    pub fn render(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if let Some(ref cmd) = self.build_command {
            parts.push(format!("- Compilación/build: `{cmd}`"));
        }
        if let Some(ref cmd) = self.test_command {
            parts.push(format!("- Tests: `{cmd}`"));
        }
        if let Some(ref cmd) = self.lint_command {
            parts.push(format!("- Linting: `{cmd}`"));
        }
        if let Some(ref cmd) = self.fmt_command {
            parts.push(format!("- Formato: `{cmd}`"));
        }

        if parts.is_empty() {
            "Compila/construye el proyecto, ejecuta los tests, y aplica \
             linting y formato según las convenciones del proyecto."
                .into()
        } else {
            format!(
                "Ejecuta los siguientes comandos para verificar tu trabajo:\n{}",
                parts.join("\n")
            )
        }
    }
}

impl PromptContext {
    /// Cabecera común a todos los prompts: acción + historia a leer.
    fn header(&self, action: &str) -> String {
        format!(
            "{action} {id}. Lee {dir}/{id}.md.",
            action = action,
            id = self.story_id,
            dir = self.stories_dir,
        )
    }

    /// Cierre estándar: Activity Log, decisiones, y orden de autonomía.
    fn suffix(&self, role_short: &str) -> String {
        format!(
            "Añade entrada en Activity Log: YYYY-MM-DD | {short} | descripción.\n\
             Documenta decisiones en {dec}/.\n\
             NO preguntes. 100% autónomo.",
            short = role_short,
            dec = self.decisions_dir,
        )
    }

    /// Prompt para el Product Owner — Refinamiento (Draft → Ready).
    pub fn po_plan(&self) -> String {
        format!(
            "{}\n\
             Valídala contra el Definition of Ready. Si está lista, muévela de {from} → {to}.\n\
             Si necesitas tomar decisiones, documéntalas en {dec}/.\n\
             {}",
            self.header("Refina"),
            self.suffix("PO"),
            from = self.from,
            to = self.to,
            dec = self.decisions_dir,
        )
    }

    /// Prompt para el Product Owner — Validación (Business Review → Done).
    pub fn po_validate(&self) -> String {
        format!(
            "{}\n\
             Verifica que el valor de negocio se cumple. Si OK → {to}.\n\
             Si no: rechaza a In Review o In Progress según gravedad. Detalla el motivo.\n\
             {}",
            self.header("Valida"),
            self.suffix("PO"),
            to = self.to,
        )
    }

    /// Prompt para QA — Escribir tests (Ready → Tests Ready).
    ///
    /// El QA SOLO escribe tests. No crea módulos, no implementa features,
    /// no crea fake providers ni infraestructura de testing. Eso es trabajo
    /// del Developer. El QA tampoco ejecuta build ni tests — el Developer
    /// lo hará cuando implemente.
    pub fn qa_tests(&self) -> String {
        let stack_block = self.stack.render();

        format!(
            "{}\n\
             Escribe SOLO tests unitarios que cubran CADA criterio de aceptación.\n\
             \n\
             REGLAS ESTRICTAS:\n\
             - NO crees módulos nuevos. Usa los módulos que ya existen.\n\
             - NO generes fake providers ni infrastructure de testing.\n\
             - NO implementes features ni lógica de negocio — eso es del Developer.\n\
             - NO ejecutes build ni tests — el Developer verificará que compilan.\n\
             - Si un test necesita un placeholder mínimo para compilar, créalo (solo el esqueleto).\n\
             - Prioriza tests unitarios sobre tests de integración.\n\
             - Si encuentras tests existentes que cubren los CAs, simplemente verifica que son suficientes.\n\
             \n\
             El Developer se encargará de ejecutar y validar con:\n\
             {}\n\
             Mueve el estado de {from} → {to}.\n\
             {}",
            self.header("Escribe tests para"),
            stack_block,
            self.suffix("QA"),
            from = self.from,
            to = self.to,
        )
    }

    /// Prompt para QA — Corregir tests (Tests Ready → Tests Ready).
    pub fn qa_fix_tests(&self) -> String {
        let stack_block = self.stack.render();
        format!(
            "{}\n\
             El Developer reportó problemas con los tests actuales.\n\
             Lee especialmente el Activity Log para entender el feedback.\n\
             Corrige los tests y verifica que compilan y pasan:\n\
             {}\n\
             El estado se mantiene en {to}.\n\
             {}",
            self.header("Corrige los tests de"),
            stack_block,
            self.suffix("QA"),
            to = self.to,
        )
    }

    /// Prompt para Developer — Implementar (Tests Ready → In Review).
    pub fn dev_implement(&self) -> String {
        let stack_block = self.stack.render();
        format!(
            "{}\n\
             Los tests ya existen (QA los escribió). Búscalos y haz que pasen.\n\
             Implementa en el código fuente.\n\
             \n\
             SI LOS TESTS NO COMPILAN O ESTÁN MAL ESCRITOS:\n\
             - NO los corrijas. Es trabajo del QA.\n\
             - NO avances el estado a InReview.\n\
             - Añade en el Activity Log qué test falla y por qué.\n\
             - El orquestador se encargará de pasar el turno al QA automáticamente.\n\
             \n\
             Si los tests compilan y tu implementación es correcta:\n\
             {}\n\
             Mueve de {from} → {to}.\n\
             {}",
            self.header("Implementa"),
            stack_block,
            self.suffix("Dev"),
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
        let stack_block = self.stack.render();
        format!(
            "{}\n\
             El Reviewer/PO rechazó la implementación anterior:\n\
             \n  {rejection}\n\
             \n\
             Lee especialmente el Activity Log para el contexto completo.\n\
             Corrige la implementación.\n\
             {}\n\
             Mueve de {from} → {to}.\n\
             {}",
            self.header("Corrige"),
            stack_block,
            self.suffix("Dev"),
            from = self.from,
            to = self.to,
        )
    }

    /// Prompt para Reviewer — Revisión técnica (In Review → Business Review / In Progress).
    pub fn reviewer(&self) -> String {
        let stack_block = self.stack.render();
        format!(
            "{}\n\
             Verifica el DoD técnico.\n\
             {}\n\
             Si TODO OK → Business Review.\n\
             Si algo falla → In Progress, con detalles CONCRETOS de archivo, línea y problema.\n\
             {}",
            self.header("Revisa"),
            stack_block,
            self.suffix("Reviewer"),
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
            stack: StackConfig::default(),
        }
    }

    // ── StackConfig::render ──────────────────────────────────────

    #[test]
    fn stack_render_empty_is_generic() {
        let stack = StackConfig::default();
        let out = stack.render();
        assert!(out.contains("Compila/construye"));
        assert!(out.contains("linting"));
        assert!(!out.contains('`'), "sin comandos = sin backticks");
    }

    #[test]
    fn stack_render_all_commands() {
        let stack = StackConfig {
            build_command: Some("npm run build".into()),
            test_command: Some("npm test".into()),
            lint_command: Some("eslint .".into()),
            fmt_command: Some("prettier --check .".into()),
            src_dir: Some("src/".into()),
        };
        let out = stack.render();
        assert!(out.contains("npm run build"));
        assert!(out.contains("npm test"));
        assert!(out.contains("eslint"));
        assert!(out.contains("prettier"));
        // src_dir no aparece en render() — solo se usa en qa_tests()
        assert!(!out.contains("src/"));
    }

    #[test]
    fn stack_render_partial_omits_missing() {
        let stack = StackConfig {
            test_command: Some("pytest".into()),
            ..Default::default()
        };
        let out = stack.render();
        assert!(out.contains("pytest"));
        assert!(!out.contains("build"), "sin build_command no se menciona");
        assert!(!out.contains("lint"), "sin lint_command no se menciona");
    }

    // ── Contenido de prompts ──────────────────────────────────────

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

    // ── Prompts con stack definido ────────────────────────────────

    fn ctx_with_stack() -> PromptContext {
        PromptContext {
            story_id: "STORY-007".into(),
            stories_dir: "docs/stories".into(),
            decisions_dir: "docs/decisions".into(),
            last_rejection: None,
            from: Status::TestsReady,
            to: Status::InReview,
            stack: StackConfig {
                build_command: Some("make".into()),
                test_command: Some("make test".into()),
                lint_command: Some("golangci-lint run".into()),
                fmt_command: Some("gofmt -l .".into()),
                src_dir: Some("pkg/".into()),
            },
        }
    }

    #[test]
    fn dev_implement_includes_stack_commands() {
        let prompt = ctx_with_stack().dev_implement();
        assert!(prompt.contains("make"));
        assert!(prompt.contains("make test"));
        assert!(prompt.contains("golangci-lint"));
        assert!(prompt.contains("gofmt"));
    }

    #[test]
    fn dev_fix_includes_stack_commands() {
        let mut c = ctx_with_stack();
        c.last_rejection = Some("bug en edge case".into());
        c.from = Status::InProgress;
        let prompt = c.dev_fix();
        assert!(prompt.contains("make"));
        assert!(prompt.contains("bug en edge case"));
        assert!(prompt.contains("In Progress"));
        assert!(prompt.contains("In Review"));
    }

    #[test]
    fn reviewer_includes_stack_commands() {
        let mut c = ctx_with_stack();
        c.from = Status::InReview;
        c.to = Status::BusinessReview;
        let prompt = c.reviewer();
        assert!(prompt.contains("make"));
        assert!(prompt.contains("make test"));
        assert!(prompt.contains("Business Review"));
    }

    #[test]
    fn qa_tests_incluye_reglas_estrictas() {
        let prompt = ctx_with_stack().qa_tests();
        assert!(prompt.contains("REGLAS ESTRICTAS"));
        assert!(prompt.contains("NO crees módulos"));
        assert!(prompt.contains("NO ejecutes build"));
    }

    #[test]
    fn qa_tests_menciona_placeholders_minimos() {
        // El nuevo prompt menciona placeholders mínimos siempre,
        // independientemente de si src_dir está configurado.
        let prompt = ctx_with_stack().qa_tests();
        assert!(prompt.contains("placeholder"));
        assert!(prompt.contains("esqueleto"));
    }

    #[test]
    fn qa_tests_includes_stack_commands() {
        let prompt = ctx_with_stack().qa_tests();
        assert!(prompt.contains("make"));
        assert!(prompt.contains("make test"));
        assert!(prompt.contains("golangci-lint"));
    }

    #[test]
    fn qa_fix_tests_includes_stack_commands() {
        let mut c = ctx_with_stack();
        c.from = Status::TestsReady;
        c.to = Status::TestsReady;
        let prompt = c.qa_fix_tests();
        assert!(prompt.contains("make"));
        assert!(prompt.contains("Activity Log"));
        assert!(prompt.contains("Tests Ready"));
    }

    #[test]
    fn qa_tests_sin_stack_no_menciona_comandos() {
        let c = ctx(); // stack default: todo None
        let prompt = c.qa_tests();
        assert!(prompt.contains("Compila/construye"));
        assert!(!prompt.contains('`'), "sin comandos = sin backticks");
    }

    #[test]
    fn po_plan_sin_stack_commands() {
        let mut c = ctx();
        c.from = Status::Draft;
        c.to = Status::Ready;
        let prompt = c.po_plan();
        assert!(!prompt.contains('`'), "PO plan no lleva comandos de stack");
        assert!(prompt.contains("Draft"));
        assert!(prompt.contains("Ready"));
    }

    #[test]
    fn po_validate_sin_stack_commands() {
        let mut c = ctx();
        c.from = Status::BusinessReview;
        c.to = Status::Done;
        let prompt = c.po_validate();
        assert!(
            !prompt.contains('`'),
            "PO validate no lleva comandos de stack"
        );
        assert!(prompt.contains("Done"));
        assert!(prompt.contains("In Review"));
        assert!(prompt.contains("In Progress"));
    }
}
