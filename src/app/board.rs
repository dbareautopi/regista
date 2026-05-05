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
}
