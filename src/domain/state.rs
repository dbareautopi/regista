//! Máquina de estados del workflow.
//!
//! Define los estados por los que pasa una historia, los actores
//! que pueden ejecutar transiciones, y las reglas de qué transiciones
//! son válidas desde cada estado.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Estados del workflow de una historia de usuario.
///
/// El flujo feliz es: Draft → Ready → TestsReady → InReview → BusinessReview → Done.
/// Los estados `Blocked` y `Failed` son estados laterales/terminales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    /// Historia creada pero no refinada. Pendiente de PO (plan).
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

/// Conteo de tokens de entrada y salida de una invocación de agente.
///
/// Cada invocación (incluyendo reintentos) produce un `TokenCount`
/// que se acumula en `SharedState::token_usage`.
#[derive(Debug, Clone, Default)]
pub struct TokenCount {
    /// Tokens de entrada (prompt).
    pub input: u64,
    /// Tokens de salida (respuesta).
    pub output: u64,
}

/// Estado compartido del orquestador protegido por RwLock para acceso concurrente.
///
/// Agrupa los contadores que antes se pasaban como `&mut HashMap<...>`
/// a través de la pila de llamadas. Con `Arc` se puede compartir entre
/// múltiples tareas de tokio (paralelismo #01).
#[derive(Debug, Clone, Default)]
pub struct SharedState {
    /// Contador de ciclos de rechazo por historia.
    pub reject_cycles: Arc<RwLock<HashMap<String, u32>>>,
    /// Contador de iteraciones del agente por historia.
    pub story_iterations: Arc<RwLock<HashMap<String, u32>>>,
    /// Último error registrado por historia.
    pub story_errors: Arc<RwLock<HashMap<String, String>>>,
    /// Conteo de tokens por invocación, indexado por story_id.
    /// Cada entrada es un vector con los conteos de cada invocación (incluyendo reintentos).
    pub token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>,
}

impl SharedState {
    /// Construye un SharedState a partir de mapas ya poblados.
    /// Útil al reanudar desde un checkpoint.
    pub fn new(
        reject_cycles: HashMap<String, u32>,
        story_iterations: HashMap<String, u32>,
        story_errors: HashMap<String, String>,
    ) -> Self {
        Self {
            reject_cycles: Arc::new(RwLock::new(reject_cycles)),
            story_iterations: Arc::new(RwLock::new(story_iterations)),
            story_errors: Arc::new(RwLock::new(story_errors)),
            token_usage: Arc::new(RwLock::new(HashMap::new())),
        }
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
        Transition::new(
            Status::BusinessReview,
            Status::InReview,
            Actor::ProductOwner,
        ),
        Transition::new(
            Status::BusinessReview,
            Status::InProgress,
            Actor::ProductOwner,
        ),
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
        Self::ALL.iter().filter(|t| t.from == *self).collect()
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
        assert!(Status::BusinessReview.can_transition_to(Status::Done, Actor::ProductOwner));
    }

    // ── Rechazos ────────────────────────────────────────────────────

    #[test]
    fn inreview_to_inprogress_by_reviewer() {
        assert!(Status::InReview.can_transition_to(Status::InProgress, Actor::Reviewer));
    }

    #[test]
    fn businessreview_to_inreview_by_po() {
        assert!(Status::BusinessReview.can_transition_to(Status::InReview, Actor::ProductOwner));
    }

    #[test]
    fn businessreview_to_inprogress_by_po() {
        assert!(Status::BusinessReview.can_transition_to(Status::InProgress, Actor::ProductOwner));
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
        for target in [
            Status::Ready,
            Status::InReview,
            Status::BusinessReview,
            Status::Draft,
        ] {
            for actor in [
                Actor::ProductOwner,
                Actor::QaEngineer,
                Actor::Developer,
                Actor::Reviewer,
            ] {
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

    // ── STORY-020: TokenCount y token_usage en SharedState ──

    mod story020 {
        use super::*;
        use std::collections::HashMap;
        use std::sync::{Arc, RwLock};

        // ══════════════════════════════════════════════════════════
        // CA1: Existe pub struct TokenCount con input: u64, output: u64
        // ══════════════════════════════════════════════════════════

        /// CA1: TokenCount existe y sus campos son accesibles con los tipos correctos.
        #[test]
        fn token_count_exists_with_correct_fields() {
            let tc = TokenCount {
                input: 1500,
                output: 800,
            };
            assert_eq!(tc.input, 1500);
            assert_eq!(tc.output, 800);

            // Verificar tipos en tiempo de compilación
            let _input: u64 = tc.input;
            let _output: u64 = tc.output;
            let _ = (_input, _output);
        }

        /// CA1: TokenCount se puede construir con valores cero.
        #[test]
        fn token_count_zero_values() {
            let tc = TokenCount { input: 0, output: 0 };
            assert_eq!(tc.input, 0);
            assert_eq!(tc.output, 0);
        }

        // ══════════════════════════════════════════════════════════
        // CA2: TokenCount implementa Debug, Clone, y Default
        // ══════════════════════════════════════════════════════════

        /// CA2: TokenCount implementa Debug.
        #[test]
        fn token_count_implements_debug() {
            let tc = TokenCount {
                input: 1234,
                output: 567,
            };
            let debug_str = format!("{:?}", tc);
            assert!(debug_str.contains("1234"), "debug debe contener input: {debug_str}");
            assert!(debug_str.contains("567"), "debug debe contener output: {debug_str}");
        }

        /// CA2: TokenCount implementa Clone (clon produce un valor independiente).
        #[test]
        fn token_count_implements_clone() {
            let tc1 = TokenCount {
                input: 100,
                output: 200,
            };
            let tc2 = tc1.clone();
            assert_eq!(tc2.input, 100);
            assert_eq!(tc2.output, 200);

            // Verificar que son independientes (no comparten memoria)
            let tc3 = tc2.clone();
            drop(tc1);
            drop(tc2);
            assert_eq!(tc3.input, 100);
            assert_eq!(tc3.output, 200);
        }

        /// CA2: TokenCount implementa Default (valores a cero).
        #[test]
        fn token_count_implements_default() {
            let tc = TokenCount::default();
            assert_eq!(tc.input, 0);
            assert_eq!(tc.output, 0);
        }

        /// CA2: Default de TokenCount es consistente.
        #[test]
        fn token_count_default_is_zero() {
            let tc1 = TokenCount::default();
            let tc2 = TokenCount::default();
            assert_eq!(tc1.input, tc2.input);
            assert_eq!(tc1.output, tc2.output);
        }

        // ══════════════════════════════════════════════════════════
        // CA3: SharedState tiene campo token_usage
        // ══════════════════════════════════════════════════════════

        /// CA3: SharedState tiene el campo token_usage del tipo correcto.
        #[test]
        fn shared_state_has_token_usage_field() {
            let state = SharedState::default();
            // Verificar que el campo existe y tiene el tipo esperado
            let _: &Arc<RwLock<HashMap<String, Vec<TokenCount>>>> = &state.token_usage;
        }

        /// CA3: token_usage comienza vacío con default.
        #[test]
        fn token_usage_default_is_empty() {
            let state = SharedState::default();
            let guard = state.token_usage.read().unwrap();
            assert!(guard.is_empty());
        }

        // ══════════════════════════════════════════════════════════
        // CA4: SharedState::new() inicializa token_usage
        // ══════════════════════════════════════════════════════════

        /// CA4: SharedState::new() inicializa token_usage como HashMap vacío.
        #[test]
        fn shared_state_new_initializes_token_usage() {
            let state = SharedState::new(
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            );

            let guard = state.token_usage.read().unwrap();
            assert!(
                guard.is_empty(),
                "token_usage debe estar vacío tras new()"
            );
        }

        /// CA4: SharedState::new() con otros mapas poblados no afecta a token_usage.
        #[test]
        fn shared_state_new_isolates_token_usage() {
            let mut reject = HashMap::new();
            reject.insert("S1".into(), 3u32);

            let state = SharedState::new(reject, HashMap::new(), HashMap::new());

            // token_usage debe seguir vacío independientemente de otros campos
            assert!(state.token_usage.read().unwrap().is_empty());
            assert_eq!(state.reject_cycles.read().unwrap().get("S1"), Some(&3));
        }

        /// CA4: token_usage es escribible tras new().
        #[test]
        fn token_usage_writable_after_new() {
            let state = SharedState::new(
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            );

            state.token_usage.write().unwrap().insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 10,
                    output: 5,
                }],
            );

            let guard = state.token_usage.read().unwrap();
            assert_eq!(guard.get("STORY-001").unwrap().len(), 1);
        }

        // ══════════════════════════════════════════════════════════
        // CA5: SharedState sigue implementando Clone (comparte Arc)
        // ══════════════════════════════════════════════════════════

        /// CA5: SharedState implementa Clone.
        #[test]
        fn shared_state_implements_clone() {
            let state = SharedState::default();
            let _clone = state.clone();
        }

        /// CA5: Clonar SharedState comparte el Arc de reject_cycles (regresión).
        #[test]
        fn clone_shares_existing_fields() {
            let state = SharedState::default();
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("S1".into(), 5);

            let clone = state.clone();
            assert_eq!(clone.reject_cycles.read().unwrap().get("S1"), Some(&5));

            // Escribir en el clone afecta al original (mismo Arc)
            clone
                .reject_cycles
                .write()
                .unwrap()
                .insert("S1".into(), 99);
            assert_eq!(state.reject_cycles.read().unwrap().get("S1"), Some(&99));
        }

        // ══════════════════════════════════════════════════════════
        // CA8: token_usage se puede leer y escribir concurrentemente
        // ══════════════════════════════════════════════════════════

        /// CA8: Se puede escribir y luego leer token_usage.
        #[test]
        fn token_usage_write_then_read() {
            let state = SharedState::default();

            // Escribir una entrada
            {
                let mut w = state.token_usage.write().unwrap();
                w.insert(
                    "STORY-001".into(),
                    vec![
                        TokenCount {
                            input: 100,
                            output: 50,
                        },
                        TokenCount {
                            input: 200,
                            output: 150,
                        },
                    ],
                );
            }

            // Leerla
            {
                let r = state.token_usage.read().unwrap();
                let entries = r.get("STORY-001").unwrap();
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].input, 100);
                assert_eq!(entries[0].output, 50);
                assert_eq!(entries[1].input, 200);
                assert_eq!(entries[1].output, 150);
            }
        }

        /// CA8: Múltiples readers pueden acceder token_usage simultáneamente.
        #[test]
        fn token_usage_multiple_readers() {
            let state = SharedState::default();

            state.token_usage.write().unwrap().insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 10,
                    output: 5,
                }],
            );

            // Dos read locks simultáneos (RwLock lo permite)
            let r1 = state.token_usage.read().unwrap();
            let r2 = state.token_usage.read().unwrap();

            assert_eq!(r1.get("STORY-001").unwrap()[0].input, 10);
            assert_eq!(r2.get("STORY-001").unwrap()[0].input, 10);

            drop(r1);
            drop(r2);
        }

        /// CA8: Write lock es exclusivo — try_read falla durante write lock.
        #[test]
        fn token_usage_write_lock_is_exclusive() {
            let state = SharedState::default();

            let mut w = state.token_usage.write().unwrap();
            w.insert(
                "X".into(),
                vec![TokenCount { input: 1, output: 2 }],
            );

            // try_read debe fallar porque hay un write lock activo
            assert!(state.token_usage.try_read().is_err());

            drop(w);

            // Ahora try_read debe funcionar
            assert!(state.token_usage.try_read().is_ok());
        }

        /// CA8: Se puede añadir una segunda entrada a una historia existente
        /// (simulando múltiples invocaciones sobre la misma historia).
        #[test]
        fn token_usage_append_to_existing_story() {
            let state = SharedState::default();

            // Primera invocación
            state.token_usage.write().unwrap().insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 100,
                    output: 50,
                }],
            );

            // Segunda invocación (append)
            {
                let mut w = state.token_usage.write().unwrap();
                let entries = w.get_mut("STORY-001").unwrap();
                entries.push(TokenCount {
                    input: 200,
                    output: 100,
                });
            }

            let r = state.token_usage.read().unwrap();
            let entries = r.get("STORY-001").unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].input, 100);
            assert_eq!(entries[1].input, 200);
        }

        /// CA8: Se pueden gestionar múltiples historias independientemente.
        #[test]
        fn token_usage_multiple_stories() {
            let state = SharedState::default();

            {
                let mut w = state.token_usage.write().unwrap();
                w.insert(
                    "STORY-001".into(),
                    vec![TokenCount {
                        input: 10,
                        output: 5,
                    }],
                );
                w.insert(
                    "STORY-002".into(),
                    vec![TokenCount {
                        input: 20,
                        output: 15,
                    }],
                );
            }

            let r = state.token_usage.read().unwrap();
            assert_eq!(r.len(), 2);
            assert_eq!(r.get("STORY-001").unwrap()[0].input, 10);
            assert_eq!(r.get("STORY-002").unwrap()[0].input, 20);
        }

        // ══════════════════════════════════════════════════════════
        // CA9: Clonar SharedState comparte el mismo token_usage
        // ══════════════════════════════════════════════════════════

        /// CA9: Clonar SharedState comparte el token_usage (escritura en clone
        /// visible en original).
        #[test]
        fn clone_shares_token_usage_write_visible() {
            let state = SharedState::default();

            // Escribir en el original
            state.token_usage.write().unwrap().insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 100,
                    output: 50,
                }],
            );

            let clone = state.clone();

            // El clone ve los datos del original
            {
                let r = clone.token_usage.read().unwrap();
                let entries = r.get("STORY-001").unwrap();
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].input, 100);
                assert_eq!(entries[0].output, 50);
            }

            // Escribir en el clone
            clone.token_usage.write().unwrap().insert(
                "STORY-002".into(),
                vec![TokenCount {
                    input: 200,
                    output: 100,
                }],
            );

            // El original ve la escritura del clone
            {
                let r = state.token_usage.read().unwrap();
                assert!(r.contains_key("STORY-001"));
                assert!(r.contains_key("STORY-002"));
                assert_eq!(r.get("STORY-002").unwrap()[0].input, 200);
            }
        }

        /// CA9: Append en clone visible en original (misma referencia).
        #[test]
        fn clone_append_visible_in_original() {
            let state = SharedState::default();

            state.token_usage.write().unwrap().insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 10,
                    output: 5,
                }],
            );

            let clone = state.clone();

            // Append en el clone
            {
                let mut w = clone.token_usage.write().unwrap();
                w.get_mut("STORY-001")
                    .unwrap()
                    .push(TokenCount {
                        input: 20,
                        output: 10,
                    });
            }

            // Visible en el original
            let r = state.token_usage.read().unwrap();
            let entries = r.get("STORY-001").unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[1].input, 20);
        }

        /// CA9: Múltiples clones comparten el mismo token_usage.
        #[test]
        fn multiple_clones_share_same_token_usage() {
            let state = SharedState::default();
            let clone1 = state.clone();
            let clone2 = state.clone();

            // Escribir en clone1
            clone1.token_usage.write().unwrap().insert(
                "A".into(),
                vec![TokenCount { input: 1, output: 1 }],
            );

            // clone2 y original lo ven
            assert_eq!(
                clone2.token_usage.read().unwrap().get("A").unwrap()[0].input,
                1
            );
            assert_eq!(
                state.token_usage.read().unwrap().get("A").unwrap()[0].input,
                1
            );
        }
    }

    // ── STORY-011: SharedState con Arc<RwLock<>> ──

    mod story011 {
        use super::*;
        use std::collections::HashMap;
        use std::sync::{Arc, RwLock};

        // ══════════════════════════════════════════════════════════
        // CA1: SharedState agrupa los contadores
        // ══════════════════════════════════════════════════════════

        /// CA1: SharedState existe con los tres campos del tipo correcto.
        #[test]
        fn shared_state_has_required_fields() {
            let state = SharedState::default();

            // Verificar que los campos son accesibles y del tipo correcto
            let _: &Arc<RwLock<HashMap<String, u32>>> = &state.reject_cycles;
            let _: &Arc<RwLock<HashMap<String, u32>>> = &state.story_iterations;
            let _: &Arc<RwLock<HashMap<String, String>>> = &state.story_errors;
        }

        /// CA1: SharedState::default() tiene HashMaps vacíos.
        #[test]
        fn shared_state_default_is_empty() {
            let state = SharedState::default();

            assert!(state.reject_cycles.read().unwrap().is_empty());
            assert!(state.story_iterations.read().unwrap().is_empty());
            assert!(state.story_errors.read().unwrap().is_empty());
        }

        /// CA1: SharedState es Clone y los clones comparten el mismo Arc.
        #[test]
        fn shared_state_clone_shares_data() {
            let state = SharedState::default();
            state.reject_cycles.write().unwrap().insert("S1".into(), 5);

            let clone = state.clone();
            assert_eq!(clone.reject_cycles.read().unwrap().get("S1"), Some(&5));

            // Escribir en el clone también afecta al original (mismo Arc)
            clone
                .story_iterations
                .write()
                .unwrap()
                .insert("S1".into(), 10);
            assert_eq!(state.story_iterations.read().unwrap().get("S1"), Some(&10));
        }

        // ══════════════════════════════════════════════════════════
        // CA3: Lecturas con .read().unwrap(), escrituras con .write().unwrap()
        // ══════════════════════════════════════════════════════════

        /// CA3: read() lock devuelve una guarda que permite leer.
        #[test]
        fn read_lock_allows_reading() {
            let state = SharedState::default();
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-001".into(), 3);

            let guard = state.reject_cycles.read().unwrap();
            assert_eq!(guard.get("STORY-001"), Some(&3));
        }

        /// CA3: write() lock permite insertar y modificar.
        #[test]
        fn write_lock_allows_mutation() {
            let state = SharedState::default();

            {
                let mut guard = state.reject_cycles.write().unwrap();
                guard.insert("STORY-001".into(), 1);
                guard.insert("STORY-002".into(), 2);
            }

            let guard = state.reject_cycles.read().unwrap();
            assert_eq!(guard.get("STORY-001"), Some(&1));
            assert_eq!(guard.get("STORY-002"), Some(&2));
        }

        /// CA3: El lock se libera al salir del scope (drop).
        #[test]
        fn lock_is_released_after_scope() {
            let state = SharedState::default();

            // Tomar write lock, modificarlo, soltarlo
            {
                let mut w = state.reject_cycles.write().unwrap();
                w.insert("A".into(), 42);
            }
            // Lock liberado — podemos tomar read lock
            {
                let r = state.reject_cycles.read().unwrap();
                assert_eq!(r.get("A"), Some(&42));
            }
            // Se puede volver a tomar write lock
            {
                let mut w = state.reject_cycles.write().unwrap();
                w.insert("B".into(), 100);
            }
            assert_eq!(state.reject_cycles.read().unwrap().len(), 2);
        }

        /// CA3: Múltiples readers concurrentes (RwLock lo permite).
        #[test]
        fn multiple_readers_allowed() {
            let state = SharedState::default();
            state.reject_cycles.write().unwrap().insert("X".into(), 99);

            // Dos read locks simultáneos deben ser posibles
            let r1 = state.reject_cycles.read().unwrap();
            let r2 = state.reject_cycles.read().unwrap();

            assert_eq!(r1.get("X"), Some(&99));
            assert_eq!(r2.get("X"), Some(&99));

            drop(r1);
            drop(r2);
        }

        /// CA3: Write lock es exclusivo (no se puede leer mientras se escribe
        /// en el mismo scope).
        #[test]
        fn write_lock_is_exclusive() {
            let state = SharedState::default();

            // Tomar write lock
            let mut w = state.reject_cycles.write().unwrap();
            w.insert("Z".into(), 1);

            // try_read falla porque hay un write lock activo
            assert!(state.reject_cycles.try_read().is_err());

            drop(w);

            // Ahora sí se puede leer
            assert!(state.reject_cycles.try_read().is_ok());
        }
    }
}
