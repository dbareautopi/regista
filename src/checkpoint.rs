//! Checkpoint del orquestador: guarda el estado tras cada iteración para
//! poder reanudar la ejecución si se interrumpe (crash, timeout, Ctrl+C).
//!
//! Archivo: `<project_dir>/.regista.state.toml`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Estado completo del orquestador que se persiste entre ejecuciones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorState {
    pub iteration: u32,
    pub reject_cycles: HashMap<String, u32>,
    pub story_iterations: HashMap<String, u32>,
    pub story_errors: HashMap<String, String>,
}

impl OrchestratorState {
    /// Ruta al archivo de checkpoint.
    fn path(project_root: &Path) -> PathBuf {
        project_root.join(".regista.state.toml")
    }

    /// Guarda el estado a disco.
    pub fn save(&self, project_root: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(Self::path(project_root), content)?;
        tracing::debug!("💾 checkpoint guardado (iteración {})", self.iteration);
        Ok(())
    }

    /// Carga el estado desde disco, si existe.
    pub fn load(project_root: &Path) -> Option<Self> {
        let path = Self::path(project_root);
        if !path.exists() {
            return None;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<OrchestratorState>(&content) {
                Ok(state) => {
                    tracing::info!(
                        "📂 checkpoint cargado: iteración {}, {} ciclos de rechazo acumulados",
                        state.iteration,
                        state.reject_cycles.len()
                    );
                    Some(state)
                }
                Err(e) => {
                    tracing::warn!("⚠️  checkpoint corrupto (parse error): {e}. Se ignorará.");
                    let _ = std::fs::remove_file(&path);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("⚠️  no se pudo leer el checkpoint: {e}. Se ignorará.");
                None
            }
        }
    }

    /// Elimina el archivo de checkpoint (pipeline completado o limpieza manual).
    pub fn remove(project_root: &Path) {
        let path = Self::path(project_root);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
            tracing::debug!("🧹 checkpoint eliminado");
        }
    }

    /// Crea un estado inicial vacío (primera ejecución).
    #[allow(dead_code)]
    pub fn fresh() -> Self {
        Self {
            iteration: 0,
            reject_cycles: HashMap::new(),
            story_iterations: HashMap::new(),
            story_errors: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_save_and_load_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();

        let mut state = OrchestratorState::fresh();
        state.iteration = 7;
        state.reject_cycles.insert("STORY-001".into(), 2);
        state.story_iterations.insert("STORY-001".into(), 3);
        state
            .story_errors
            .insert("STORY-002".into(), "timeout".into());

        state.save(tmp.path()).unwrap();
        let loaded = OrchestratorState::load(tmp.path()).unwrap();

        assert_eq!(loaded.iteration, 7);
        assert_eq!(loaded.reject_cycles.get("STORY-001"), Some(&2));
        assert_eq!(loaded.story_iterations.get("STORY-001"), Some(&3));
        assert_eq!(
            loaded.story_errors.get("STORY-002"),
            Some(&"timeout".to_string())
        );
    }

    #[test]
    fn load_returns_none_when_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(OrchestratorState::load(tmp.path()).is_none());
    }

    #[test]
    fn load_returns_none_when_corrupt() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".regista.state.toml"), "esto no es toml{{{").unwrap();
        assert!(OrchestratorState::load(tmp.path()).is_none());
        // Debe haber borrado el archivo corrupto
        assert!(!tmp.path().join(".regista.state.toml").exists());
    }

    #[test]
    fn remove_cleans_file() {
        let tmp = tempfile::tempdir().unwrap();
        let state = OrchestratorState::fresh();
        state.save(tmp.path()).unwrap();
        assert!(tmp.path().join(".regista.state.toml").exists());
        OrchestratorState::remove(tmp.path());
        assert!(!tmp.path().join(".regista.state.toml").exists());
    }

    #[test]
    fn fresh_state_is_empty() {
        let state = OrchestratorState::fresh();
        assert_eq!(state.iteration, 0);
        assert!(state.reject_cycles.is_empty());
        assert!(state.story_iterations.is_empty());
        assert!(state.story_errors.is_empty());
    }
}
