//! Dashboard de historias (`regista board`).
//!
//! Muestra un tablero Kanban con el conteo de historias por estado,
//! y lista las que están bloqueadas o fallidas con detalle.
//!
//! Diseñado para ser resistente a #04 (workflows configurables):
//! trabaja con claves string (`status.to_string()`) en vez de acoplarse
//! a las variantes del enum `Status`. Cuando los estados pasen a ser
//! dinámicos, este módulo apenas necesitará cambios.

use crate::app::pipeline;
use crate::config::Config;
use crate::domain::story::Story;
use crate::domain::workflow::{CanonicalWorkflow, Workflow};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Datos agregados del tablero de historias.
#[derive(Debug, Clone, Serialize)]
pub struct BoardData {
    /// Conteo de historias por estado (clave = representación string del estado).
    pub counts: HashMap<String, usize>,
    /// Total de historias cargadas.
    pub total: usize,
    /// Historias bloqueadas, con sus dependencias.
    pub blocked: Vec<BlockedStory>,
    /// Historias fallidas, con el motivo del último rechazo.
    pub failed: Vec<FailedStory>,
}

/// Una historia bloqueada y qué la bloquea.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedStory {
    pub id: String,
    /// IDs de las historias que bloquean a esta.
    pub blocked_by: Vec<String>,
}

/// Una historia fallida y el motivo.
#[derive(Debug, Clone, Serialize)]
pub struct FailedStory {
    pub id: String,
    /// Motivo del último rechazo (del Activity Log), si está disponible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Ejecuta el comando `board`.
///
/// Carga todas las historias, construye el `BoardData` y lo imprime
/// en formato humano o JSON.
pub fn run(
    project_root: &Path,
    json: bool,
    epic_filter: Option<&str>,
    config_path: Option<&Path>,
) -> anyhow::Result<()> {
    let cfg = Config::load(project_root, config_path)?;
    let mut stories = pipeline::load_all_stories(project_root, &cfg)?;

    // Filtrar por épica si se especifica
    if let Some(epic) = epic_filter {
        stories.retain(|s| s.epic.as_ref().is_some_and(|e| e == epic));
    }

    let data = build_board_data(&stories);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".into())
        );
    } else {
        print_human(&data, &CanonicalWorkflow);
    }

    Ok(())
}

/// Construye el `BoardData` a partir de una lista de historias.
fn build_board_data(stories: &[Story]) -> BoardData {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut blocked: Vec<BlockedStory> = Vec::new();
    let mut failed: Vec<FailedStory> = Vec::new();

    for story in stories {
        let status_key = story.status.to_string();
        *counts.entry(status_key.clone()).or_default() += 1;

        // Estados especiales: usamos la clave string, no el enum.
        // Cuando #04 llegue, estos literales se leerán de la config.
        match status_key.as_str() {
            "Blocked" => {
                blocked.push(BlockedStory {
                    id: story.id.clone(),
                    blocked_by: story.blockers.clone(),
                });
            }
            "Failed" => {
                failed.push(FailedStory {
                    id: story.id.clone(),
                    reason: story.last_rejection.clone(),
                });
            }
            _ => {}
        }
    }

    // Ordenar por ID numérico para salida predecible
    blocked.sort_by_key(|s| extract_numeric(&s.id));
    failed.sort_by_key(|s| extract_numeric(&s.id));

    BoardData {
        total: stories.len(),
        counts,
        blocked,
        failed,
    }
}

/// Renderiza el tablero a un `String` usando el orden de columnas del workflow.
///
/// - Obtiene columnas de `workflow.canonical_column_order()`  (CA2)
/// - Omite columnas con count = 0                             (CA3)
/// - Formatea igual que la versión hardcodeada actual          (CA4)
fn render_board(data: &BoardData, workflow: &dyn Workflow) -> String {
    let mut output = String::new();

    // Cabecera
    output.push_str("📊 Story Board — regista\n");
    output.push_str("==========================\n");
    output.push('\n');

    // Columnas en orden del workflow, omitiendo las vacías
    let column_order = workflow.canonical_column_order();
    let has_visible = column_order
        .iter()
        .any(|c| data.counts.get(*c).copied().unwrap_or(0) > 0);

    if has_visible {
        for col in column_order {
            let count = data.counts.get(*col).copied().unwrap_or(0);
            if count > 0 {
                output.push_str(&format!("  {col:<18} {count:>3}\n"));
            }
        }
        output.push_str(&format!("  {}\n", "─".repeat(22)));
    }

    // Total
    output.push_str(&format!("  {:<18} {:>3}\n", "Total", data.total));
    output.push('\n');

    // Bloqueadas
    if !data.blocked.is_empty() {
        output.push_str(&format!("🔴 Blocked ({}):\n", data.blocked.len()));
        for bs in &data.blocked {
            let blockers = bs.blocked_by.join(", ");
            output.push_str(&format!("  {} — blocked by: {}\n", bs.id, blockers));
        }
        output.push('\n');
    }

    // Fallidas
    if !data.failed.is_empty() {
        output.push_str(&format!("❌ Failed ({}):\n", data.failed.len()));
        for fs in &data.failed {
            match &fs.reason {
                Some(reason) => output.push_str(&format!("  {} — {}\n", fs.id, reason)),
                None => output.push_str(&format!("  {} — (sin motivo registrado)\n", fs.id)),
            }
        }
        output.push('\n');
    }

    output
}

/// Imprime el tablero en formato legible por humanos,
/// usando el orden de columnas definido por el workflow.
fn print_human(data: &BoardData, workflow: &dyn Workflow) {
    let rendered = render_board(data, workflow);
    print!("{rendered}");
}

/// Extrae el número de un ID tipo "STORY-NNN".
fn extract_numeric(id: &str) -> u32 {
    id.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::state::Status;

    /// Construye una Story sintética para tests.
    fn fake_story(
        id: &str,
        status: Status,
        epic: Option<&str>,
        blockers: &[&str],
        last_rejection: Option<&str>,
    ) -> Story {
        Story {
            id: id.to_string(),
            path: format!("stories/{id}.md").into(),
            status,
            epic: epic.map(|s| s.to_string()),
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            last_rejection: last_rejection.map(|s| s.to_string()),
            raw_content: String::new(),
        }
    }

    #[test]
    fn build_board_counts_correctly() {
        let stories = vec![
            fake_story("STORY-001", Status::Draft, None, &[], None),
            fake_story("STORY-002", Status::Ready, None, &[], None),
            fake_story("STORY-003", Status::Done, None, &[], None),
            fake_story("STORY-004", Status::Done, None, &[], None),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.total, 4);
        assert_eq!(data.counts.get("Draft").copied().unwrap_or(0), 1);
        assert_eq!(data.counts.get("Ready").copied().unwrap_or(0), 1);
        assert_eq!(data.counts.get("Done").copied().unwrap_or(0), 2);
        assert!(data.blocked.is_empty());
        assert!(data.failed.is_empty());
    }

    #[test]
    fn build_board_lists_blocked_stories() {
        let stories = vec![
            fake_story("STORY-001", Status::Done, None, &[], None),
            fake_story("STORY-002", Status::Blocked, None, &["STORY-001"], None),
            fake_story(
                "STORY-003",
                Status::Blocked,
                None,
                &["STORY-001", "STORY-002"],
                None,
            ),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.blocked.len(), 2);
        assert_eq!(data.blocked[0].id, "STORY-002");
        assert_eq!(data.blocked[0].blocked_by, vec!["STORY-001"]);
        assert_eq!(data.blocked[1].id, "STORY-003");
        assert_eq!(data.blocked[1].blocked_by, vec!["STORY-001", "STORY-002"]);
        assert!(data.failed.is_empty());
    }

    #[test]
    fn build_board_lists_failed_stories_with_reason() {
        let stories = vec![
            fake_story(
                "STORY-001",
                Status::Failed,
                None,
                &[],
                Some("max reject cycles (8/8)"),
            ),
            fake_story("STORY-002", Status::Failed, None, &[], None),
            fake_story("STORY-003", Status::Done, None, &[], None),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.failed.len(), 2);
        assert_eq!(data.failed[0].id, "STORY-001");
        assert_eq!(
            data.failed[0].reason.as_deref(),
            Some("max reject cycles (8/8)")
        );
        assert_eq!(data.failed[1].id, "STORY-002");
        assert_eq!(data.failed[1].reason, None);
        assert!(data.blocked.is_empty());
    }

    #[test]
    fn build_board_handles_empty_list() {
        let stories: Vec<Story> = vec![];
        let data = build_board_data(&stories);

        assert_eq!(data.total, 0);
        assert!(data.counts.is_empty());
        assert!(data.blocked.is_empty());
        assert!(data.failed.is_empty());
    }

    #[test]
    fn build_board_sorts_by_numeric_id() {
        let stories = vec![
            fake_story("STORY-010", Status::Blocked, None, &["STORY-005"], None),
            fake_story("STORY-002", Status::Blocked, None, &["STORY-001"], None),
            fake_story("STORY-005", Status::Blocked, None, &["STORY-003"], None),
        ];

        let data = build_board_data(&stories);

        assert_eq!(data.blocked.len(), 3);
        assert_eq!(data.blocked[0].id, "STORY-002");
        assert_eq!(data.blocked[1].id, "STORY-005");
        assert_eq!(data.blocked[2].id, "STORY-010");
    }

    // ═══════════════════════════════════════════════════════════════
    // Tests para STORY-009: columnas dinámicas según workflow
    // ═══════════════════════════════════════════════════════════════

    /// CA1: `render_board` acepta `&dyn Workflow` como parámetro.
    /// La mera compilación de este test satisface CA1:
    /// si el trait no fuera object-safe o la firma no aceptara `&dyn Workflow`,
    /// este test no compilaría.
    #[test]
    fn render_board_accepts_dyn_workflow() {
        let wf: &dyn Workflow = &CanonicalWorkflow;
        let data = BoardData {
            counts: HashMap::new(),
            total: 0,
            blocked: vec![],
            failed: vec![],
        };
        // Debe compilar y aceptar un trait object como parámetro
        let _output = render_board(&data, wf);
    }

    /// CA2: El orden de columnas se obtiene de `workflow.canonical_column_order()`.
    ///
    /// Usamos `CanonicalWorkflow` y verificamos que las columnas en la salida
    /// respetan el orden exacto devuelto por `canonical_column_order()`.
    #[test]
    fn column_order_comes_from_workflow() {
        let wf = CanonicalWorkflow;
        let expected_order = wf.canonical_column_order();

        let mut counts = HashMap::new();
        // Poblar todas las columnas con al menos 1 para que ninguna se omita
        for col in expected_order {
            counts.insert(col.to_string(), 1);
        }

        let data = BoardData {
            counts,
            total: expected_order.len(),
            blocked: vec![],
            failed: vec![],
        };

        let output = render_board(&data, &wf as &dyn Workflow);

        // Cada columna debe aparecer en el orden definido por canonical_column_order()
        let positions: Vec<Option<usize>> =
            expected_order.iter().map(|col| output.find(col)).collect();

        for i in 1..positions.len() {
            match (positions[i - 1], positions[i]) {
                (Some(prev), Some(curr)) => {
                    assert!(
                        prev < curr,
                        "'{}' debería aparecer antes que '{}' en la salida",
                        expected_order[i - 1],
                        expected_order[i]
                    );
                }
                _ => panic!(
                    "Columna '{}' o '{}' no encontrada en la salida",
                    expected_order[i - 1],
                    expected_order[i]
                ),
            }
        }
    }

    /// CA3: Las columnas sin historias (count = 0) se omiten en la salida.
    #[test]
    fn empty_columns_are_skipped() {
        let wf = CanonicalWorkflow;
        let mut counts = HashMap::new();
        counts.insert("Draft".into(), 1);
        counts.insert("Done".into(), 2);
        // El resto (Ready, Tests Ready, In Progress, In Review,
        // Business Review, Blocked, Failed) tienen count = 0 implícito

        let data = BoardData {
            counts,
            total: 3,
            blocked: vec![],
            failed: vec![],
        };

        let output = render_board(&data, &wf as &dyn Workflow);

        // Columnas con count > 0 sí aparecen
        assert!(
            output.contains("Draft"),
            "Draft (count=1) debería aparecer en la salida"
        );
        assert!(
            output.contains("Done"),
            "Done (count=2) debería aparecer en la salida"
        );

        // Columnas con count = 0 NO aparecen
        for state in &[
            "Ready",
            "Tests Ready",
            "In Progress",
            "In Review",
            "Business Review",
            "Blocked",
            "Failed",
        ] {
            assert!(
                !output.contains(state),
                "'{state}' (count=0) NO debería aparecer en la salida"
            );
        }
    }

    /// CA3 (borde): Si TODAS las columnas están vacías, no se muestra ninguna.
    #[test]
    fn all_empty_columns_shows_none() {
        let wf = CanonicalWorkflow;
        let data = BoardData {
            counts: HashMap::new(),
            total: 0,
            blocked: vec![],
            failed: vec![],
        };

        let output = render_board(&data, &wf as &dyn Workflow);

        // Ninguna columna del workflow canónico debe aparecer
        for col in wf.canonical_column_order() {
            assert!(
                !output.contains(col),
                "Columna '{col}' con count=0 NO debería aparecer"
            );
        }
    }

    /// CA4: La salida para `CanonicalWorkflow` es visualmente idéntica a la actual.
    ///
    /// Verifica que:
    /// - Las columnas respetan el orden canónico
    /// - La línea de total está presente con el valor correcto
    /// - Las secciones de bloqueadas/fallidas se renderizan correctamente
    #[test]
    fn canonical_workflow_output_matches_current_behavior() {
        let wf = CanonicalWorkflow;
        let mut counts = HashMap::new();
        counts.insert("Draft".into(), 2);
        counts.insert("Ready".into(), 1);
        counts.insert("Done".into(), 3);

        let data = BoardData {
            counts,
            total: 6,
            blocked: vec![BlockedStory {
                id: "STORY-005".into(),
                blocked_by: vec!["STORY-001".into()],
            }],
            failed: vec![FailedStory {
                id: "STORY-007".into(),
                reason: Some("rechazada 3 veces".into()),
            }],
        };

        let output = render_board(&data, &wf as &dyn Workflow);

        // Cabecera tradicional
        assert!(
            output.contains("Story Board"),
            "Debe contener la cabecera 'Story Board'"
        );
        assert!(output.contains("regista"), "Debe contener 'regista'");

        // Columnas con contenido en orden canónico
        let draft_pos = output.find("Draft").expect("Draft no encontrado");
        let ready_pos = output.find("Ready").expect("Ready no encontrado");
        let done_pos = output.find("Done").expect("Done no encontrado");
        assert!(draft_pos < ready_pos, "Draft debe aparecer antes que Ready");
        assert!(ready_pos < done_pos, "Ready debe aparecer antes que Done");

        // Línea de total
        assert!(output.contains("Total"), "Debe mostrar 'Total'");
        assert!(output.contains("6"), "El total debe ser 6");

        // Sección de bloqueadas
        assert!(
            output.contains("Blocked"),
            "Debe mostrar la sección de bloqueadas"
        );
        assert!(output.contains("STORY-005"), "Debe listar STORY-005");
        assert!(
            output.contains("STORY-001"),
            "Debe mostrar la dependencia STORY-001"
        );

        // Sección de fallidas
        assert!(
            output.contains("Failed"),
            "Debe mostrar la sección de fallidas"
        );
        assert!(output.contains("STORY-007"), "Debe listar STORY-007");
        assert!(
            output.contains("rechazada 3 veces"),
            "Debe mostrar el motivo de rechazo"
        );
    }

    /// CA6: Si se pasa un workflow hipotético con solo 5 columnas,
    /// el board muestra exactamente esas 5 columnas (test unitario).
    #[test]
    fn custom_5_column_workflow_shows_exactly_those_columns() {
        struct FiveColumnWorkflow;

        impl Workflow for FiveColumnWorkflow {
            fn next_status(&self, current: Status) -> Status {
                current
            }
            fn map_status_to_role(&self, _status: Status) -> &'static str {
                "product_owner"
            }
            fn canonical_column_order(&self) -> &[&'static str] {
                &["Alpha", "Beta", "Gamma", "Delta", "Omega"]
            }
        }

        let wf = FiveColumnWorkflow;
        let mut counts = HashMap::new();
        for col in wf.canonical_column_order() {
            counts.insert(col.to_string(), 1);
        }

        let data = BoardData {
            counts,
            total: 5,
            blocked: vec![],
            failed: vec![],
        };

        let output = render_board(&data, &wf as &dyn Workflow);

        // Las 5 columnas aparecen
        assert!(output.contains("Alpha"));
        assert!(output.contains("Beta"));
        assert!(output.contains("Gamma"));
        assert!(output.contains("Delta"));
        assert!(output.contains("Omega"));

        // En el orden exacto definido por el workflow
        let alpha = output.find("Alpha").unwrap();
        let beta = output.find("Beta").unwrap();
        let gamma = output.find("Gamma").unwrap();
        let delta = output.find("Delta").unwrap();
        let omega = output.find("Omega").unwrap();
        assert!(alpha < beta);
        assert!(beta < gamma);
        assert!(gamma < delta);
        assert!(delta < omega);

        // Ninguna columna del workflow canónico se cuela
        assert!(!output.contains("Draft"));
        assert!(!output.contains("Ready"));
        assert!(!output.contains("Done"));
        assert!(!output.contains("Blocked"));
        assert!(!output.contains("Failed"));
    }

    /// CA6 + CA3 combinados: workflow hipotético con columnas vacías
    /// también omite las columnas count=0.
    #[test]
    fn custom_workflow_skips_empty_columns() {
        struct SparseWorkflow;

        impl Workflow for SparseWorkflow {
            fn next_status(&self, current: Status) -> Status {
                current
            }
            fn map_status_to_role(&self, _status: Status) -> &'static str {
                "product_owner"
            }
            fn canonical_column_order(&self) -> &[&'static str] {
                &["P1", "P2", "P3", "P4", "P5"]
            }
        }

        let wf = SparseWorkflow;
        let mut counts = HashMap::new();
        counts.insert("P1".into(), 1);
        counts.insert("P3".into(), 2);
        counts.insert("P5".into(), 1);
        // P2 y P4 count = 0

        let data = BoardData {
            counts,
            total: 4,
            blocked: vec![],
            failed: vec![],
        };

        let output = render_board(&data, &wf as &dyn Workflow);

        assert!(output.contains("P1"), "P1 (count=1) debe aparecer");
        assert!(output.contains("P3"), "P3 (count=2) debe aparecer");
        assert!(output.contains("P5"), "P5 (count=1) debe aparecer");
        assert!(!output.contains("P2"), "P2 (count=0) debe omitirse");
        assert!(!output.contains("P4"), "P4 (count=0) debe omitirse");

        // El orden relativo se preserva entre las columnas visibles
        let p1 = output.find("P1").unwrap();
        let p3 = output.find("P3").unwrap();
        let p5 = output.find("P5").unwrap();
        assert!(p1 < p3, "P1 debe aparecer antes que P3");
        assert!(p3 < p5, "P3 debe aparecer antes que P5");
    }

    // ═══════════════════════════════════════════════════════════════
    // Tests existentes (pre-STORY-009)
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn board_data_serializes_to_json() {
        let mut counts = HashMap::new();
        counts.insert("Done".into(), 3usize);
        counts.insert("Draft".into(), 2usize);

        let data = BoardData {
            counts,
            total: 5,
            blocked: vec![BlockedStory {
                id: "STORY-002".into(),
                blocked_by: vec!["STORY-001".into()],
            }],
            failed: vec![FailedStory {
                id: "STORY-005".into(),
                reason: Some("rechazada 3 veces".into()),
            }],
        };

        let json = serde_json::to_string_pretty(&data).unwrap();
        assert!(json.contains("\"Done\": 3"));
        assert!(json.contains("\"Draft\": 2"));
        assert!(json.contains("\"total\": 5"));
        assert!(json.contains("STORY-002"));
        assert!(json.contains("STORY-005"));
        assert!(json.contains("rechazada 3 veces"));
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-V10-016: BoardData::from_tasks con columnas dinámicas
    // ═══════════════════════════════════════════════════════════════
    //
    // NOTA TDD: Estos tests verifican que BoardData puede construirse
    // desde Task (genérico) además de Story (legacy). El Developer debe
    // implementar `BoardData::from_tasks()` y adaptar `board.rs`.

    use crate::domain::task::Task;
    use std::collections::HashMap as TaskFields;
    use std::path::PathBuf;

    fn make_task(id: &str, status: &str, blockers: &[&str], epic: Option<&str>) -> Task {
        let mut fields = TaskFields::new();
        fields.insert("status".to_string(), status.to_string());
        if let Some(epic) = epic {
            fields.insert("epic".to_string(), epic.to_string());
        }
        Task {
            id: id.to_string(),
            path: PathBuf::from(format!("tasks/{id}.md")),
            fields,
            blockers: blockers.iter().map(|s| s.to_string()).collect(),
            activity_log: vec![],
            raw_content: String::new(),
        }
    }

    fn make_dynamic_workflow() -> impl Workflow {
        struct DynamicColumns([&'static str; 6]);
        impl Workflow for DynamicColumns {
            fn next_status(&self, current: Status) -> Status { current }
            fn map_status_to_role(&self, _status: Status) -> &'static str { "developer" }
            fn canonical_column_order(&self) -> &[&'static str] { &self.0 }
        }
        DynamicColumns(["draft", "ready", "review", "done", "blocked", "failed"])
    }

    // ── CA1: Columnas siguen orden topológico del workflow ─────────

    #[test]
    fn from_tasks_groups_by_status() {
        // CA1: BoardData agrupa tasks por su status
        let tasks = vec![
            make_task("TASK-001", "draft", &[], None),
            make_task("TASK-002", "draft", &[], None),
            make_task("TASK-003", "ready", &[], None),
            make_task("TASK-004", "review", &[], None),
            make_task("TASK-005", "done", &[], None),
        ];

        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);

        assert_eq!(data.total, 5);
        assert_eq!(data.counts.get("draft").copied().unwrap_or(0), 2);
        assert_eq!(data.counts.get("ready").copied().unwrap_or(0), 1);
        assert_eq!(data.counts.get("review").copied().unwrap_or(0), 1);
        assert_eq!(data.counts.get("done").copied().unwrap_or(0), 1);
    }

    #[test]
    fn from_tasks_columns_in_workflow_order() {
        // CA1: Las columnas deben seguir el orden del workflow (topológico)
        let tasks = vec![
            make_task("TASK-001", "done", &[], None),
            make_task("TASK-002", "draft", &[], None),
            make_task("TASK-003", "ready", &[], None),
            make_task("TASK-004", "review", &[], None),
        ];

        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);

        let output = render_board(&data, &wf as &dyn Workflow);

        // Verificar orden: draft → ready → review → done
        let draft_pos = output.find("draft").unwrap();
        let ready_pos = output.find("ready").unwrap();
        let review_pos = output.find("review").unwrap();
        let done_pos = output.find("done").unwrap();

        assert!(draft_pos < ready_pos, "draft debe aparecer antes que ready");
        assert!(ready_pos < review_pos, "ready debe aparecer antes que review");
        assert!(review_pos < done_pos, "review debe aparecer antes que done");
    }

    #[test]
    fn from_tasks_blocked_and_failed_at_end() {
        // CA1: Estados terminales (blocked, failed) al final
        let tasks = vec![
            make_task("TASK-001", "draft", &[], None),
            make_task("TASK-002", "blocked", &["TASK-001"], None),
            make_task("TASK-003", "failed", &[], None),
            make_task("TASK-004", "done", &[], None),
        ];

        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);
        let output = render_board(&data, &wf as &dyn Workflow);

        let done_pos = output.find("done").unwrap();
        let blocked_pos = output.find("blocked").unwrap();
        let failed_pos = output.find("failed").unwrap();

        assert!(done_pos < blocked_pos, "done debe aparecer antes que blocked");
        assert!(done_pos < failed_pos, "done debe aparecer antes que failed");
    }

    // ── CA2: Estados sin tareas se omiten ─────────────────────────

    #[test]
    fn from_tasks_omits_empty_columns() {
        // CA2: Estados sin tasks NO aparecen en la salida
        let tasks = vec![
            make_task("TASK-001", "draft", &[], None),
            make_task("TASK-002", "done", &[], None),
        ];

        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);
        let output = render_board(&data, &wf as &dyn Workflow);

        assert!(output.contains("draft"), "draft debe aparecer");
        assert!(output.contains("done"), "done debe aparecer");
        assert!(!output.contains("ready"), "ready (count=0) debe omitirse");
        assert!(!output.contains("review"), "review (count=0) debe omitirse");
    }

    #[test]
    fn board_omits_arbitrary_state_like_validating() {
        // CA2: El Gherkin especifica el estado "validating" como ejemplo
        // de un estado definido en el workflow pero sin tareas.
        // Debemos verificar que el mecanismo funciona con cualquier estado.

        struct ValidatingWorkflow;
        impl Workflow for ValidatingWorkflow {
            fn next_status(&self, current: Status) -> Status { current }
            fn map_status_to_role(&self, _status: Status) -> &'static str { "developer" }
            fn canonical_column_order(&self) -> &[&'static str] {
                &["draft", "validating", "review", "done"]
            }
        }

        let tasks = vec![
            make_task("TASK-001", "draft", &[], None),
            make_task("TASK-002", "review", &[], None),
            make_task("TASK-003", "done", &[], None),
        ];
        // "validating" está en el workflow pero ninguna task lo usa

        let wf = ValidatingWorkflow;
        let data = build_board_data_from_tasks(&tasks, &wf);
        let output = render_board(&data, &wf as &dyn Workflow);

        assert!(output.contains("draft"), "draft debe aparecer");
        assert!(output.contains("review"), "review debe aparecer");
        assert!(output.contains("done"), "done debe aparecer");
        assert!(
            !output.contains("validating"),
            "validating (count=0) NO debe aparecer en la salida"
        );
    }

    // ── CA3: --json emite estructura con columns, tasks, summary ──

    #[test]
    fn from_tasks_board_data_json_has_columns_tasks_summary() {
        // CA3: JSON debe tener estructura con total, columnas, blocked/failed
        let tasks = vec![
            make_task("TASK-001", "draft", &[], None),
            make_task("TASK-002", "review", &[], None),
            make_task("TASK-003", "done", &[], None),
            make_task("TASK-004", "done", &[], None),
            make_task("TASK-005", "blocked", &["TASK-001"], None),
        ];

        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);
        let json = serde_json::to_string_pretty(&data).unwrap();

        // Verificar presencia de total
        assert!(json.contains("\"total\""), "JSON debe incluir total");
        assert!(json.contains("5"), "total debe ser 5");

        // Verificar presencia de columnas con conteo
        assert!(json.contains("\"draft\""), "JSON debe incluir columna draft");
        assert!(json.contains("\"review\""), "JSON debe incluir columna review");
        assert!(json.contains("\"done\""), "JSON debe incluir columna done");

        // Verificar blocked
        assert!(json.contains("\"blocked\""), "JSON debe incluir array blocked");
        assert!(json.contains("TASK-005"), "JSON debe listar TASK-005 en blocked");

        // NOTA TDD: El Gherkin dice que 'columns' debe ser un array ordenado
        // según el workflow. Con HashMap esto no se garantiza. El Developer
        // debe añadir un campo `columns_order: Vec<String>` a BoardData o
        // usar IndexMap para preservar el orden.
        assert_eq!(data.total, 5);
    }

    // ── CA3: --epic filtra tareas por campo epic ──────────────────

    #[test]
    fn from_tasks_filters_by_epic() {
        // CA3: Filtrado por épica usando el campo definido en section_markers
        let tasks = vec![
            make_task("TASK-001", "draft", &[], Some("EPIC-ALFA")),
            make_task("TASK-002", "draft", &[], Some("EPIC-BETA")),
            make_task("TASK-003", "done", &[], Some("EPIC-ALFA")),
            make_task("TASK-004", "ready", &[], None),
        ];

        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);

        // Filtrar por EPIC-ALFA (el Developer debe implementar filter_by_epic)
        let alfa_tasks: Vec<&Task> = tasks.iter()
            .filter(|t| t.fields.get("epic").map(|e| e.as_str()) == Some("EPIC-ALFA"))
            .collect();

        assert_eq!(alfa_tasks.len(), 2, "EPIC-ALFA debe tener 2 tareas");
        assert!(alfa_tasks.iter().any(|t| t.id == "TASK-001"));
        assert!(alfa_tasks.iter().any(|t| t.id == "TASK-003"));
    }

    #[test]
    fn from_tasks_without_epic_field_all_tasks_included() {
        // CA3: Si no hay campo epic en task_format, no se filtra
        let tasks = vec![
            make_task("TASK-001", "draft", &[], None),
            make_task("TASK-002", "done", &[], None),
        ];

        // Sin epic_filter, todas las tasks deben incluirse
        let wf = make_dynamic_workflow();
        let data = build_board_data_from_tasks(&tasks, &wf);
        assert_eq!(data.total, 2);
    }

    // ── Función auxiliar temporal (el Developer la hará pública) ──
    //
    // NOTA TDD: build_board_data_from_tasks() no existe aún. El Developer debe
    // implementar esta función en app/board.rs. Este helper temporal permite
    // que los tests compilen AHORA pero debe ser reemplazado por la
    // implementación real.
    fn build_board_data_from_tasks(tasks: &[Task], workflow: &dyn Workflow) -> BoardData {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut blocked: Vec<BlockedStory> = Vec::new();
        let mut failed: Vec<FailedStory> = Vec::new();

        for task in tasks {
            let status_key = task.fields.get("status").cloned().unwrap_or_default();
            *counts.entry(status_key.clone()).or_default() += 1;

            match status_key.as_str() {
                "blocked" => {
                    blocked.push(BlockedStory {
                        id: task.id.clone(),
                        blocked_by: task.blockers.clone(),
                    });
                }
                "failed" => {
                    failed.push(FailedStory {
                        id: task.id.clone(),
                        reason: task.last_rejection().map(|s| s.to_string()),
                    });
                }
                _ => {}
            }
        }

        blocked.sort_by_key(|s| extract_numeric(&s.id));
        failed.sort_by_key(|s| extract_numeric(&s.id));

        BoardData {
            total: tasks.len(),
            counts,
            blocked,
            failed,
        }
    }
}
