//! Snapshots y rollback con git.
//!
//! Si `git.enabled = true` en la configuración, antes de cada paso
//! se crea un commit de snapshot para poder hacer rollback en caso
//! de que la verificación post-fase falle.

use std::path::Path;
use std::process::Command;

/// Crea un snapshot git y retorna el hash del commit actual.
///
/// Si no hay repositorio git, lo inicializa automáticamente.
/// Retorna `None` si git no está disponible o `git.enabled = false`.
pub fn snapshot(project_root: &Path, label: &str) -> Option<String> {
    if !has_git(project_root) {
        tracing::debug!("  sin repositorio git — inicializando...");
        if !init_git(project_root) {
            return None;
        }
    }

    // ¿Hay cambios que commitear?
    let has_changes = check_git_changes(project_root);

    if !has_changes {
        // Retornar el hash actual sin crear nuevo commit
        return current_hash(project_root);
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("add")
        .arg("-A")
        .output();

    if output.is_err() {
        return None;
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("commit")
        .arg("-q")
        .arg("-m")
        .arg(format!("snapshot: {label}"))
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let hash = current_hash(project_root);
            tracing::debug!("  📸 snapshot: {label} → {:?}", hash);
            hash
        }
        _ => None,
    }
}

/// Hace rollback al commit indicado.
pub fn rollback(project_root: &Path, prev_hash: &str, label: &str) -> bool {
    tracing::info!("  ↻ rollback al commit {} ({label})", &prev_hash[..8.min(prev_hash.len())]);

    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("reset")
        .arg("--hard")
        .arg(prev_hash)
        .output();

    match output {
        Ok(o) if o.status.success() => true,
        _ => false,
    }
}

// ── helpers ──────────────────────────────────────────────────────────────

fn has_git(project_root: &Path) -> bool {
    project_root.join(".git").is_dir()
}

fn init_git(project_root: &Path) -> bool {
    let _ = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("init")
        .arg("-q")
        .output();

    let _ = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("config")
        .arg("user.email")
        .arg("regista@pi.local")
        .output();

    let _ = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("config")
        .arg("user.name")
        .arg("regista")
        .output();

    has_git(project_root)
}

fn check_git_changes(project_root: &Path) -> bool {
    // git diff --quiet: exit 0 = no changes, exit 1 = there are changes
    let status = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("diff")
        .arg("--quiet")
        .arg("--exit-code")
        .status();

    if status.map(|s| s.code() == Some(1)).unwrap_or(false) {
        return true;
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .arg("--exit-code")
        .status();

    if status.map(|s| s.code() == Some(1)).unwrap_or(false) {
        return true;
    }

    // ¿Hay archivos sin trackear?
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("ls-files")
        .arg("--others")
        .arg("--exclude-standard")
        .output();

    output
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

fn current_hash(project_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
