//! I/O operations for Task files.
//!
//! Separates filesystem I/O from domain parsing logic.
//! Domain `Task` only does pure parsing; this module handles reading/writing.

use crate::domain::task::{Task, TaskFormatConfig};
use std::path::Path;

/// Loads a task from a .md file.
pub fn load(path: &Path, task_format: &TaskFormatConfig) -> Result<Task, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("error al leer {}: {e}", path.display()))?;
    Task::parse(path, &content, task_format)
}

/// Saves a new value for a task field, with atomic backup.
pub fn save_field(
    task: &mut Task,
    field_name: &str,
    new_value: &str,
    task_format: &TaskFormatConfig,
) -> Result<(), String> {
    let new_content = task.render_field_update(field_name, new_value, &task_format.section_markers)?;

    let task_path = task.file_path().to_path_buf();

    // Atomic write with backup
    std::fs::copy(&task_path, task_path.with_extension("md.bak"))
        .map_err(|e| format!("{}: error al hacer backup: {e}", task.id))?;
    std::fs::write(&task_path, &new_content)
        .map_err(|e| format!("{}: error al escribir: {e}", task.id))?;

    // Verify by re-reading
    let verification = load(&task_path, task_format)?;
    if verification.fields.get(field_name).map(|s| s.as_str()) != Some(new_value) {
        // Restore backup
        std::fs::copy(task_path.with_extension("md.bak"), &task_path)
            .map_err(|e| format!("{}: error al restaurar backup: {e}", task.id))?;
        let _ = std::fs::remove_file(task_path.with_extension("md.bak"));
        return Err(format!(
            "{}: la verificación falló tras escribir '{}', se lee '{:?}'",
            task.id,
            new_value,
            verification.fields.get(field_name)
        ));
    }

    // Success
    let _ = std::fs::remove_file(task_path.with_extension("md.bak"));
    task.fields.insert(field_name.to_string(), new_value.to_string());
    task.raw_content = new_content;
    Ok(())
}
