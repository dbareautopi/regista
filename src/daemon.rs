//! Modo daemon de regista.
//!
//! Permite lanzar regista en segundo plano (`--detach`),
//! seguir su log en vivo (`--follow`), consultar estado (`--status`)
//! y detenerlo (`--kill`).
//!
//! El estado del daemon (PID, archivo de log, directorio del proyecto)
//! se guarda en un archivo TOML: `<project_dir>/.regista/daemon.pid`.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::{fs, thread};

/// Metadatos del daemon guardados en el archivo PID.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonState {
    pub pid: u32,
    pub log_file: PathBuf,
    pub project_dir: PathBuf,
}

impl DaemonState {
    /// Ruta al archivo de estado del daemon dentro del proyecto.
    pub fn pid_file(project_dir: &Path) -> PathBuf {
        project_dir.join(".regista/daemon.pid")
    }

    /// Carga el estado desde el archivo PID, si existe.
    pub fn load(project_dir: &Path) -> Option<Self> {
        let path = Self::pid_file(project_dir);
        let content = fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    }

    /// Guarda el estado en el archivo PID.
    pub fn save(&self, project_dir: &Path) -> anyhow::Result<()> {
        let path = Self::pid_file(project_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Elimina el archivo PID.
    pub fn remove(project_dir: &Path) {
        let _ = fs::remove_file(Self::pid_file(project_dir));
    }
}

/// Guard que limpia el archivo PID al salir del proceso daemon.
pub struct PidCleanup(pub PathBuf);

impl Drop for PidCleanup {
    fn drop(&mut self) {
        DaemonState::remove(&self.0);
    }
}

// ── Comandos daemon ──────────────────────────────────────────────────────

/// Lanza el orquestador en segundo plano (modo daemon).
///
/// `child_args` son los argumentos completos que se pasarán al proceso hijo,
/// excluyendo el path del binario. Deben incluir `--daemon` y `--log-file`.
///
/// Ejemplo de child_args:
///   ["run", ".", "--daemon", "--log-file", ".regista/daemon.log", "--epic", "EPIC-001"]
///
/// Retorna el PID del hijo.
pub fn detach(
    project_dir: &Path,
    child_args: &[String],
    log_file_override: Option<&Path>,
) -> anyhow::Result<u32> {
    let exe = std::env::current_exe()?;
    let canonical_project = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    // Determinar el archivo de log: override explícito > buscar --log-file en child_args > default
    let log_file = match log_file_override {
        Some(p) => p.to_path_buf(),
        None => {
            let mut log_path = canonical_project.join(".regista/daemon.log");
            let mut i = 0;
            while i < child_args.len() {
                if child_args[i] == "--log-file" && i + 1 < child_args.len() {
                    log_path = PathBuf::from(&child_args[i + 1]);
                    break;
                }
                i += 1;
            }
            log_path
        }
    };

    // Crear directorio padre del log si es necesario
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent)?;
    }

    // Crear/truncar el archivo de log para stdout del hijo
    let log_handle = fs::File::create(&log_file)?;

    let child = Command::new(&exe)
        .args(child_args)
        .stdin(std::process::Stdio::null())
        .stdout(log_handle)
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let pid = child.id();

    // Guardar estado para los comandos de gestión
    let state = DaemonState {
        pid,
        log_file,
        project_dir: canonical_project.clone(),
    };
    state.save(&canonical_project)?;

    Ok(pid)
}

/// Consulta si el daemon está corriendo.
/// Retorna un mensaje descriptivo para el usuario.
pub fn status(project_dir: &Path) -> anyhow::Result<String> {
    let canonical = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    match DaemonState::load(&canonical) {
        None => Ok("❌ No se encontró archivo PID. El daemon no está corriendo.".to_string()),
        Some(state) => {
            if is_process_alive(state.pid) {
                Ok(format!(
                    "✅ Daemon corriendo (PID: {}, log: {})",
                    state.pid,
                    state.log_file.display()
                ))
            } else {
                DaemonState::remove(&canonical);
                Ok(format!(
                    "❌ PID {} ya no existe. Archivo PID huérfano limpiado.",
                    state.pid
                ))
            }
        }
    }
}

/// Detiene el daemon (SIGTERM, luego SIGKILL si es necesario).
pub fn kill(project_dir: &Path) -> anyhow::Result<String> {
    let canonical = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    let state = match DaemonState::load(&canonical) {
        Some(s) => s,
        None => {
            return Ok("❌ No se encontró archivo PID. El daemon no está corriendo.".to_string());
        }
    };

    if !is_process_alive(state.pid) {
        DaemonState::remove(&canonical);
        return Ok(format!(
            "❌ PID {} ya no existe. Archivo PID huérfano limpiado.",
            state.pid
        ));
    }

    // Enviar SIGTERM
    send_signal(state.pid, 15);
    thread::sleep(Duration::from_secs(2));

    // Si sigue vivo, SIGKILL
    if is_process_alive(state.pid) {
        send_signal(state.pid, 9);
        thread::sleep(Duration::from_millis(500));
    }

    DaemonState::remove(&canonical);

    if !is_process_alive(state.pid) {
        Ok(format!(
            "✅ Daemon (PID: {}) detenido correctamente.",
            state.pid
        ))
    } else {
        Ok(format!(
            "⚠️  No se pudo detener el proceso {}. Prueba: kill -9 {}",
            state.pid, state.pid
        ))
    }
}

/// Sigue el log del daemon en vivo (como `tail -f`).
///
/// Se queda bloqueado mostrando nuevas líneas hasta que el usuario
/// pulsa Ctrl+C o el daemon termina.
pub fn follow(project_dir: &Path) -> anyhow::Result<()> {
    let canonical = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());

    let state = match DaemonState::load(&canonical) {
        Some(s) => s,
        None => {
            anyhow::bail!("No se encontró archivo PID. ¿Está el daemon corriendo?");
        }
    };

    if !is_process_alive(state.pid) {
        DaemonState::remove(&canonical);
        anyhow::bail!("El daemon (PID: {}) ya no está corriendo.", state.pid);
    }

    eprintln!(
        "Siguiendo log: {}\nCtrl+C para salir (el daemon sigue corriendo).\n",
        state.log_file.display()
    );

    let mut file = fs::File::open(&state.log_file)?;
    // Saltar al final del archivo
    file.seek(std::io::SeekFrom::End(0))?;

    loop {
        // Verificar que el daemon siga vivo
        if !is_process_alive(state.pid) {
            // Leer lo que quede antes de salir
            drain_remaining(&mut file)?;
            eprintln!("\n── Daemon terminado (PID: {}) ──", state.pid);
            DaemonState::remove(&canonical);
            break;
        }

        let mut buf = [0u8; 4096];
        match file.read(&mut buf) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(200));
                // Reabrir archivo por si hubo rotación (poco probable pero seguro)
                if !state.log_file.exists() {
                    break;
                }
            }
            Ok(n) => {
                std::io::stdout().write_all(&buf[..n])?;
                std::io::stdout().flush()?;
            }
            Err(_) => break,
        }
    }

    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────

/// Comprueba si un proceso está vivo mediante `/proc/<pid>`.
fn is_process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Envía una señal a un proceso mediante el comando `kill`.
fn send_signal(pid: u32, sig: i32) -> bool {
    Command::new("kill")
        .arg(format!("-{sig}"))
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Drena y muestra el contenido restante de un archivo.
fn drain_remaining(file: &mut fs::File) -> anyhow::Result<()> {
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    if !buf.is_empty() {
        std::io::stdout().write_all(buf.as_bytes())?;
        std::io::stdout().flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_file_path_ends_with_correct_name() {
        let path = DaemonState::pid_file(Path::new("/tmp/myproject"));
        assert_eq!(path.file_name().unwrap(), "daemon.pid");
    }

    #[test]
    fn is_process_alive_init_is_pid1() {
        // PID 1 (init/systemd) siempre existe en Linux
        assert!(is_process_alive(1));
    }

    #[test]
    fn is_process_alive_returns_false_for_impossible_pid() {
        // Un PID muy alto que casi seguro no existe
        assert!(!is_process_alive(0xFFFF_FFF0));
    }

    #[test]
    fn state_save_and_load_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        let state = DaemonState {
            pid: 12345,
            log_file: PathBuf::from("/tmp/foo.log"),
            project_dir: PathBuf::from("/tmp/myproject"),
        };
        state.save(tmp.path()).unwrap();
        let loaded = DaemonState::load(tmp.path()).unwrap();
        assert_eq!(loaded.pid, 12345);
        assert_eq!(loaded.log_file, PathBuf::from("/tmp/foo.log"));
        assert_eq!(loaded.project_dir, PathBuf::from("/tmp/myproject"));
        // Cleanup
        DaemonState::remove(tmp.path());
    }

    #[test]
    fn state_load_returns_none_when_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(DaemonState::load(tmp.path()).is_none());
    }

    #[test]
    fn state_remove_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let state = DaemonState {
            pid: 42,
            log_file: PathBuf::from("/dev/null"),
            project_dir: tmp.path().to_path_buf(),
        };
        state.save(tmp.path()).unwrap();
        assert!(DaemonState::pid_file(tmp.path()).exists());
        DaemonState::remove(tmp.path());
        assert!(!DaemonState::pid_file(tmp.path()).exists());
    }
}
