//! Parseo y manipulación de archivos de historia (.md).
//!
//! Las historias siguen un formato fijo con secciones markdown.
//! Este módulo extrae: status, épica, bloqueadores, activity log,
//! y permite actualizar el status de forma segura.

use crate::state::Status;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Representación en memoria de una historia de usuario.
#[derive(Debug, Clone)]
pub struct Story {
    /// Identificador único (STORY-NNN).
    pub id: String,
    /// Ruta al archivo .md en disco.
    pub path: PathBuf,
    /// Estado actual según la máquina de estados.
    pub status: Status,
    /// Épica a la que pertenece (EPIC-NNN), si está definida.
    pub epic: Option<String>,
    /// IDs de historias que bloquean esta (dependencias).
    pub blockers: Vec<String>,
    /// Línea del Activity Log que contiene la última razón de rechazo.
    pub last_rejection: Option<String>,
    /// Contenido completo del archivo (para reescribir al actualizar).
    pub(crate) raw_content: String,
}

// ── Parseo ──────────────────────────────────────────────────────────────

/// Extrae el status de la sección `## Status`.
fn parse_status(content: &str) -> Option<Status> {
    // Buscar la sección "## Status" (case-insensitive)
    let status_section = content
        .lines()
        .skip_while(|l| !l.to_lowercase().starts_with("## status"))
        .nth(1)?; // línea siguiente

    let cleaned = status_section.trim().replace("**", "").trim().to_string();

    match cleaned.to_lowercase().as_str() {
        "draft" => Some(Status::Draft),
        "ready" => Some(Status::Ready),
        "tests ready" => Some(Status::TestsReady),
        "in progress" => Some(Status::InProgress),
        "in review" => Some(Status::InReview),
        "business review" => Some(Status::BusinessReview),
        "done" => Some(Status::Done),
        "blocked" => Some(Status::Blocked),
        "failed" => Some(Status::Failed),
        _ => None,
    }
}

/// Extrae la épica de la sección `## Epic`.
fn parse_epic(content: &str) -> Option<String> {
    static EPIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)EPIC-\d+").unwrap());
    let section = extract_section(content, "## Epic")?;
    EPIC_RE.find(&section).map(|m| m.as_str().to_uppercase())
}

/// Extrae los bloqueadores de la sección `## Dependencias`.
fn parse_blockers(content: &str) -> Vec<String> {
    static BLOCKER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)STORY-\d+").unwrap());

    // Buscar la línea con "Bloqueado por" dentro de la sección Dependencias
    let section = match extract_section(content, "## Dependencias") {
        Some(s) => s,
        None => return vec![],
    };

    let blockers_line = section
        .lines()
        .find(|l| l.to_lowercase().contains("bloqueado por"))
        .unwrap_or("");

    BLOCKER_RE
        .find_iter(blockers_line)
        .map(|m| m.as_str().to_uppercase())
        .collect()
}

/// Extrae la última razón de rechazo del `## Activity Log`.
fn parse_last_rejection(content: &str) -> Option<String> {
    let section = extract_section(content, "## Activity Log")?;

    // Buscar líneas que contengan "rechaz" (case-insensitive)
    let rejection_lines: Vec<&str> = section
        .lines()
        .filter(|l| l.to_lowercase().contains("rechaz"))
        .collect();

    rejection_lines.last().map(|l| l.trim().to_string())
}

/// Extrae el último actor del `## Activity Log`.
///
/// El formato esperado por línea es: `- YYYY-MM-DD | Actor | descripción`.
/// Retorna el Actor (ej: "Dev", "QA", "PO", "Reviewer") de la última línea.
fn parse_last_actor(content: &str) -> Option<String> {
    static ACTOR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|\s*([^|]+?)\s*\|").unwrap());

    let section = extract_section(content, "## Activity Log")?;
    let last_line = section.lines().rfind(|l| !l.trim().is_empty())?;

    ACTOR_RE
        .captures(last_line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim().to_string())
}

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
            // Si encontramos otra sección `## ...`, terminamos
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

// ── Lectura / Escritura ─────────────────────────────────────────────────

impl Story {
    /// Carga una historia desde un archivo .md.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let status = parse_status(&content)
            .ok_or_else(|| anyhow::anyhow!("{id}: no se pudo parsear el status"))?;

        Ok(Self {
            id,
            path: path.to_path_buf(),
            status,
            epic: parse_epic(&content),
            blockers: parse_blockers(&content),
            last_rejection: parse_last_rejection(&content),
            raw_content: content,
        })
    }

    /// Actualiza el status en el archivo .md de forma segura.
    ///
    /// Escribe entre `**...**` en la línea siguiente a `## Status`.
    /// Hace backup automático en `<archivo>.bak`.
    pub fn set_status(&mut self, new_status: Status) -> anyhow::Result<()> {
        let _old_status_str = format!("**{}**", self.status);
        let new_status_str = format!("**{}**", new_status);

        // Buscar y reemplazar la línea de status
        let mut lines: Vec<String> = self.raw_content.lines().map(|l| l.to_string()).collect();
        let mut found = false;

        for i in 0..lines.len() {
            if lines[i].to_lowercase().trim() == "## status" {
                // La siguiente línea contiene el status actual
                if i + 1 < lines.len() {
                    let old_line = &lines[i + 1];
                    // Reemplazar manteniendo la indentación original
                    let leading = old_line.len() - old_line.trim_start().len();
                    let trailing = old_line.len() - old_line.trim_end().len();
                    let spaces_leading = " ".repeat(leading);
                    let spaces_trailing = " ".repeat(trailing);
                    lines[i + 1] =
                        format!("{}{}{}", spaces_leading, new_status_str, spaces_trailing);
                    found = true;
                }
                break;
            }
        }

        if !found {
            anyhow::bail!("{}: no se encontró la sección '## Status'", self.id);
        }

        let new_content = lines.join("\n");

        // Backup antes de escribir
        std::fs::copy(&self.path, self.path.with_extension("md.bak"))?;
        std::fs::write(&self.path, &new_content)?;

        // Verificar que se leyó correctamente
        let verification = Story::load(&self.path)?;
        if verification.status != new_status {
            // Restaurar backup
            std::fs::copy(self.path.with_extension("md.bak"), &self.path)?;
            let _ = std::fs::remove_file(self.path.with_extension("md.bak"));
            anyhow::bail!(
                "{}: la verificación falló tras escribir '{}', se lee '{}'",
                self.id,
                new_status,
                verification.status
            );
        }

        // Éxito: borrar backup
        let _ = std::fs::remove_file(self.path.with_extension("md.bak"));

        self.status = new_status;
        self.raw_content = new_content;
        Ok(())
    }

    /// ¿Bloquea esta historia a otras? (útil para deadlock detection).
    #[allow(dead_code)]
    pub fn blocks_stories(&self, all_stories: &[Story]) -> Vec<String> {
        all_stories
            .iter()
            .filter(|s| s.blockers.contains(&self.id))
            .map(|s| s.id.clone())
            .collect()
    }

    /// Devuelve el último actor que registró una entrada en el Activity Log.
    ///
    /// Útil para decidir si el Dev reportó problemas con los tests
    /// (lo que desencadena `TestsReady → TestsReady` en vez de `→ InReview`).
    pub fn last_actor(&self) -> Option<String> {
        parse_last_actor(&self.raw_content)
    }

    /// Avanza el estado en memoria sin escribir a disco (útil para dry-run).
    pub fn advance_status_in_memory(&mut self, new_status: Status) {
        let old = format!("**{}**", self.status);
        let new = format!("**{}**", new_status);
        self.raw_content = self.raw_content.replacen(&old, &new, 1);
        self.status = new_status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{name}")).expect("fixture not found")
    }

    #[test]
    fn parse_status_draft() {
        let content = fixture("story_draft.md");
        assert_eq!(parse_status(&content), Some(Status::Draft));
    }

    #[test]
    fn parse_status_business_review() {
        let content = fixture("story_business_review.md");
        assert_eq!(parse_status(&content), Some(Status::BusinessReview));
    }

    #[test]
    fn parse_status_blocked() {
        let content = fixture("story_blocked.md");
        assert_eq!(parse_status(&content), Some(Status::Blocked));
    }

    #[test]
    fn parse_blockers_multiple() {
        let content = fixture("story_blocked.md");
        let blockers = parse_blockers(&content);
        assert!(blockers.contains(&"STORY-001".to_string()));
        assert!(blockers.contains(&"STORY-002".to_string()));
    }

    #[test]
    fn parse_missing_section() {
        let content = "# STORY-001\nnada aqui\n";
        assert_eq!(parse_status(&content), None);
        assert_eq!(parse_epic(&content), None);
        assert!(parse_blockers(&content).is_empty());
        assert_eq!(parse_last_rejection(&content), None);
    }

    #[test]
    fn parse_last_rejection_finds_rechazo() {
        let content = r#"## Activity Log
- 2026-04-29 | Reviewer | Movida a In Progress. RECHAZADA: falta test para CA2
- 2026-04-30 | PO | Movida a Done
"#;
        let rejection = parse_last_rejection(content);
        assert!(rejection.is_some());
        assert!(rejection.unwrap().contains("RECHAZADA"));
    }

    #[test]
    fn parse_last_rejection_none_when_no_rejection() {
        let content = r#"## Activity Log
- 2026-04-29 | Dev | Implementado
- 2026-04-30 | Reviewer | Aprobado
"#;
        assert_eq!(parse_last_rejection(&content), None);
    }

    #[test]
    fn extract_section_returns_correct_content() {
        let content = r#"## Status
**Draft**

## Epic
EPIC-001

## Descripción
Foo bar
"#;
        let section = extract_section(content, "## Epic").unwrap();
        assert!(section.contains("EPIC-001"));
        assert!(!section.contains("Draft"));
    }

    // ── parse_last_actor ───────────────────────────────────────────

    #[test]
    fn parse_last_actor_extracts_dev() {
        let content = r#"## Activity Log
- 2026-04-29 | QA | Tests escritos
- 2026-04-30 | Dev | Implementación iniciada
"#;
        assert_eq!(parse_last_actor(content).as_deref(), Some("Dev"));
    }

    #[test]
    fn parse_last_actor_extracts_qa() {
        let content = r#"## Activity Log
- 2026-04-29 | PO | Grooming completado
- 2026-04-30 | QA | Tests corregidos
"#;
        assert_eq!(parse_last_actor(content).as_deref(), Some("QA"));
    }

    #[test]
    fn parse_last_actor_none_when_no_activity_log() {
        let content = "# STORY-001\n## Status\n**Draft**\n";
        assert_eq!(parse_last_actor(content), None);
    }

    #[test]
    fn parse_last_actor_none_when_empty_log() {
        let content = "## Activity Log\n";
        assert_eq!(parse_last_actor(content), None);
    }
}
