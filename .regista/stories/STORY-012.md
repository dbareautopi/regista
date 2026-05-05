# STORY-012: Migrar `pipeline.rs` a async — `process_story` y loop principal

## Status
**Tests Ready**

## Epic
EPIC-04

## Descripción
Con `agent.rs` ya async (STORY-010) y el estado compartido en `Arc<RwLock<>>` (STORY-011), el último paso es convertir `process_story()` y el loop principal de `pipeline.rs` a async. El loop principal usará `.await` en `process_story()` y estará listo para que en #01 se añada `tokio::spawn` para ejecutar múltiples historias en paralelo. Por ahora, el comportamiento sigue siendo secuencial (una historia a la vez), pero la infraestructura async está en su lugar.

## Criterios de aceptación
- [ ] CA1: `process_story()` es una función `async` que invoca `invoke_with_retry(...).await`
- [ ] CA2: El loop principal en `run_real()` usa `process_story(...).await` secuencialmente (sin `tokio::spawn` aún)
- [ ] CA3: `run_dry()` también se adapta a async o se mantiene síncrono (no invoca agentes reales, no necesita tokio)
- [ ] CA4: Las funciones `run_hook()` en `hooks.rs` se migran a `tokio::process::Command` (o se ejecutan con `spawn_blocking`)
- [ ] CA5: `snapshot()` y `rollback()` en `git.rs` se ejecutan con `spawn_blocking` (git es I/O de proceso, no necesita ser async nativo)
- [ ] CA6: `cargo test --lib orchestrator` pasa
- [ ] CA7: `cargo build` compila sin warnings
- [ ] CA8: El pipeline ejecutado con `regista run --dry-run` produce la misma salida que antes de la migración

## Dependencias
- Bloqueado por: STORY-010, STORY-011

## Activity Log
- 2026-05-05 | QA | Verificación de cobertura de tests para los 8 CAs. Los 8 CAs ya están cubiertos por 29 tests existentes en 3 archivos (pipeline.rs: 16 tests CA1-CA3/CA8, hooks.rs: 6 tests CA4, git.rs: 7 tests CA5). No se escribieron nuevos tests. Decisión documentada en .regista/decisions/STORY-012-qa-20260505T000000.md.
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #10.2, recomendación #6).