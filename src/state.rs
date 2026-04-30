//! Máquina de estados del workflow.
//!
//! Define los estados por los que pasa una historia, los actores
//! que pueden ejecutar transiciones, y las reglas de qué transiciones
//! son válidas desde cada estado.

use serde::{Deserialize, Serialize};

/// Estados del workflow de una historia de usuario.
///
/// El flujo feliz es: Draft → Ready → TestsReady → InReview → BusinessReview → Done.
/// Los estados `Blocked` y `Failed` son estados laterales/terminales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    /// Historia creada pero no refinada. Pendiente de PO (groom).
    Draft,
    /// Historia refinada, cumple DoR. Pendiente de QA.
    Ready,
    /// Tests escritos por QA. Pendiente de Developer.
    TestsReady,
    /// Developer está corrigiendo tras un rechazo.
    InProgress,
    /// Implementación lista. Pendiente de Reviewer.
    InReview,
    /// Reviewer aprobó DoD técnico. Pendiente de PO (validate).
    BusinessReview,
    /// Historia completada y validada. Estado terminal exitoso.
    Done,
    /// Bloqueada por dependencias no resueltas.
    Blocked,
    /// Superó el máximo de ciclos de rechazo. Estado terminal de fallo.
    Failed,
}

/// Actores que pueden ejecutar transiciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Actor {
    /// Product Owner — refina (Draft→Ready) y valida valor de negocio (BusinessReview→Done).
    ProductOwner,
    /// QA Engineer — escribe tests (Ready→TestsReady) y corrige tests (TestsReady→TestsReady).
    QaEngineer,
    /// Developer — implementa (TestsReady→InReview) y corrige tras rechazo (InProgress→InReview).
    Developer,
    /// Reviewer — puerta técnica (InReview→BusinessReview / InProgress).
    Reviewer,
    /// El propio orquestador — transiciones automáticas (Blocked, Failed, desbloqueo).
    Orchestrator,
}

/// Una transición entre dos estados, con el actor responsable.
///
/// Solo las transiciones definidas en las constantes de este módulo
/// son válidas. Cualquier otra combinación es un error de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Transition {
    pub from: Status,
    pub to: Status,
    pub actor: Actor,
}

impl Transition {
    #[allow(dead_code)]
    pub const fn new(from: Status, to: Status, actor: Actor) -> Self {
        Self { from, to, actor }
    }
}

// ── Transiciones canónicas ──────────────────────────────────────────────

impl Status {
    /// Todas las transiciones permitidas.
    #[allow(dead_code)]
    pub const ALL: &[Transition] = &[
        // ── PO ──────────────────────────────────────────────────
        Transition::new(Status::Draft, Status::Ready, Actor::ProductOwner),
        Transition::new(Status::BusinessReview, Status::Done, Actor::ProductOwner),
        Transition::new(Status::BusinessReview, Status::InReview, Actor::ProductOwner),
        Transition::new(Status::BusinessReview, Status::InProgress, Actor::ProductOwner),
        // ── QA ──────────────────────────────────────────────────
        Transition::new(Status::Ready, Status::TestsReady, Actor::QaEngineer),
        Transition::new(Status::Ready, Status::Draft, Actor::QaEngineer),
        Transition::new(Status::TestsReady, Status::TestsReady, Actor::QaEngineer),
        // ── Developer ───────────────────────────────────────────
        Transition::new(Status::TestsReady, Status::InReview, Actor::Developer),
        Transition::new(Status::InProgress, Status::InReview, Actor::Developer),
        // ── Reviewer ────────────────────────────────────────────
        Transition::new(Status::InReview, Status::BusinessReview, Actor::Reviewer),
        Transition::new(Status::InReview, Status::InProgress, Actor::Reviewer),
        // ── Orchestrator (automático) ───────────────────────────
        Transition::new(Status::Blocked, Status::Ready, Actor::Orchestrator),
    ];

    /// Transiciones permitidas DESDE este estado.
    #[allow(dead_code)]
    pub fn allowed_from(&self) -> Vec<&'static Transition> {
        Self::ALL
            .iter()
            .filter(|t| t.from == *self)
            .collect()
    }

    /// ¿Es válida la transición de `self` a `target` ejecutada por `actor`?
    #[allow(dead_code)]
    pub fn can_transition_to(&self, target: Status, actor: Actor) -> bool {
        Self::ALL
            .iter()
            .any(|t| t.from == *self && t.to == target && t.actor == actor)
    }

    /// ¿Es un estado terminal? (el pipeline no volverá a tocar esta historia).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Status::Done | Status::Failed)
    }

    /// ¿Es un estado desde el que el loop normal puede disparar un agente?
    /// (excluye Draft, Blocked, y terminales)
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            Status::Ready
                | Status::TestsReady
                | Status::InProgress
                | Status::InReview
                | Status::BusinessReview
        )
    }

    /// ¿Es un estado "stuck" que requiere intervención del PO?
    /// (Draft siempre, Blocked depende del contexto de dependencias)
    #[allow(dead_code)]
    pub fn is_stuck(&self) -> bool {
        matches!(self, Status::Draft)
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Status::Draft => "Draft",
            Status::Ready => "Ready",
            Status::TestsReady => "Tests Ready",
            Status::InProgress => "In Progress",
            Status::InReview => "In Review",
            Status::BusinessReview => "Business Review",
            Status::Done => "Done",
            Status::Blocked => "Blocked",
            Status::Failed => "Failed",
        };
        write!(f, "{s}")
    }
}

impl std::fmt::Display for Actor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Actor::ProductOwner => "PO",
            Actor::QaEngineer => "QA",
            Actor::Developer => "Dev",
            Actor::Reviewer => "Reviewer",
            Actor::Orchestrator => "Orchestrator",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Transiciones felices ────────────────────────────────────────

    #[test]
    fn draft_to_ready_by_po() {
        assert!(Status::Draft.can_transition_to(Status::Ready, Actor::ProductOwner));
    }

    #[test]
    fn ready_to_testsready_by_qa() {
        assert!(Status::Ready.can_transition_to(Status::TestsReady, Actor::QaEngineer));
    }

    #[test]
    fn testsready_to_inreview_by_dev() {
        assert!(Status::TestsReady.can_transition_to(Status::InReview, Actor::Developer));
    }

    #[test]
    fn inreview_to_businessreview_by_reviewer() {
        assert!(Status::InReview.can_transition_to(Status::BusinessReview, Actor::Reviewer));
    }

    #[test]
    fn businessreview_to_done_by_po() {
        assert!(
            Status::BusinessReview.can_transition_to(Status::Done, Actor::ProductOwner)
        );
    }

    // ── Rechazos ────────────────────────────────────────────────────

    #[test]
    fn inreview_to_inprogress_by_reviewer() {
        assert!(Status::InReview.can_transition_to(Status::InProgress, Actor::Reviewer));
    }

    #[test]
    fn businessreview_to_inreview_by_po() {
        assert!(
            Status::BusinessReview.can_transition_to(Status::InReview, Actor::ProductOwner)
        );
    }

    #[test]
    fn businessreview_to_inprogress_by_po() {
        assert!(
            Status::BusinessReview.can_transition_to(Status::InProgress, Actor::ProductOwner)
        );
    }

    #[test]
    fn inprogress_to_inreview_by_dev() {
        assert!(Status::InProgress.can_transition_to(Status::InReview, Actor::Developer));
    }

    // ── Rollbacks ───────────────────────────────────────────────────

    #[test]
    fn ready_to_draft_by_qa() {
        assert!(Status::Ready.can_transition_to(Status::Draft, Actor::QaEngineer));
    }

    #[test]
    fn testsready_to_testsready_by_qa() {
        assert!(Status::TestsReady.can_transition_to(Status::TestsReady, Actor::QaEngineer));
    }

    // ── Automáticas (Orchestrator) ──────────────────────────────────

    #[test]
    fn blocked_to_ready_by_orchestrator() {
        assert!(Status::Blocked.can_transition_to(Status::Ready, Actor::Orchestrator));
    }

    // ── Transiciones PROHIBIDAS ──────────────────────────────────────────

    #[test]
    fn draft_cannot_go_directly_to_done() {
        assert!(!Status::Draft.can_transition_to(Status::Done, Actor::ProductOwner));
    }

    #[test]
    fn ready_cannot_be_done_by_dev() {
        assert!(!Status::Ready.can_transition_to(Status::Done, Actor::Developer));
    }

    #[test]
    fn inreview_cannot_be_done_by_reviewer() {
        assert!(!Status::InReview.can_transition_to(Status::Done, Actor::Reviewer));
    }

    #[test]
    fn done_cannot_transition_to_anything() {
        for target in [Status::Ready, Status::InReview, Status::BusinessReview, Status::Draft] {
            for actor in [Actor::ProductOwner, Actor::QaEngineer, Actor::Developer, Actor::Reviewer] {
                assert!(
                    !Status::Done.can_transition_to(target, actor),
                    "Done should not transition to {target} by {actor}"
                );
            }
        }
    }

    #[test]
    fn failed_cannot_transition_to_anything() {
        for target in [Status::Ready, Status::InReview, Status::Draft] {
            for actor in [Actor::ProductOwner, Actor::QaEngineer, Actor::Developer] {
                assert!(
                    !Status::Failed.can_transition_to(target, actor),
                    "Failed should not transition to {target} by {actor}"
                );
            }
        }
    }

    #[test]
    fn qa_cannot_mark_done() {
        assert!(!Status::TestsReady.can_transition_to(Status::Done, Actor::QaEngineer));
    }

    #[test]
    fn dev_cannot_mark_done() {
        assert!(!Status::InReview.can_transition_to(Status::Done, Actor::Developer));
    }

    // ── Propiedades ────────────────────────────────────────────────────

    #[test]
    fn terminal_states() {
        assert!(Status::Done.is_terminal());
        assert!(Status::Failed.is_terminal());
        assert!(!Status::Draft.is_terminal());
        assert!(!Status::Ready.is_terminal());
        assert!(!Status::InReview.is_terminal());
    }

    #[test]
    fn actionable_states() {
        assert!(Status::Ready.is_actionable());
        assert!(Status::TestsReady.is_actionable());
        assert!(Status::InProgress.is_actionable());
        assert!(Status::InReview.is_actionable());
        assert!(Status::BusinessReview.is_actionable());
        assert!(!Status::Draft.is_actionable());
        assert!(!Status::Done.is_actionable());
        assert!(!Status::Failed.is_actionable());
        assert!(!Status::Blocked.is_actionable());
    }

    #[test]
    fn allowed_from_returns_valid_transitions() {
        let ready_transitions = Status::Ready.allowed_from();
        assert_eq!(ready_transitions.len(), 2); // → TestsReady (QA), → Draft (QA)
        let targets: Vec<Status> = ready_transitions.iter().map(|t| t.to).collect();
        assert!(targets.contains(&Status::TestsReady));
        assert!(targets.contains(&Status::Draft));
    }

    #[test]
    fn display_formats_correctly() {
        assert_eq!(Status::TestsReady.to_string(), "Tests Ready");
        assert_eq!(Status::InProgress.to_string(), "In Progress");
        assert_eq!(Status::InReview.to_string(), "In Review");
        assert_eq!(Status::BusinessReview.to_string(), "Business Review");
        assert_eq!(Actor::ProductOwner.to_string(), "PO");
        assert_eq!(Actor::Orchestrator.to_string(), "Orchestrator");
    }
}
