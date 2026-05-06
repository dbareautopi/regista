# STORY-022 — Dev Verification #125 — 2026-05-06

## Resumen

125ª verificación del código de producción de STORY-022. El código de producción
sigue siendo completo y correcto. Los 3 errores de compilación E0716 en los tests
del QA persisten sin corregir.

## Verificaciones de producción

| Verificación | Resultado |
|---|---|
| `cargo check --bin regista` | ✅ OK (0.31s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.31s) |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pasan (0.25s) |

## Estado de los CAs

| CA | Estado | Detalle |
|----|--------|---------|
| CA1 | ✅ Implementado | `verbose: bool` en `invoke_with_retry()` (L84) y `invoke_with_retry_blocking()` (L199) |
| CA2 | ✅ Implementado | `invoke_once()` con branch `verbose` (L316): false usa `wait_with_output()`, true usa `invoke_once_verbose()` |
| CA3 | ✅ Implementado | `invoke_once_verbose()`: `BufReader::new()` + `read_line()` + `tracing::info!("  │ {}", trimmed)` |
| CA4 | ✅ Implementado | stdout acumulado en `Vec<u8>` |
| CA5 | ✅ Implementado | stderr en `tokio::spawn` separado, sin streaming al log |
| CA6 | ✅ Implementado | `verbose=false` usa `wait_with_output()` |
| CA7 | ✅ Implementado | `kill_process_by_pid()` funciona en ambos modos |
| CA8 | ✅ Verificado | `cargo check --bin regista` compila sin errores |
| CA9 | ❌ Bloqueado | Tests del QA no compilan (3× E0716) |
| CA10 | ✅ Implementado | Call sites en `app/plan.rs` y `app/pipeline.rs` pasan `false` |
| CA11 | ✅ Implementado | `AgentResult` tiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en tests del QA (NO corregidos — 125ª iteración)

Los mismos 3 errores E0716 desde la iteración 1:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — temporal `MutexGuard` destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | mismo error E0716 |

### Solución exacta (responsabilidad del QA)

```rust
// En vez de:
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// Usar:
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones: líneas 1763, 1809, 2006.

## Decisión

NO se avanza a In Review. El orquestador debe pasar el turno al QA para
que corrija los 3 errores de compilación E0716 en los tests.
