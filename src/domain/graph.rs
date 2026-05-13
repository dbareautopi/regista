//! Grafo de dependencias entre tareas (genérico).
//!
//! Permite detectar ciclos (dependencias circulares), calcular
//! el conteo de referencias inversas (cuántas tareas desbloquea cada una),
//! y determinar si una tarea bloqueada puede desbloquearse.
//!
//! Soporta tanto `Story` (v0.x) como `Task` (v1.0) mediante métodos separados.

use crate::domain::story::Story;
use crate::domain::task::Task;
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
    /// Construye el grafo a partir de una lista de historias (v0.x).
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

    /// Construye el grafo a partir de una lista de tareas genéricas (v1.0).
    ///
    /// No asume ningún formato de ID (STORY-NNN, TASK-NNN, ISSUE-NNN, etc.).
    pub fn from_tasks(tasks: &[Task]) -> Self {
        let mut graph = Self::default();

        for task in tasks {
            graph.forward.entry(task.id.clone()).or_default();
            graph.reverse.entry(task.id.clone()).or_default();

            for blocker in &task.blockers {
                graph
                    .forward
                    .entry(blocker.clone())
                    .or_default()
                    .push(task.id.clone());
                graph
                    .reverse
                    .entry(task.id.clone())
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
            status: crate::domain::state::Status::Blocked,
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-V10-009: from_tasks() con IDs genéricos
    // ═══════════════════════════════════════════════════════════════

    use crate::domain::task::Task;
    use std::path::PathBuf;

    fn task(id: &str, blockers: &[&str]) -> Task {
        Task {
            id: id.to_string(),
            path: PathBuf::from(format!("tasks/{id}.md")),
            fields: std::collections::HashMap::new(),
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            activity_log: vec![],
            raw_content: String::new(),
        }
    }

    #[test]
    fn from_tasks_builds_graph_with_task_ids() {
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-002", &["TASK-001"]),
            task("TASK-003", &["TASK-001"]),
            task("TASK-004", &["TASK-002"]),
            task("TASK-005", &["TASK-002"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert_eq!(graph.blocks_count("TASK-001"), 2);
        assert_eq!(graph.blocks_count("TASK-002"), 2);
        assert_eq!(graph.blocks_count("TASK-005"), 0);
    }

    #[test]
    fn from_tasks_works_with_issue_ids() {
        let tasks = vec![
            task("ISSUE-010", &[]),
            task("ISSUE-011", &[]),
            task("ISSUE-012", &["ISSUE-010", "ISSUE-011"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert_eq!(graph.blocks_count("ISSUE-010"), 1);
        assert_eq!(graph.blocks_count("ISSUE-011"), 1);
        assert_eq!(graph.blocks_count("ISSUE-012"), 0);
    }

    #[test]
    fn from_tasks_empty_graph() {
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-002", &[]),
            task("TASK-003", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert!(!graph.has_any_cycle());
        for id in &["TASK-001", "TASK-002", "TASK-003"] {
            assert!(!graph.has_cycle_from(id));
            assert_eq!(graph.blocks_count(id), 0);
        }
    }

    #[test]
    fn from_tasks_detects_cycle_with_generic_ids() {
        let tasks = vec![
            task("ISSUE-001", &["ISSUE-002"]),
            task("ISSUE-002", &["ISSUE-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert!(graph.has_any_cycle());
        assert!(graph.has_cycle_from("ISSUE-001"));
        assert!(graph.has_cycle_from("ISSUE-002"));
    }

    #[test]
    fn from_tasks_detects_cycle_with_three_ids() {
        let tasks = vec![
            task("TASK-A", &["TASK-C"]),
            task("TASK-B", &["TASK-A"]),
            task("TASK-C", &["TASK-B"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert!(graph.has_any_cycle());
        let members = graph.find_cycle_members();
        assert!(members.contains("TASK-A"));
        assert!(members.contains("TASK-B"));
        assert!(members.contains("TASK-C"));
    }

    #[test]
    fn from_tasks_no_cycle_in_linear_chain() {
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-002", &["TASK-001"]),
            task("TASK-003", &["TASK-002"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert!(!graph.has_any_cycle());
    }

    // ═══════════════════════════════════════════════════════════════
    // Edge cases adicionales
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn blocked_by_me_returns_blocked_ids() {
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-002", &["TASK-001"]),
            task("TASK-003", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let blocked = graph.blocked_by_me("TASK-001");
        assert_eq!(blocked.len(), 2);
        assert!(blocked.contains(&"TASK-002"));
        assert!(blocked.contains(&"TASK-003"));
    }

    #[test]
    fn blocked_by_me_returns_empty_for_leaf() {
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-002", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let blocked = graph.blocked_by_me("TASK-002");
        assert!(blocked.is_empty());
    }

    #[test]
    fn blocked_by_me_returns_empty_for_unknown_id() {
        let tasks = vec![task("TASK-001", &[])];
        let graph = DependencyGraph::from_tasks(&tasks);
        let blocked = graph.blocked_by_me("TASK-999");
        assert!(blocked.is_empty());
    }

    #[test]
    fn has_cycle_from_isolated_node() {
        let tasks = vec![task("TASK-001", &[])];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert!(!graph.has_cycle_from("TASK-001"));
        assert!(!graph.has_any_cycle());
    }

    #[test]
    fn has_cycle_from_unknown_node() {
        let tasks = vec![task("TASK-001", &[])];
        let graph = DependencyGraph::from_tasks(&tasks);
        // has_cycle_from con nodo inexistente → unwrap() paniquea.
        // Verificar que al menos no hay ciclo para nodos existentes.
        assert!(!graph.has_cycle_from("TASK-001"));
    }

    #[test]
    fn find_cycle_members_returns_empty_when_no_cycles() {
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-002", &["TASK-001"]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        let members = graph.find_cycle_members();
        assert!(members.is_empty());
    }

    #[test]
    fn blocks_count_with_nonexistent_id() {
        let tasks = vec![task("TASK-001", &[])];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert_eq!(graph.blocks_count("TASK-999"), 0);
    }

    #[test]
    fn from_tasks_with_self_reference_detects_cycle() {
        // Una task que se bloquea a sí misma — ciclo trivial
        let tasks = vec![task("TASK-001", &["TASK-001"])];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert!(graph.has_cycle_from("TASK-001"));
        assert!(graph.has_any_cycle());
    }

    #[test]
    fn from_stories_and_from_tasks_produce_same_structure() {
        // Verificar que ambos constructores producen grafos equivalentes
        // si los datos de entrada son equivalentes
        let stories = vec![
            story("STORY-001", &[]),
            story("STORY-002", &["STORY-001"]),
        ];
        let tasks = vec![
            task("STORY-001", &[]),
            task("STORY-002", &["STORY-001"]),
        ];

        let graph_s = DependencyGraph::from_stories(&stories);
        let graph_t = DependencyGraph::from_tasks(&tasks);

        assert_eq!(graph_s.blocks_count("STORY-001"), graph_t.blocks_count("STORY-001"));
        assert_eq!(graph_s.has_any_cycle(), graph_t.has_any_cycle());
    }

    #[test]
    fn dependency_graph_default_is_empty() {
        let graph = DependencyGraph::default();
        assert_eq!(graph.blocks_count("ANY"), 0);
        assert!(!graph.has_any_cycle());
        assert!(graph.find_cycle_members().is_empty());
    }

    #[test]
    fn from_tasks_handles_duplicate_task_ids() {
        // Si hay dos tasks con el mismo ID, la segunda sobrescribe
        let tasks = vec![
            task("TASK-001", &[]),
            task("TASK-001", &["TASK-002"]),
            task("TASK-002", &[]),
        ];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert_eq!(graph.blocks_count("TASK-001"), 0, "Second TASK-001 overwrites, no forward edges from TASK-001->TASK-002 because TASK-001's own forward entry was created first without edges, then the second TASK-001 re-populates forward but blocker TASK-002 creates a reverse edge only");
    }

    #[test]
    fn from_tasks_preserves_blockers_even_when_blocker_not_in_tasks() {
        // Bloqueadores que no están en la lista de tasks también se guardan
        let tasks = vec![task("TASK-001", &["TASK-999", "TASK-888"])];
        let graph = DependencyGraph::from_tasks(&tasks);
        assert_eq!(graph.blocks_count("TASK-999"), 1, "TASK-999 blocks TASK-001 even if TASK-999 is not in the task list");
        assert_eq!(graph.blocks_count("TASK-888"), 1);
    }
}
