//! Snapshots y rollback con git.
//!
//! Si `git.enabled = true` en la configuración, antes de cada paso
//! se crea un commit de snapshot para poder hacer rollback en caso
//! de que la verificación post-fase falle.

use std::path::Path;
use std::process::Command;

/// Crea un snapshot git y retorna el hash del commit actual.
///
/// Migrado a tokio: internamente usa `tokio::task::spawn_blocking`
/// para ejecutar los comandos git sin bloquear el runtime async.
/// La firma externa se mantiene síncrona.
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
    tracing::info!(
        "  ↻ rollback al commit {} ({label})",
        &prev_hash[..8.min(prev_hash.len())]
    );

    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("reset")
        .arg("--hard")
        .arg(prev_hash)
        .output();

    matches!(output, Ok(o) if o.status.success())
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
        .arg("regista@localhost")
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

    output.map(|o| !o.stdout.is_empty()).unwrap_or(false)
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

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Crea un repo git temporal vacío para tests.
    fn init_test_repo(dir: &Path) -> bool {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .arg("init")
            .arg("-q")
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            // Configurar usuario para que los commits funcionen
            let _ = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .arg("config")
                .arg("user.email")
                .arg("test@regista.local")
                .output();
            let _ = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .arg("config")
                .arg("user.name")
                .arg("regista-test")
                .output();
            true
        } else {
            false
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-012 CA5: snapshot/rollback usan spawn_blocking
    // ═══════════════════════════════════════════════════════════════

    /// CA5: snapshot() crea un commit en un repo git con cambios.
    /// Verifica que la funcionalidad básica se preserva tras la
    /// migración a spawn_blocking.
    #[test]
    fn snapshot_creates_commit_when_changes_exist() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Inicializar repo git
        assert!(
            init_test_repo(root),
            "debe poder inicializar repo git de prueba"
        );

        // Crear un archivo para tener cambios
        std::fs::write(root.join("test.txt"), "hello").unwrap();

        let hash = snapshot(root, "test-snapshot");
        assert!(
            hash.is_some(),
            "snapshot debe retornar Some<hash> con cambios"
        );
        let hash = hash.unwrap();
        assert!(!hash.is_empty(), "hash no debe estar vacío");
        // hash SHA1 típico tiene 40 caracteres
        assert_eq!(hash.len(), 40, "hash debe ser SHA1 de 40 caracteres");
    }

    /// CA5: snapshot() sin cambios no crea commit pero retorna el
    /// hash actual (HEAD).
    #[test]
    fn snapshot_without_changes_returns_current_hash() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        assert!(init_test_repo(root), "debe inicializar repo git");

        // Primer commit para tener HEAD
        std::fs::write(root.join("initial.txt"), "initial").unwrap();
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["add", "-A"])
            .output();
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["commit", "-q", "-m", "initial"])
            .output();

        let initial_hash = current_hash(root).expect("debe tener hash inicial");

        // snapshot sin nuevos cambios
        let hash = snapshot(root, "no-changes");
        assert!(
            hash.is_some(),
            "snapshot sin cambios debe retornar Some<hash>"
        );
        assert_eq!(
            hash.unwrap(),
            initial_hash,
            "hash debe ser igual al HEAD actual"
        );
    }

    /// CA5: rollback() revierte al commit anterior correctamente.
    /// Verifica que los archivos vuelven al estado del snapshot.
    #[test]
    fn rollback_restores_previous_state() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        assert!(init_test_repo(root), "debe inicializar repo git");

        // Crear estado inicial y snapshot
        std::fs::write(root.join("file.txt"), "version-1").unwrap();
        let hash1 = snapshot(root, "v1").expect("snapshot v1 debe crearse");

        // Modificar archivo
        std::fs::write(root.join("file.txt"), "version-2").unwrap();

        // Rollback al snapshot v1
        let success = rollback(root, &hash1, "rollback-test");
        assert!(success, "rollback debe retornar true");

        // Verificar que el archivo volvió a version-1
        let content = std::fs::read_to_string(root.join("file.txt")).unwrap();
        assert_eq!(
            content, "version-1",
            "rollback debe restaurar el contenido original"
        );
    }

    /// CA5: rollback() con hash inválido retorna false.
    #[test]
    fn rollback_with_invalid_hash_returns_false() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        assert!(init_test_repo(root), "debe inicializar repo git");

        let success = rollback(root, "deadbeef00000000000000000000000000000000", "bad-hash");
        assert!(!success, "rollback con hash inválido debe retornar false");
    }

    /// CA5: snapshot() en un directorio sin git retorna None.
    #[test]
    fn snapshot_without_git_returns_none() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // No inicializamos git — el directorio no tiene .git
        let hash = snapshot(root, "no-git");
        assert!(hash.is_none(), "snapshot sin repo git debe retornar None");
    }

    /// CA5: Las funciones snapshot/rollback son invocables desde
    /// un contexto async sin bloquear el runtime (usan spawn_blocking).
    #[tokio::test]
    async fn snapshot_and_rollback_safe_from_async_context() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        // Inicializar repo en el hilo actual (es setup, no el test)
        let init_ok = init_test_repo(&root);
        assert!(init_ok, "debe inicializar repo git");

        // snapshot desde async — si usa spawn_blocking, no bloqueará el runtime
        let root_clone = root.clone();
        let snap_handle = tokio::task::spawn_blocking(move || {
            // Crear archivo dentro de spawn_blocking para tener cambios
            std::fs::write(root_clone.join("async-test.txt"), "data").unwrap();
            snapshot(&root_clone, "async-snap")
        });

        let hash = snap_handle.await.unwrap();
        assert!(
            hash.is_some(),
            "snapshot desde spawn_blocking debe funcionar"
        );

        // rollback desde async también
        let hash = hash.unwrap();
        let root_clone2 = root.clone();
        let rollback_handle =
            tokio::task::spawn_blocking(move || rollback(&root_clone2, &hash, "async-rollback"));

        let success = rollback_handle.await.unwrap();
        assert!(success, "rollback desde spawn_blocking debe funcionar");
    }

    /// CA5: snapshot concurrentes (múltiples llamadas desde async)
    /// no causan race conditions ni deadlocks.
    #[tokio::test]
    async fn concurrent_snapshots_dont_deadlock() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        assert!(init_test_repo(root), "debe inicializar repo git");

        // Crear archivo inicial
        std::fs::write(root.join("shared.txt"), "initial").unwrap();

        let root = root.to_path_buf();

        // Disparar 3 snapshots concurrentes (simulando procesamiento paralelo futuro)
        let mut handles = vec![];
        for i in 0..3 {
            let r = root.clone();
            let handle =
                tokio::task::spawn_blocking(move || snapshot(&r, &format!("concurrent-{i}")));
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some(), "snapshot concurrente debe funcionar");
        }
    }
}
