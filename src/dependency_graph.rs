//! Grafo de dependencias entre historias.
//!
//! Permite detectar ciclos (dependencias circulares), calcular
//! el conteo de referencias inversas (cuántas historias desbloquea cada una),
//! y determinar si una historia bloqueada puede desbloquearse.

use crate::story::Story;
use std::collections::{HashMap, HashSet};

/// Grafo dirigido de dependencias: `bloqueador → bloqueados`.
///
/// Las aristas van del bloqueador a la historia bloqueada.
/// Ejemplo: si STORY-002 depende de STORY-001, hay arista 001→002.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// Para cada historia, las historias que dependen de ella.
    forward: HashMap<String, Vec<String>>,
    /// Para cada historia, las historias de las que depende.
    reverse: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Construye el grafo a partir de una lista de historias.
    pub fn from_stories(stories: &[Story]) -> Self {
        let mut graph = Self::default();

        for story in stories {
            // Asegurar que toda historia tenga entrada (aunque no tenga dependencias)
            graph.forward.entry(story.id.clone()).or_default();
            graph.reverse.entry(story.id.clone()).or_default();

            for blocker in &story.blockers {
                graph
                    .forward
                    .entry(blocker.clone())
                    .or_default()
                    .push(story.id.clone());
                graph
                    .reverse
                    .entry(story.id.clone())
                    .or_default()
                    .push(blocker.clone());
            }
        }

        graph
    }

    /// Cuántas historias bloquea directamente esta historia.
    pub fn blocks_count(&self, story_id: &str) -> usize {
        self.forward.get(story_id).map(|v| v.len()).unwrap_or(0)
    }

    /// IDs de historias bloqueadas por esta historia.
    #[allow(dead_code)]
    pub fn blocked_by_me(&self, story_id: &str) -> Vec<&str> {
        self.forward
            .get(story_id)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Detecta si existe un ciclo que incluya a `story_id`.
    ///
    /// Usa DFS con colores: 0 = no visitado, 1 = en pila, 2 = procesado.
    pub fn has_cycle_from(&self, story_id: &str) -> bool {
        let mut color: HashMap<&str, u8> = self.forward.keys().map(|k| (k.as_str(), 0u8)).collect();

        self.dfs_has_cycle(story_id, &mut color)
    }

    /// Detecta si existe ALGÚN ciclo en todo el grafo.
    #[allow(dead_code)]
    pub fn has_any_cycle(&self) -> bool {
        let mut color: HashMap<&str, u8> = self.forward.keys().map(|k| (k.as_str(), 0u8)).collect();

        for node in self.forward.keys() {
            if color.get(node.as_str()) == Some(&0) && self.dfs_has_cycle(node, &mut color) {
                return true;
            }
        }
        false
    }

    fn dfs_has_cycle(&self, node: &str, color: &mut HashMap<&str, u8>) -> bool {
        *color.get_mut(node).unwrap() = 1; // en pila

        if let Some(neighbors) = self.forward.get(node) {
            for neighbor in neighbors {
                let neighbor_color = *color.get(neighbor.as_str()).unwrap_or(&0);
                if neighbor_color == 1 {
                    return true; // ciclo detectado
                }
                if neighbor_color == 0 && self.dfs_has_cycle(neighbor, color) {
                    return true;
                }
            }
        }

        *color.get_mut(node).unwrap() = 2; // procesado
        false
    }

    /// Encuentra los IDs de todas las historias que forman parte de un ciclo.
    #[allow(dead_code)]
    pub fn find_cycle_members(&self) -> HashSet<String> {
        let mut color: HashMap<&str, u8> = self.forward.keys().map(|k| (k.as_str(), 0u8)).collect();

        let mut in_stack = HashSet::new();
        let mut cycle_members = HashSet::new();

        for node in self.forward.keys() {
            if color.get(node.as_str()) == Some(&0) {
                self.dfs_find_cycle(node, &mut color, &mut in_stack, &mut cycle_members);
            }
        }

        cycle_members
    }

    #[allow(dead_code)]
    fn dfs_find_cycle(
        &self,
        node: &str,
        color: &mut HashMap<&str, u8>,
        in_stack: &mut HashSet<String>,
        cycle_members: &mut HashSet<String>,
    ) {
        *color.get_mut(node).unwrap() = 1;
        in_stack.insert(node.to_string());

        if let Some(neighbors) = self.forward.get(node) {
            for neighbor in neighbors {
                let neighbor_color = *color.get(neighbor.as_str()).unwrap_or(&0);
                if neighbor_color == 1 {
                    // Encontramos un ciclo: marcamos todo lo que está en la pila
                    cycle_members.extend(in_stack.iter().cloned());
                } else if neighbor_color == 0 {
                    self.dfs_find_cycle(neighbor, color, in_stack, cycle_members);
                }
            }
        }

        in_stack.remove(node);
        *color.get_mut(node).unwrap() = 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn story(id: &str, blockers: &[&str]) -> Story {
        Story {
            id: id.to_string(),
            path: format!("stories/{id}.md").into(),
            status: crate::state::Status::Blocked,
            epic: None,
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            last_rejection: None,
            raw_content: String::new(),
        }
    }

    #[test]
    fn no_cycle_linear_chain() {
        let stories = vec![
            story("STORY-001", &[]),
            story("STORY-002", &["STORY-001"]),
            story("STORY-003", &["STORY-002"]),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        assert!(!graph.has_any_cycle());
        assert!(!graph.has_cycle_from("STORY-001"));
        assert!(!graph.has_cycle_from("STORY-003"));
    }

    #[test]
    fn cycle_two_nodes() {
        let stories = vec![
            story("STORY-001", &["STORY-002"]),
            story("STORY-002", &["STORY-001"]),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        assert!(graph.has_any_cycle());
        assert!(graph.has_cycle_from("STORY-001"));
        assert!(graph.has_cycle_from("STORY-002"));
    }

    #[test]
    fn cycle_three_nodes() {
        let stories = vec![
            story("STORY-001", &["STORY-003"]),
            story("STORY-002", &["STORY-001"]),
            story("STORY-003", &["STORY-002"]),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        assert!(graph.has_any_cycle());

        let members = graph.find_cycle_members();
        assert!(members.len() >= 3);
        assert!(members.contains("STORY-001"));
        assert!(members.contains("STORY-002"));
        assert!(members.contains("STORY-003"));
    }

    #[test]
    fn blocks_count_works() {
        let stories = vec![
            story("STORY-001", &[]),
            story("STORY-002", &["STORY-001"]),
            story("STORY-003", &["STORY-001"]),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        assert_eq!(graph.blocks_count("STORY-001"), 2);
        assert_eq!(graph.blocks_count("STORY-002"), 0);
    }
}
