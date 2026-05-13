//! Sistema de templates para prompts con `{{variables}}`.
//!
//! Reemplaza los 7 prompts hardcodeados de v0.x. El sistema sustituye
//! variables como `{{task_id}}`, `{{task_fields.*}}`, `{{context.clave}}`
//! en tiempo de ejecución usando los campos de la `Task`.
//!
//! **Capa**: Dominio puro. Recibe `&Task` y `&HashMap<String,String>`.
//!           No importa `app/`, `infra/`, `cli/`, ni `config/`.

use crate::domain::task::Task;
use std::collections::HashMap;

/// Sustituye variables `{{...}}` en un template usando los datos de la task y el contexto.
///
/// Variables soportadas:
/// - `{{task_id}}` → `task.id`
/// - `{{task_status}}` → `task.fields["status"]`
/// - `{{task_fields.<campo>}}` → valor de un campo concreto
/// - `{{task_fields.*}}` → bullet list de todos los campos (`- campo: valor`)
/// - `{{last_rejection}}` → último rechazo del activity_log
/// - `{{blockers}}` → lista de dependencias
/// - `{{context.<clave>}}` → valor del contexto
/// - `{{role_name}}` → del contexto (para system prompts)
///
/// Las variables no encontradas se sustituyen por `"(no definido)"`.
pub fn render_template(template: &str, task: &Task, context: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    // Variables simples que no necesitan procesamiento complejo
    let replacements: &[(&str, fn(&Task) -> String)] = &[
        ("{{task_id}}", |t: &Task| t.id.clone()),
        (
            "{{task_status}}",
            |t: &Task| {
                t.fields
                    .get("status")
                    .cloned()
                    .unwrap_or_else(|| "(no definido)".to_string())
            },
        ),
        (
            "{{last_rejection}}",
            |t: &Task| {
                t.last_rejection()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "(sin rechazos previos)".to_string())
            },
        ),
    ];

    for (pattern, getter) in replacements {
        result = result.replace(pattern, &getter(task));
    }

    // {{blockers}} → bullet list
    let blockers_str = if task.blockers.is_empty() {
        "(sin dependencias)".to_string()
    } else {
        task.blockers
            .iter()
            .map(|b| format!("- {}", b))
            .collect::<Vec<_>>()
            .join("\n")
    };
    result = result.replace("{{blockers}}", &blockers_str);

    // {{task_fields.*}} → bullet list de todos los campos
    let fields_bullet = {
        let mut sorted_fields: Vec<_> = task.fields.iter().collect();
        sorted_fields.sort_by(|a, b| a.0.cmp(b.0));
        if sorted_fields.is_empty() {
            "(sin campos adicionales)".to_string()
        } else {
            sorted_fields
                .iter()
                .map(|(k, v)| format!("- {}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    result = result.replace("{{task_fields.*}}", &fields_bullet);

    // {{task_fields.<campo>}} → valor de un campo
    {
        let mut search_start = 0;
        while let Some(start) = result[search_start..].find("{{task_fields.") {
            let abs_start = search_start + start;
            let after_open = abs_start + "{{task_fields.".len();
            if let Some(end) = result[after_open..].find("}}") {
                let field_name = &result[after_open..after_open + end];
                let full_pattern = format!("{{{{task_fields.{}}}}}", field_name);
                let replacement = task
                    .fields
                    .get(field_name)
                    .cloned()
                    .unwrap_or_else(|| "(no definido)".to_string());
                result = result.replace(&full_pattern, &replacement);
                search_start = abs_start + replacement.len();
            } else {
                // `{{task_fields.` sin cierre `}}`
                break;
            }
        }
    }

    // {{context.<clave>}} → valor del contexto
    {
        let mut search_start = 0;
        while let Some(start) = result[search_start..].find("{{context.") {
            let abs_start = search_start + start;
            let after_open = abs_start + "{{context.".len();
            if let Some(end) = result[after_open..].find("}}") {
                let key = &result[after_open..after_open + end];
                let full_pattern = format!("{{{{context.{}}}}}", key);
                let replacement = context
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| "(no definido)".to_string());
                result = result.replace(&full_pattern, &replacement);
                search_start = abs_start + replacement.len();
            } else {
                break;
            }
        }
    }

    // {{role_name}} → del contexto
    if let Some(role_name) = context.get("role_name") {
        result = result.replace("{{role_name}}", role_name);
    } else if result.contains("{{role_name}}") {
        result = result.replace("{{role_name}}", "(no definido)");
    }

    // Variables de task no sustituidas en contextos sin task
    if result.contains("{{task_id}}") {
        result = result.replace("{{task_id}}", "(sin tarea)");
    }
    if result.contains("{{task_status}}") {
        result = result.replace("{{task_status}}", "(sin tarea)");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::ActivityLogEntry;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // ── Helper ───────────────────────────────────────────────────────

    fn make_task() -> Task {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), "pending".to_string());
        fields.insert("priority".to_string(), "high".to_string());
        fields.insert("topic".to_string(), "IA generativa".to_string());
        fields.insert("effort".to_string(), "5".to_string());

        Task {
            id: "TASK-005".to_string(),
            path: PathBuf::from("tasks/TASK-005.md"),
            fields,
            blockers: vec!["TASK-002".to_string(), "TASK-003".to_string()],
            activity_log: vec![ActivityLogEntry {
                date: "2026-05-08".to_string(),
                actor: "reviewer".to_string(),
                description: "rechazado: tests no compilan".to_string(),
            }],
            raw_content: String::new(),
        }
    }

    fn make_context() -> HashMap<String, String> {
        let mut ctx = HashMap::new();
        ctx.insert("fecha_limite".to_string(), "2026-06-01".to_string());
        ctx.insert("asignado_a".to_string(), "Alice".to_string());
        ctx
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: {{task_id}} y {{task_status}}
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_task_id_and_status() {
        let result = render_template(
            "Procesa {{task_id}} con estado {{task_status}}",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "Procesa TASK-005 con estado pending");
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: {{last_rejection}}
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_last_rejection() {
        let result = render_template("Corrige: {{last_rejection}}", &make_task(), &HashMap::new());
        assert!(result.contains("rechazado: tests no compilan"));
    }

    #[test]
    fn last_rejection_without_rejections() {
        let mut task = make_task();
        task.activity_log = vec![ActivityLogEntry {
            date: "2026-05-09".to_string(),
            actor: "dev".to_string(),
            description: "implementado".to_string(),
        }];
        let result = render_template("{{last_rejection}}", &task, &HashMap::new());
        assert_eq!(result, "(sin rechazos previos)");
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: {{blockers}}
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_blockers_as_bullet_list() {
        let result = render_template("Depende de:\n{{blockers}}", &make_task(), &HashMap::new());
        assert!(result.contains("- TASK-002"));
        assert!(result.contains("- TASK-003"));
    }

    #[test]
    fn blockers_empty_returns_placeholder() {
        let mut task = make_task();
        task.blockers = vec![];
        let result = render_template("{{blockers}}", &task, &HashMap::new());
        assert_eq!(result, "(sin dependencias)");
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: {{task_fields.<campo>}} y {{task_fields.*}}
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_specific_field() {
        let result = render_template(
            "Prioridad: {{task_fields.priority}}",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "Prioridad: high");
    }

    #[test]
    fn renders_multiple_specific_fields() {
        let result = render_template(
            "Tema: {{task_fields.topic}} — Esfuerzo: {{task_fields.effort}} días",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "Tema: IA generativa — Esfuerzo: 5 días");
    }

    #[test]
    fn renders_fields_bullet_all() {
        let result = render_template(
            "Campos:\n{{task_fields.*}}",
            &make_task(),
            &HashMap::new(),
        );
        assert!(result.contains("- effort: 5"));
        assert!(result.contains("- priority: high"));
        assert!(result.contains("- status: pending"));
        assert!(result.contains("- topic: IA generativa"));
    }

    #[test]
    fn fields_bullet_empty_when_no_fields() {
        let mut task = make_task();
        task.fields.clear();
        let result = render_template("{{task_fields.*}}", &task, &HashMap::new());
        assert_eq!(result, "(sin campos adicionales)");
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: {{context.<clave>}}
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_context_variables() {
        let result = render_template(
            "Deadline: {{context.fecha_limite}} — Asignado: {{context.asignado_a}}",
            &make_task(),
            &make_context(),
        );
        assert_eq!(result, "Deadline: 2026-06-01 — Asignado: Alice");
    }

    // ═══════════════════════════════════════════════════════════════
    // CA3: Variables no definidas → "(no definido)"
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn unknown_field_returns_placeholder() {
        let result = render_template(
            "{{task_fields.inexistente}}",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "(no definido)");
    }

    #[test]
    fn unknown_context_key_returns_placeholder() {
        let result = render_template(
            "{{context.inexistente}}",
            &make_task(),
            &make_context(),
        );
        assert_eq!(result, "(no definido)");
    }

    #[test]
    fn template_without_variables_returns_unchanged() {
        let result = render_template("Este prompt no tiene variables", &make_task(), &HashMap::new());
        assert_eq!(result, "Este prompt no tiene variables");
    }

    // ═══════════════════════════════════════════════════════════════
    // CA2: System prompt con {{role_name}}
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_role_name_from_context() {
        let mut ctx = HashMap::new();
        ctx.insert("role_name".to_string(), "QA Engineer".to_string());
        let result = render_template(
            "Eres {{role_name}}. Procesa {{task_id}}",
            &make_task(),
            &ctx,
        );
        assert_eq!(result, "Eres QA Engineer. Procesa TASK-005");
    }

    #[test]
    fn renders_system_prompt_with_task_fields() {
        let mut ctx = HashMap::new();
        ctx.insert("role_name".to_string(), "Developer".to_string());
        let result = render_template(
            "Rol: {{role_name}}. Prioridad: {{task_fields.priority}}",
            &make_task(),
            &ctx,
        );
        assert!(result.contains("Rol: Developer"));
        assert!(result.contains("Prioridad: high"));
    }

    #[test]
    fn renders_system_prompt_without_task() {
        // Sin task (usando un Task vacío)
        let empty_task = Task {
            id: String::new(),
            path: PathBuf::new(),
            fields: HashMap::new(),
            blockers: vec![],
            activity_log: vec![],
            raw_content: String::new(),
        };
        let mut ctx = HashMap::new();
        ctx.insert("role_name".to_string(), "Reviewer".to_string());
        let result = render_template("Eres {{role_name}}", &empty_task, &ctx);
        assert_eq!(result, "Eres Reviewer");
    }

    // ═══════════════════════════════════════════════════════════════
    // Combinación de variables
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn renders_all_variables_combined() {
        let template = "Tarea: {{task_id}} [{{task_status}}]\nPrioridad: {{task_fields.priority}}\nBloqueada: {{blockers}}\nRechazo: {{last_rejection}}\nDeadline: {{context.fecha_limite}}";
        let result = render_template(template, &make_task(), &make_context());
        assert!(result.contains("Tarea: TASK-005 [pending]"));
        assert!(result.contains("Prioridad: high"));
        assert!(result.contains("TASK-002"));
        assert!(result.contains("rechazado"));
        assert!(result.contains("Deadline: 2026-06-01"));
    }

    // ═══════════════════════════════════════════════════════════════
    // Pureza: render_template es determinista y sin efectos secundarios
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn render_template_is_deterministic() {
        let task = make_task();
        let ctx = make_context();
        let template = "{{task_id}} {{task_status}}";
        let first = render_template(template, &task, &ctx);
        for _ in 0..5 {
            assert_eq!(render_template(template, &task, &ctx), first);
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Integridad cross-layer
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn module_does_not_import_other_crate_layers() {
        let source = std::fs::read_to_string(file!()).unwrap();
        let bad_imports: Vec<&str> = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("use crate::app::")
                    || trimmed.starts_with("use crate::infra::")
                    || trimmed.starts_with("use crate::cli::")
                    || trimmed.starts_with("use crate::config::")
            })
            .collect();

        assert!(
            bad_imports.is_empty(),
            "domain/templates.rs contiene imports no permitidos:\n{}",
            bad_imports.join("\n")
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // Edge cases adicionales
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn unclosed_brace_after_task_fields_is_literal() {
        // "{{task_fields.priority" sin cierre "}}" no debería interpretarse
        let result = render_template(
            "Valor: {{task_fields.priority y más texto sin cerrar",
            &make_task(),
            &HashMap::new(),
        );
        // El texto completo debe conservarse literal
        assert!(result.contains("{{task_fields.priority"), "Unclosed braces should be preserved: {result}");
    }

    #[test]
    fn unknown_variable_outside_known_patterns() {
        let result = render_template(
            "{{variable_inventada}}",
            &make_task(),
            &HashMap::new(),
        );
        // Las variables no reconocidas se quedan como texto literal
        // (el sistema actual solo procesa patrones conocidos: task_id, task_status,
        //  task_fields, last_rejection, blockers, context, role_name)
        assert_eq!(result, "{{variable_inventada}}", "Unknown variable pattern should be kept literal");
    }

    #[test]
    fn unclosed_brace_without_final_brackets() {
        // "{{task_id" sin "}}" es texto literal
        let result = render_template(
            "Empieza {{task_id y aquí sigue",
            &make_task(),
            &HashMap::new(),
        );
        assert!(result.contains("{{task_id"), "Unclosed {{task_id should remain as literal: {result}");
    }

    #[test]
    fn empty_braces_remain_literal() {
        // "{{}}" - braces vacías
        let result = render_template(
            "Esto es {{}} vacío",
            &make_task(),
            &HashMap::new(),
        );
        assert!(result.contains("{{}}"), "Empty braces should remain as literal: {result}");
    }

    #[test]
    fn task_fields_with_empty_key() {
        // "{{task_fields.}}" — punto sin nombre de campo
        let result = render_template(
            "{{task_fields.}}",
            &make_task(),
            &HashMap::new(),
        );
        // El sistema busca el campo "" (vacío), que no existe → "(no definido)"
        assert_eq!(result, "(no definido)");
    }

    #[test]
    fn context_with_empty_key() {
        // "{{context.}}" — punto sin nombre de clave
        let result = render_template(
            "{{context.}}",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "(no definido)");
    }

    #[test]
    fn role_name_without_context_returns_placeholder() {
        // Sin role_name en el context
        let result = render_template(
            "Eres {{role_name}}",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "Eres (no definido)");
    }

    #[test]
    fn task_fields_bullet_is_sorted_alphabetically() {
        let result = render_template(
            "{{task_fields.*}}",
            &make_task(),
            &HashMap::new(),
        );
        let lines: Vec<&str> = result.lines().collect();
        // Verificar orden alfabético: effort, priority, status, topic
        // (aunque effort y priority podrían salir en distinto orden, la idea es que estén ordenados)
        let effort_pos = lines.iter().position(|l| l.starts_with("- effort:")).unwrap_or(usize::MAX);
        let priority_pos = lines.iter().position(|l| l.starts_with("- priority:")).unwrap_or(usize::MAX);
        let status_pos = lines.iter().position(|l| l.starts_with("- status:")).unwrap_or(usize::MAX);
        let topic_pos = lines.iter().position(|l| l.starts_with("- topic:")).unwrap_or(usize::MAX);
        assert!(effort_pos < priority_pos, "effort should come before priority alphabetically");
        assert!(priority_pos < status_pos, "priority should come before status alphabetically");
        assert!(status_pos < topic_pos, "status should come before topic alphabetically");
    }

    #[test]
    fn renders_task_id_when_present() {
        let result = render_template(
            "ID: {{task_id}}",
            &make_task(),
            &HashMap::new(),
        );
        assert_eq!(result, "ID: TASK-005");
    }

    #[test]
    fn system_prompt_without_variables_returns_unchanged() {
        let mut ctx = HashMap::new();
        ctx.insert("role_name".to_string(), "Developer".to_string());
        let result = render_template(
            "Eres un desarrollador senior. Escribe código limpio.",
            &make_task(),
            &ctx,
        );
        assert_eq!(result, "Eres un desarrollador senior. Escribe código limpio.");
    }

    #[test]
    fn renders_task_status_from_fields() {
        let mut task = make_task();
        task.fields.insert("status".to_string(), "in_progress".to_string());
        let result = render_template("Estado: {{task_status}}", &task, &HashMap::new());
        assert_eq!(result, "Estado: in_progress");
    }

    #[test]
    fn task_status_when_status_field_missing() {
        let mut task = make_task();
        task.fields.remove("status");
        let result = render_template("Estado: {{task_status}}", &task, &HashMap::new());
        assert_eq!(result, "Estado: (no definido)");
    }

    #[test]
    fn task_fields_bullet_includes_all_fields_except_nothing() {
        let result = render_template(
            "{{task_fields.*}}",
            &make_task(),
            &HashMap::new(),
        );
        assert!(result.contains("- effort: 5"));
        assert!(result.contains("- priority: high"));
        assert!(result.contains("- status: pending"));
        assert!(result.contains("- topic: IA generativa"));
        // Exactamente 4 campos
        assert_eq!(result.lines().count(), 4);
    }
}
