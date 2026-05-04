# STORY-010: Migrar `agent.rs` a `tokio` — eliminar busy-polling con `thread::sleep`

## Status
**Draft**

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
