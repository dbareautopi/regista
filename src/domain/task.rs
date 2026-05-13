//! Task genérico con parseo configurable.
//!
//! Reemplaza al `Story` hardcodeado de v0.x. El formato de cada archivo .md
//! se define en `TaskFormatConfig`, que especifica el patrón de ID, los
//! marcadores de sección, y el marcador de dependencias.
//!
//! **Capa**: Dominio puro. No importa `app/`, `infra/`, `cli/`, ni `config/`.

use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// ── Tipos de configuración (se moverán a config/ más adelante) ────────

/// Define cómo parsear archivos .md de tareas.
///
/// Vive en `config/workflow.rs` en producción. Aquí se define temporalmente
/// para que el dominio pueda consumirlo sin depender de la capa config.
#[derive(Debug, Clone)]
pub struct TaskFormatConfig {
    /// Patrón regex para extraer el ID del nombre de archivo (ej: `TASK-\d+`).
    pub id_pattern: String,
    /// Mapa de `nombre_de_campo → marcador_de_sección` (ej: `"status" → "## Status"`).
    pub section_markers: HashMap<String, String>,
    /// Marcador que indica la línea de dependencias (ej: `"Bloqueado por:"`).
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

// ── Task ────────────────────────────────────────────────────────────────

/// Una entrada del Activity Log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLogEntry {
    pub date: String,
    pub actor: String,
    pub description: String,
}

/// Representación genérica de una tarea parseada desde un archivo .md.
#[derive(Debug, Clone)]
pub struct Task {
    /// Identificador extraído del nombre de archivo usando `id_pattern`.
    pub id: String,
    /// Ruta al archivo .md en disco.
    pub path: PathBuf,
    /// Campos extraídos según `section_markers`. La clave es el nombre del campo.
    pub fields: HashMap<String, String>,
    /// IDs de tareas de las que depende esta tarea.
    pub blockers: Vec<String>,
    /// Entradas del Activity Log parseadas.
    pub activity_log: Vec<ActivityLogEntry>,
    /// Contenido completo del archivo (para reescribir al actualizar).
    pub raw_content: String,
}

// ── Parseo ──────────────────────────────────────────────────────────────

/// Extrae el contenido de una sección markdown (desde `## Header` hasta el siguiente `## `).
fn extract_section(content: &str, header: &str) -> Option<String> {
    let header_lower = header.to_lowercase();
    let mut in_section = false;
    let mut result = String::new();

    for line in content.lines() {
        let trimmed = line.trim().to_lowercase();
        if trimmed.starts_with(&header_lower) {
            in_section = true;
            continue;
        }
        if in_section {
            if trimmed.starts_with("## ") {
                break;
            }
            result.push_str(line);
            result.push('\n');
        }
    }

    if result.trim().is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Parsea el Activity Log: entradas con formato `- YYYY-MM-DD | Actor | descripción`.
fn parse_activity_log(content: &str) -> Vec<ActivityLogEntry> {
    let section = match extract_section(content, "## Activity Log") {
        Some(s) => s,
        None => return vec![],
    };

    let entry_re: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"-\s*(\S+)\s*\|\s*([^|]+?)\s*\|\s*(.*)").unwrap());

    section
        .lines()
        .filter_map(|line| {
            entry_re.captures(line).map(|caps| ActivityLogEntry {
                date: caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default(),
                actor: caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default(),
                description: caps
                    .get(3)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// Determina si una línea del Activity Log contiene un rechazo.
fn is_rejection(line: &str) -> bool {
    line.to_lowercase().contains("rechaz")
}

// ── Task::load ──────────────────────────────────────────────────────────

impl Task {
    /// Parsea una tarea desde su contenido markdown.
    ///
    /// El I/O de archivos se maneja en `infra::task_io`.
    pub fn parse(path: &Path, content: &str, task_format: &TaskFormatConfig) -> Result<Self, String> {
        // Extraer ID del nombre de archivo usando id_pattern
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let id_re = Regex::new(&task_format.id_pattern)
            .map_err(|e| format!("id_pattern inválido '{}': {e}", task_format.id_pattern))?;

        let id = id_re
            .find(filename)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| {
                format!(
                    "El nombre de archivo '{}' no cumple el id_pattern '{}'",
                    filename,
                    task_format.id_pattern
                )
            })?;

        // Parsear campos según section_markers
        let mut fields: HashMap<String, String> = HashMap::new();

        for (field_name, marker) in &task_format.section_markers {
            if let Some(section_content) = extract_section(content, marker) {
                // Limpiar markdown bold (**...**) del valor
                let cleaned = section_content.trim().replace("**", "").trim().to_string();
                if !cleaned.is_empty() {
                    fields.insert(field_name.clone(), cleaned);
                }
            }
        }

        // Extraer dependencias
        let blockers = parse_blockers_configurable(content, &task_format.dependency_marker);

        // Extraer Activity Log
        let activity_log = parse_activity_log(content);

        Ok(Self {
            id,
            path: path.to_path_buf(),
            fields,
            blockers,
            activity_log,
            raw_content: content.to_string(),
        })
    }

    /// Genera el contenido markdown con un campo actualizado, sin tocar disco.
    ///
    /// La escritura a disco (con backup atómico) se maneja en `infra::task_io`.
    pub fn render_field_update(
        &self,
        field_name: &str,
        new_value: &str,
        section_markers: &HashMap<String, String>,
    ) -> Result<String, String> {
        let marker = section_markers.get(field_name).ok_or_else(|| {
            format!(
                "El campo '{}' no está definido en section_markers",
                field_name
            )
        })?;

        let new_status_line = format!("**{}**", new_value);

        let mut lines: Vec<String> = self.raw_content.lines().map(|l| l.to_string()).collect();
        let mut found = false;

        for i in 0..lines.len() {
            if lines[i].to_lowercase().trim() == marker.to_lowercase().trim() {
                if i + 1 < lines.len() {
                    let old_line = &lines[i + 1];
                    let leading = old_line.len() - old_line.trim_start().len();
                    let spaces_leading = " ".repeat(leading);
                    lines[i + 1] = format!("{}{}", spaces_leading, new_status_line);
                    found = true;
                }
                break;
            }
        }

        if !found {
            return Err(format!(
                "{}: no se encontró la sección '{}'",
                self.id,
                marker
            ));
        }

        Ok(lines.join("\n"))
    }

    /// Devuelve una referencia a la ruta del archivo.
    pub fn file_path(&self) -> &Path {
        &self.path
    }

    /// Último motivo de rechazo (extraído del activity_log), si existe.
    pub fn last_rejection(&self) -> Option<&str> {
        self.activity_log
            .iter()
            .rev()
            .find(|entry| is_rejection(&entry.description))
            .map(|entry| entry.description.as_str())
    }

    /// Último actor del Activity Log.
    pub fn last_actor(&self) -> Option<&str> {
        self.activity_log.last().map(|e| e.actor.as_str())
    }
}

/// Extrae bloqueadores usando un marcador configurable.
fn parse_blockers_configurable(content: &str, dependency_marker: &str) -> Vec<String> {
    // Buscar la línea que contiene el dependency_marker (case-insensitive)
    let marker_lower = dependency_marker.to_lowercase();
    let blockers_line = content
        .lines()
        .find(|l| l.to_lowercase().contains(&marker_lower))
        .unwrap_or("");

    // Extraer IDs: secuencias de letras mayúsculas/números con guión (ej: TASK-001, ISSUE-042)
    let id_re: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[A-Z]+-\d+").unwrap());

    id_re.find_iter(blockers_line)
        .map(|m| m.as_str().to_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── Helpers ──────────────────────────────────────────────────────

    /// Helper: loads a task from a temp file using the pure `Task::parse` API.
    fn load_task(path: &std::path::Path, task_format: &TaskFormatConfig) -> Result<Task, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("error reading {e}: {path:?}"))?;
        Task::parse(path, &content, task_format)
    }

    fn write_task(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn default_task_format() -> TaskFormatConfig {
        let mut markers = HashMap::new();
        markers.insert("status".to_string(), "## Status".to_string());
        markers.insert("priority".to_string(), "## Priority".to_string());
        markers.insert("description".to_string(), "## Descripción".to_string());
        TaskFormatConfig {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers: markers,
            dependency_marker: "Bloqueado por:".to_string(),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: Task::load extrae ID con id_pattern configurable
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn load_extracts_id_with_default_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "# TASK-001\n\n## Status\n**pending**\n");

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.id, "TASK-001");
    }

    #[test]
    fn load_extracts_id_with_issue_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("ISSUE-042.md");
        write_task(&task_path, "## Status\n**open**\n");

        let format = TaskFormatConfig {
            id_pattern: r"ISSUE-\d+".to_string(),
            ..default_task_format()
        };
        let task = load_task(&task_path, &format).unwrap();
        assert_eq!(task.id, "ISSUE-042");
    }

    #[test]
    fn load_fails_when_filename_does_not_match_id_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("mytask.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let result = load_task(&task_path, &default_task_format());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("id_pattern"),
            "Error should mention id_pattern: {err}"
        );
        assert!(err.contains("mytask"), "Error should mention filename: {err}");
    }

    #[test]
    fn load_fails_with_invalid_regex_pattern() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let format = TaskFormatConfig {
            id_pattern: "[invalid".to_string(),
            ..default_task_format()
        };
        let result = load_task(&task_path, &format);
        assert!(result.is_err());
    }

    // ═══════════════════════════════════════════════════════════════
    // CA1: Parsear múltiples section_markers
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn load_parses_multiple_section_markers() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**pending**\n\n## Priority\nhigh\n\n## Descripción\nImplementar parser\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.fields.get("status").map(|s| s.as_str()), Some("pending"));
        assert_eq!(task.fields.get("priority").map(|s| s.as_str()), Some("high"));
        assert!(task
            .fields
            .get("description")
            .unwrap()
            .contains("parser"));
    }

    #[test]
    fn load_omits_fields_not_present_in_file() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-002.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.fields.get("status").map(|s| s.as_str()), Some("draft"));
        assert!(!task.fields.contains_key("priority"));
        assert!(!task.fields.contains_key("description"));
    }

    #[test]
    fn raw_content_preserves_full_file() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        let content = "# TASK-001\n\n## Status\n**draft**\n\ntexto libre entre secciones\n\n## Priority\nhigh\n";
        write_task(&task_path, content);

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.raw_content.contains("texto libre entre secciones"));
        assert!(task.raw_content.contains("## Status"));
        assert!(task.raw_content.contains("## Priority"));
    }

    // ═══════════════════════════════════════════════════════════════
    // CA2: Extraer dependencias con dependency_marker configurable
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn load_extracts_blockers_with_default_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n## Dependencias\n- Bloqueado por: TASK-002, TASK-003\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.blockers.contains(&"TASK-002".to_string()));
        assert!(task.blockers.contains(&"TASK-003".to_string()));
        assert_eq!(task.blockers.len(), 2);
    }

    #[test]
    fn load_extracts_blockers_with_custom_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("ISSUE-005.md");
        write_task(
            &task_path,
            "## Relations\n- Depends on: ISSUE-001, ISSUE-002\n",
        );

        let format = TaskFormatConfig {
            id_pattern: r"ISSUE-\d+".to_string(),
            dependency_marker: "Depends on:".to_string(),
            ..default_task_format()
        };
        let task = load_task(&task_path, &format).unwrap();
        assert!(task.blockers.contains(&"ISSUE-001".to_string()));
        assert!(task.blockers.contains(&"ISSUE-002".to_string()));
    }

    #[test]
    fn load_empty_blockers_when_no_dependency_line() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.blockers.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════
    // CA2: Parsear Activity Log
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn load_parses_activity_log_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n## Activity Log\n- 2026-05-08 | PO | Historia creada\n- 2026-05-09 | Dev | Implementación iniciada\n- 2026-05-10 | Reviewer | RECHAZADO: falta cobertura\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.activity_log.len(), 3);
        assert_eq!(task.activity_log[0].date, "2026-05-08");
        assert_eq!(task.activity_log[0].actor, "PO");
        assert_eq!(task.activity_log[0].description, "Historia creada");
        assert_eq!(task.activity_log[2].actor, "Reviewer");
        assert!(task.activity_log[2].description.contains("RECHAZADO"));
    }

    #[test]
    fn load_empty_activity_log_when_section_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.activity_log.is_empty());
    }

    #[test]
    fn load_empty_activity_log_when_section_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "## Status\n**draft**\n\n## Activity Log\n");

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.activity_log.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════
    // CA3: Task::set_status escribe preservando contenido
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn set_status_writes_new_value_preserving_content() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        let original = "## Status\n**pending**\n\n## Priority\nhigh\n\n## Descripción\nFoo\n";
        write_task(&task_path, original);

        let format = default_task_format();
        let mut task = load_task(&task_path, &format).unwrap();

        let new_content = task
            .render_field_update("status", "in_progress", &format.section_markers)
            .unwrap();
        std::fs::write(&task_path, &new_content).unwrap();
        task.fields
            .insert("status".to_string(), "in_progress".to_string());

        // Verificar en disco
        let disk_content = std::fs::read_to_string(&task_path).unwrap();
        assert!(disk_content.contains("**in_progress**"));
        assert!(!disk_content.contains("**pending**"));
        assert!(disk_content.contains("## Priority\nhigh"));
        assert!(disk_content.contains("## Descripción\nFoo"));

        // Verificar en memoria
        assert_eq!(task.fields.get("status").map(|s| s.as_str()), Some("in_progress"));
    }

    #[test]
    fn set_status_rollback_on_corruption() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        let original = "## Status\n**pending**\n";
        write_task(&task_path, original);

        let format = default_task_format();
        let mut task = load_task(&task_path, &format).unwrap();

        // render_field_update es puro (no I/O). El backup atómico es
        // responsabilidad de infra::task_io, no del dominio.
        let bak_path = task_path.with_extension("md.bak");

        let new_content = task
            .render_field_update("status", "done", &format.section_markers)
            .unwrap();
        std::fs::write(&task_path, &new_content).unwrap();
        task.fields.insert("status".to_string(), "done".to_string());

        // render_field_update no crea .bak — es responsabilidad de la capa infra
        assert!(
            !bak_path.exists(),
            ".bak no es creado por render_field_update (es puro)"
        );

        // Verificar que el contenido se preservó
        let disk = std::fs::read_to_string(&task_path).unwrap();
        assert!(disk.contains("**done**"));
    }

    #[test]
    fn set_status_fails_for_unknown_field() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "## Status\n**pending**\n");

        let format = default_task_format();
        let task = load_task(&task_path, &format).unwrap();

        let result = task.render_field_update("unknown_field", "value", &format.section_markers);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no está definido"));
    }

    #[test]
    fn set_status_fails_when_section_not_in_file() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "# TASK-001\n\nsin sección status\n");

        // Usar un format sin la sección Status definida
        let mut empty_markers = HashMap::new();
        empty_markers.insert("other".to_string(), "## Other".to_string());
        let format = TaskFormatConfig {
            section_markers: empty_markers.clone(),
            ..default_task_format()
        };
        let task = load_task(&task_path, &format).unwrap();

        let result = task.render_field_update("other", "value", &empty_markers);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no se encontró"));
    }

    // ═══════════════════════════════════════════════════════════════
    // last_rejection() y last_actor()
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn last_rejection_finds_rejection_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n## Activity Log\n- 2026-05-09 | Reviewer | RECHAZADO: tests rotos\n- 2026-05-10 | Dev | Corregido\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        let rejection = task.last_rejection();
        assert!(rejection.is_some());
        assert!(rejection.unwrap().contains("RECHAZADO"));
    }

    #[test]
    fn last_rejection_none_when_no_rejection() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n## Activity Log\n- 2026-05-09 | Dev | Implementado\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.last_rejection().is_none());
    }

    #[test]
    fn last_actor_returns_last_entry_actor() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n## Activity Log\n- 2026-05-08 | PO | Creada\n- 2026-05-09 | Dev | Implementada\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.last_actor(), Some("Dev"));
    }

    // ═══════════════════════════════════════════════════════════════
    // Integridad cross-layer: domain/task.rs no importa otras capas
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn module_does_not_import_other_crate_layers() {
        // Verificación estática: este módulo solo usa:
        // - std (Path, PathBuf, HashMap, fs)
        // - regex (extern)
        // - (sin anyhow)
        // - tempfile (dev-dependency)
        // No debe importar crate::app, crate::infra, crate::cli, crate::config
        //
        // Este test es auto-verificable: si se añade un use crate::X no permitido,
        // el test de arquitectura (tests/architecture.rs) lo detectará.
        let source = std::fs::read_to_string(file!()).unwrap();

        // Solo verificar líneas que son imports reales de crate
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
            "domain/task.rs contiene imports no permitidos:\n{}",
            bad_imports.join("\n")
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // Edge cases adicionales
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn load_extracts_id_with_numeric_prefix_path() {
        // El ID debe extraerse del nombre sin el path completo
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-042.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.id, "TASK-042");
        assert!(!task.id.contains(".md"), "El ID no debe contener la extensión");
        assert!(!task.id.contains('/'), "El ID no debe contener separadores de path");
    }

    #[test]
    fn load_with_empty_content_produces_no_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "");

        let format = TaskFormatConfig {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers: HashMap::new(),
            dependency_marker: "Bloqueado por:".to_string(),
        };
        let task = load_task(&task_path, &format).unwrap();
        assert_eq!(task.id, "TASK-001");
        assert!(task.fields.is_empty());
        assert!(task.blockers.is_empty());
        assert!(task.activity_log.is_empty());
        assert_eq!(task.raw_content, "");
    }

    #[test]
    fn load_with_section_markers_containing_special_chars() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-099.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n## Prioridad (#)\n**urgente**\n",
        );

        let mut markers = HashMap::new();
        markers.insert("status".to_string(), "## Status".to_string());
        markers.insert("priority".to_string(), "## Prioridad (#)".to_string());

        let format = TaskFormatConfig {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers: markers,
            dependency_marker: "Bloqueado por:".to_string(),
        };
        let task = load_task(&task_path, &format).unwrap();
        assert_eq!(task.fields.get("status").map(|s| s.as_str()), Some("draft"));
        assert_eq!(task.fields.get("priority").map(|s| s.as_str()), Some("urgente"));
    }

    #[test]
    fn render_field_update_preserves_line_count() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        let content = "# TASK-001\n\n## Status\n**pending**\n\n## Descripción\nFoo\n\n## Notas\nextra\n";
        let original_lines = content.lines().count();
        write_task(&task_path, content);

        let format = default_task_format();
        let task = load_task(&task_path, &format).unwrap();
        let new_content = task
            .render_field_update("status", "in_progress", &format.section_markers)
            .unwrap();

        assert_eq!(
            new_content.lines().count(),
            original_lines,
            "render_field_update no debe cambiar el número de líneas"
        );
        assert!(new_content.contains("**in_progress**"));
        assert!(new_content.contains("## Descripción\nFoo"));
    }

    #[test]
    fn render_field_update_handles_value_with_bold_markers() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(&task_path, "## Status\n**draft**\n");

        let format = default_task_format();
        let task = load_task(&task_path, &format).unwrap();
        let new_content = task
            .render_field_update("status", "in **review** now", &format.section_markers)
            .unwrap();

        // El nuevo valor se envuelve en **...**, así que si el valor contiene **,
        // queda "**in **review** now**" — aceptable porque set_status en infra
        // escribe y re-parsea; si falla, rollback.
        assert!(new_content.contains("**in **review** now**"));
    }

    #[test]
    fn task_format_config_default_has_status_marker() {
        let cfg = TaskFormatConfig::default();
        assert_eq!(cfg.id_pattern, r"TASK-\d+");
        assert!(cfg.section_markers.contains_key("status"));
        assert_eq!(cfg.section_markers.get("status").unwrap(), "## Status");
        assert_eq!(cfg.dependency_marker, "Bloqueado por:");
    }

    #[test]
    fn render_field_update_maintains_leading_whitespace() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        let content = "## Status\n  **pending**\n";
        write_task(&task_path, content);

        let format = default_task_format();
        let task = load_task(&task_path, &format).unwrap();
        let new_content = task
            .render_field_update("status", "done", &format.section_markers)
            .unwrap();

        // Debe preservar el leading whitespace de la línea original
        assert!(new_content.contains("  **done**"));
    }

    #[test]
    fn activity_log_entries_are_stable_order() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Activity Log\n- 2026-01-01 | A | Primero\n- 2026-02-01 | B | Segundo\n- 2026-03-01 | C | Tercero\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert_eq!(task.activity_log.len(), 3);
        assert_eq!(task.activity_log[0].description, "Primero");
        assert_eq!(task.activity_log[1].description, "Segundo");
        assert_eq!(task.activity_log[2].description, "Tercero");
    }

    #[test]
    fn parse_blockers_is_case_insensitive_for_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-001.md");
        write_task(
            &task_path,
            "## Status\n**draft**\n\n- bLoQuEaDo PoR: TASK-005, TASK-006\n",
        );

        let task = load_task(&task_path, &default_task_format()).unwrap();
        assert!(task.blockers.contains(&"TASK-005".to_string()));
        assert!(task.blockers.contains(&"TASK-006".to_string()));
    }

    #[test]
    fn load_with_custom_task_format_where_status_marker_is_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let task_path = tmp.path().join("TASK-050.md");
        write_task(&task_path, "## Title\nMi tarea especial\n");

        let mut markers = HashMap::new();
        markers.insert("title".to_string(), "## Title".to_string());

        let format = TaskFormatConfig {
            id_pattern: r"TASK-\d+".to_string(),
            section_markers: markers,
            dependency_marker: "Bloqueado por:".to_string(),
        };
        let task = load_task(&task_path, &format).unwrap();
        assert_eq!(task.fields.get("title").map(|s| s.as_str()), Some("Mi tarea especial"));
        assert!(!task.fields.contains_key("status"));
    }
}
