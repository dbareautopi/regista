//! Trait `Workflow` e implementación canónica.
//!
//! Encapsula las decisiones de la máquina de estados:
//! - `next_status`: el estado esperado tras la intervención del agente
//! - `map_status_to_role`: el rol canónico que procesa un estado
//! - `canonical_column_order`: orden visual de columnas en el board
//!
//! La implementación `CanonicalWorkflow` replica el comportamiento actual
//! hardcodeado en `pipeline.rs` y `board.rs`.

use crate::domain::state::Status;

/// Trait que define el comportamiento del workflow.
///
/// Cada método recibe `&self` (no `&mut self`) porque los workflows
/// son inmutables durante la ejecución.
#[allow(dead_code)]
pub trait Workflow {
    /// Infiere el estado esperado tras la intervención del agente
    /// para el estado `current`.
    fn next_status(&self, current: Status) -> Status;

    /// Mapea un estado al rol canónico que lo procesa.
    /// Retorna nombres como `"product_owner"`, `"qa_engineer"`, etc.
    fn map_status_to_role(&self, status: Status) -> &'static str;

    /// Orden canónico de columnas para visualización (board / dashboard).
    fn canonical_column_order(&self) -> &[&'static str];
}

/// Implementación del workflow canónico con las 14 transiciones fijas.
///
/// Replica exactamente el comportamiento hardcodeado en `pipeline.rs`:
/// - `next_status()` ≡ `pipeline::next_status()`
/// - `map_status_to_role()` ≡ `pipeline::map_status_to_role()`
/// - `canonical_column_order()` ≡ orden usado en `board.rs`
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalWorkflow;

impl Workflow for CanonicalWorkflow {
    fn next_status(&self, current: Status) -> Status {
        match current {
            Status::Draft => Status::Ready,
            Status::Ready => Status::TestsReady,
            Status::TestsReady => Status::InReview,
            Status::InProgress => Status::InReview,
            Status::InReview => Status::BusinessReview,
            Status::BusinessReview => Status::Done,
            _ => current,
        }
    }

    fn map_status_to_role(&self, status: Status) -> &'static str {
        match status {
            Status::Draft | Status::BusinessReview => "product_owner",
            Status::Ready => "qa_engineer",
            Status::TestsReady | Status::InProgress => "developer",
            Status::InReview => "reviewer",
            _ => "product_owner",
        }
    }

    fn canonical_column_order(&self) -> &[&'static str] {
        &[
            "Draft",
            "Ready",
            "Tests Ready",
            "In Progress",
            "In Review",
            "Business Review",
            "Done",
            "Blocked",
            "Failed",
        ]
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // CA1: El trait Workflow existe con los 3 métodos requeridos
    // ====================================================================
    // Estos tests verifican implícitamente CA1 al compilar:
    // si el trait no existiera o le faltara un método, no compilarían.

    /// Verifica que se puede usar `CanonicalWorkflow` como `&dyn Workflow`.
    /// Esto prueba que el trait existe (CA1) y que el struct lo implementa (CA2).
    ///
    /// NOTA TDD: este test falla ahora (todo!() panic) y pasará cuando
    /// el Developer implemente `next_status`, `map_status_to_role` y
    /// `canonical_column_order` correctamente.
    #[test]
    fn canonical_workflow_can_be_used_as_trait_object() {
        let wf: &dyn Workflow = &CanonicalWorkflow;
        // CA1: el trait existe y expone next_status
        assert_eq!(wf.next_status(Status::Draft), Status::Ready);
        // CA1: el trait existe y expone map_status_to_role
        assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
        // CA1: el trait existe y expone canonical_column_order (no vacío)
        assert!(!wf.canonical_column_order().is_empty());
    }

    // ====================================================================
    // CA2: CanonicalWorkflow implementa Workflow
    // ====================================================================
    // Se verifica en los tests concretos de next_status / map_status_to_role.

    /// CA2: CanonicalWorkflow se puede construir sin argumentos.
    #[test]
    fn canonical_workflow_can_be_constructed() {
        let _wf = CanonicalWorkflow;
        let _wf2 = CanonicalWorkflow::default();
    }

    // ====================================================================
    // CA3: CanonicalWorkflow::next_status() ≡ pipeline::next_status()
    // ====================================================================
    // Se comparan contra los outputs esperados de la función actual.
    // (El Developer debe hacer que estos tests pasen.)

    mod next_status {
        use super::*;

        /// Happy path: Draft → Ready → TestsReady → InReview → BusinessReview → Done
        #[test]
        fn happy_path() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.next_status(Status::Ready), Status::TestsReady);
            assert_eq!(wf.next_status(Status::TestsReady), Status::InReview);
            assert_eq!(wf.next_status(Status::InReview), Status::BusinessReview);
            assert_eq!(wf.next_status(Status::BusinessReview), Status::Done);
        }

        /// Fix path: InProgress → InReview
        #[test]
        fn fix_path_in_progress_to_in_review() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::InProgress), Status::InReview);
        }

        /// Estados terminales: Done y Failed se quedan como están
        #[test]
        fn terminal_states_stay() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::Done), Status::Done);
            assert_eq!(wf.next_status(Status::Failed), Status::Failed);
        }

        /// Blocked se queda como está (lo desbloquea el orchestrator, no un agente)
        #[test]
        fn blocked_stays() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.next_status(Status::Blocked), Status::Blocked);
        }

        /// Verifica todos los estados posibles contra la salida esperada
        #[test]
        fn all_states_have_expected_output() {
            let wf = CanonicalWorkflow;
            // Mapa completo de todos los estados → expected next_status
            let expected: &[(Status, Status)] = &[
                (Status::Draft, Status::Ready),
                (Status::Ready, Status::TestsReady),
                (Status::TestsReady, Status::InReview),
                (Status::InProgress, Status::InReview),
                (Status::InReview, Status::BusinessReview),
                (Status::BusinessReview, Status::Done),
                (Status::Done, Status::Done),
                (Status::Blocked, Status::Blocked),
                (Status::Failed, Status::Failed),
            ];

            for (current, expected_next) in expected {
                assert_eq!(
                    wf.next_status(*current),
                    *expected_next,
                    "next_status({current}) debería ser {expected_next}"
                );
            }
        }
    }

    // ====================================================================
    // CA4: CanonicalWorkflow::map_status_to_role() ≡ pipeline::map_status_to_role()
    // ====================================================================

    mod map_status_to_role {
        use super::*;

        #[test]
        fn product_owner_states() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::Draft), "product_owner");
            assert_eq!(
                wf.map_status_to_role(Status::BusinessReview),
                "product_owner"
            );
        }

        #[test]
        fn qa_engineer_state() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
        }

        #[test]
        fn developer_states() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::TestsReady), "developer");
            assert_eq!(wf.map_status_to_role(Status::InProgress), "developer");
        }

        #[test]
        fn reviewer_state() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.map_status_to_role(Status::InReview), "reviewer");
        }

        /// Estados terminales y otros: fallback seguro a "product_owner"
        #[test]
        fn fallback_to_product_owner() {
            let wf = CanonicalWorkflow;
            // Done, Blocked, Failed → "product_owner" (fallback seguro)
            assert_eq!(wf.map_status_to_role(Status::Done), "product_owner");
            assert_eq!(wf.map_status_to_role(Status::Blocked), "product_owner");
            assert_eq!(wf.map_status_to_role(Status::Failed), "product_owner");
        }

        /// Verifica todos los estados contra la salida esperada
        #[test]
        fn all_states_have_expected_role() {
            let wf = CanonicalWorkflow;
            let expected: &[(Status, &str)] = &[
                (Status::Draft, "product_owner"),
                (Status::Ready, "qa_engineer"),
                (Status::TestsReady, "developer"),
                (Status::InProgress, "developer"),
                (Status::InReview, "reviewer"),
                (Status::BusinessReview, "product_owner"),
                (Status::Done, "product_owner"),
                (Status::Blocked, "product_owner"),
                (Status::Failed, "product_owner"),
            ];

            for (status, expected_role) in expected {
                assert_eq!(
                    wf.map_status_to_role(*status),
                    *expected_role,
                    "map_status_to_role({status}) debería ser {expected_role}"
                );
            }
        }
    }

    // ====================================================================
    // CA5: canonical_column_order() devuelve las 9 columnas en orden
    // ====================================================================

    mod canonical_column_order {
        use super::*;

        #[test]
        fn returns_nine_columns_in_correct_order() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            assert_eq!(
                order,
                &[
                    "Draft",
                    "Ready",
                    "Tests Ready",
                    "In Progress",
                    "In Review",
                    "Business Review",
                    "Done",
                    "Blocked",
                    "Failed",
                ]
            );
        }

        #[test]
        fn has_exactly_nine_columns() {
            let wf = CanonicalWorkflow;
            assert_eq!(wf.canonical_column_order().len(), 9);
        }

        #[test]
        fn columns_are_in_priority_order() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            // Done está antes que Blocked/Failed (estados terminales exitosos primero)
            let done_idx = order.iter().position(|&c| c == "Done").unwrap();
            let blocked_idx = order.iter().position(|&c| c == "Blocked").unwrap();
            let failed_idx = order.iter().position(|&c| c == "Failed").unwrap();
            assert!(
                done_idx < blocked_idx,
                "Done debería aparecer antes que Blocked en el orden canónico"
            );
            assert!(
                done_idx < failed_idx,
                "Done debería aparecer antes que Failed en el orden canónico"
            );
        }

        #[test]
        fn draft_is_first_column() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            assert_eq!(order[0], "Draft");
        }

        #[test]
        fn failed_is_last_column() {
            let wf = CanonicalWorkflow;
            let order = wf.canonical_column_order();
            assert_eq!(order[order.len() - 1], "Failed");
        }
    }

    // ====================================================================
    // CA7: El trait usa &self (no &mut self)
    // ====================================================================
    // Este test es compilación: si algún método pidiera &mut self,
    // no podríamos llamarlo sobre una referencia compartida.

    /// CA7: Se puede llamar sobre referencia compartida (&CanonicalWorkflow).
    /// Si el trait usara `&mut self`, este test no compilaría.
    #[test]
    fn workflow_methods_accept_immutable_reference() {
        let wf = CanonicalWorkflow;
        let shared: &CanonicalWorkflow = &wf;

        // Llamar a los tres métodos vía referencia compartida.
        // Si compila, CA7 está satisfecho.
        let _ns = Workflow::next_status(shared, Status::Draft);
        let _role = Workflow::map_status_to_role(shared, Status::Ready);
        let _cols = Workflow::canonical_column_order(shared);
    }

    // ====================================================================
    // CA6: cargo test --lib domain pasa
    // ====================================================================
    // CA6 se verifica ejecutando los tests; no es un test en sí mismo.
    // El Developer comprobará: cargo test --lib state
    // y cargo test --lib workflow

    // ====================================================================
    // Test adicional: el workflow es determinista
    // ====================================================================

    #[test]
    fn workflow_is_deterministic() {
        let wf = CanonicalWorkflow;
        // Múltiples invocaciones devuelven lo mismo
        for _ in 0..5 {
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
            assert_eq!(wf.canonical_column_order().len(), 9);
        }
    }
}
