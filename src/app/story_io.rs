//! I/O operations for Story files.
//!
//! Separates filesystem I/O from domain parsing logic.
//! Domain `Story` only does pure parsing; this module handles reading/writing.

use crate::domain::state::Status;
use crate::domain::story::Story;
use std::path::Path;

/// Loads a story from a .md file.
///
/// Reads the file from disk and delegates to `Story::parse` for content parsing.
pub fn load(path: &Path) -> Result<Story, String> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        format!("{id}: error al leer archivo: {e}")
    })?;

    Story::parse(path, &content)
}

/// Saves a new status for a story, with atomic backup.
///
/// Renders the updated content via `Story::render_with_status`,
/// then writes to disk with a .bak backup and verification.
pub fn save_status(story: &mut Story, new_status: Status) -> Result<(), String> {
    let new_content = story.render_with_status(new_status)?;

    // Backup before writing
    let story_path = story.file_path().to_path_buf();
    std::fs::copy(&story_path, story_path.with_extension("md.bak"))
        .map_err(|e| format!("{}: error al hacer backup: {e}", story.id))?;
    std::fs::write(&story_path, &new_content)
        .map_err(|e| format!("{}: error al escribir: {e}", story.id))?;

    // Verify by re-reading
    let verification = load(&story_path)?;
    if verification.status != new_status {
        // Restore backup
        std::fs::copy(story_path.with_extension("md.bak"), &story_path)
            .map_err(|e| format!("{}: error al restaurar backup: {e}", story.id))?;
        let _ = std::fs::remove_file(story_path.with_extension("md.bak"));
        return Err(format!(
            "{}: la verificación falló tras escribir '{}', se lee '{}'",
            story.id,
            new_status,
            verification.status
        ));
    }

    // Success: remove backup
    let _ = std::fs::remove_file(story_path.with_extension("md.bak"));

    story.status = new_status;
    story.raw_content = new_content;
    Ok(())
}
