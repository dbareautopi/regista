# STORY-010: Migrar `agent.rs` a `tokio` — eliminar busy-polling con `thread::sleep`

## Status
**In Review**

## Epic
EPIC-04

## Descripción
Actualmente `invoke_once()` en `src/infra/agent.rs` usa busy-polling con `thread::sleep(250ms)` para esperar a que el proceso hijo (agente LLM) termine. Esto bloquea un thread del sistema operativo durante toda la ejecución del agente (2-10 minutos). Hay que migrar a `tokio::process::Command` y usar `tokio::time::timeout` para esperar de forma no bloqueante. El trait `AgentProvider` ya devuelve `Vec<String>` (no `Command`), lo que hace esta migración posible sin tocar los providers.

## Criterios de aceptación
- [ ] CA1: `invoke_once()` es una función `async` que usa `tokio::process::Command` en lugar de `std::process::Command`
- [ ] CA2: El loop de espera usa `tokio::time::timeout` en lugar de `thread::sleep` + `try_wait`
- [ ] CA3: `invoke_with_retry()` es una función `async` que usa `tokio::time::sleep` para el backoff entre reintentos
- [ ] CA4: El backoff exponencial (`delay *= 2`) se mantiene, pero con `tokio::time::sleep` en lugar de `std::thread::sleep`
- [ ] CA5: `save_agent_decision()` se migra a `tokio::fs::write` (o se mantiene síncrona con `spawn_blocking` si el impacto es mínimo)
- [ ] CA6: `cargo build` compila con `tokio` como dependencia (ya debería estar en `Cargo.toml` como dependencia opcional o añadirla)
- [ ] CA7: Los tests existentes de `agent.rs` se adaptan a async (usando `#[tokio::test]`)
- [ ] CA8: `cargo test --lib agent` pasa

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #2.4, #10.2; recomendación #6).
- 2026-05-05 | PO | Historia refinada. Cumple DoR: descripción clara, 8 CAs testeables, sin dependencias. Validado contra src/infra/agent.rs — busy-polling con thread::sleep(250ms) confirmado. Tokio ausente en Cargo.toml (CA6). Movida a Ready.
- 2026-05-05 | QA | Tests escritos en src/infra/agent.rs::tests. 14 tests: 3 sync (funciones puras preservadas), 10 async (#[tokio::test]), 1 compile-time (tokio dependency check). Cada CA tiene al menos 1 test dedicado. Los tests usan .await sobre invoke_once/invoke_with_retry/save_agent_decision — definen el contrato async que el Developer debe implementar. Tokio NO está en Cargo.toml aún (CA6 — el Developer debe añadirlo). El Developer verificará compilación con cargo test --lib agent (CA8). Movida a Tests Ready.
- 2026-05-05 | Dev | Implementación de migración a tokio completada. Cambios: (1) tokio añadido a Cargo.toml con features rt-multi-thread, macros, process, time, fs; (2) invoke_once() migrada a async con tokio::process::Command + tokio::time::timeout (timeout mata proceso por PID vía kill -9); (3) invoke_with_retry() migrada a async con tokio::time::sleep para backoff exponencial; (4) save_agent_decision() migrada a async con tokio::fs::write; (5) wrapper invoke_with_retry_blocking() para callers síncronos (plan.rs, pipeline.rs) usando LazyLock<Runtime>; (6) eliminados std::thread::sleep y busy-polling con try_wait. Build, clippy y fmt pasan limpios. 278/280 tests pasan. ⚠️ NO se avanza a InReview: el test save_agent_decision_creates_file_with_content tiene un bug del QA — el test usa decisions_dir = tmp.path() (raíz del tempdir) y también crea product-owner/SKILL.md dentro del mismo tempdir. read_dir() cuenta 2 entradas (subdirectorio + archivo de decisión) en vez de 1. Fix sugerido: usar tmp.path().join("decisions") como decisions_dir o filtrar con .is_file(). Ver decisions/STORY-010-dev-20260505T000000.md.
- 2026-05-05 | QA | Tests corregidos. Bug en save_agent_decision_creates_file_with_content: el decisions_dir colisionaba con el directorio de skills en la raíz del tempdir, causando que read_dir() contara 2 entradas en vez de 1. Corrección: decisions_dir movido a subdirectorio dedicado tmp.path().join("decisions") y skills a tmp.path().join("skills"). 14/14 tests pasan, clippy y fmt limpios. Status se mantiene en Tests Ready. Ver decisions/STORY-010-qa-20260505T000000.md.
- 2026-05-05 | Dev | Verificación post-corrección QA. Implementación ya presente en src/infra/agent.rs (migración a tokio completada en iteración anterior). Verificaciones: cargo build (OK), cargo clippy -- -D warnings (0 warnings), cargo fmt --check (OK), cargo test (290/290 pasan, 1 ignorado — requiere pi). Cambios confirmados: invoke_once() async con tokio::process::Command + tokio::time::timeout; invoke_with_retry() async con tokio::time::sleep y backoff exponencial; save_agent_decision() async con tokio::fs::write; invoke_with_retry_blocking() wrapper usando LazyLock<Runtime> para callers síncronos (plan.rs, pipeline.rs); tokio en Cargo.toml con features rt-multi-thread, macros, process, time, fs. CAs 1-8 verificados. Movida a In Review.
