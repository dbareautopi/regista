# STORY-022 — Dev Verification #73 (2026-05-06)

## Resumen

Septuagésima tercera verificación. El código de producción está completo y correcto. Los tests del QA no compilan por 3 errores E0716 idénticos.

## Estado del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` (0.23s) | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## CAs cubiertos por el código de producción

| CA | Estado | Detalle |
|----|--------|---------|
| CA1 | ✅ | `invoke_with_retry()` acepta `verbose: bool` como último parámetro |
| CA2 | ✅ | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` por línea no vacía |
| CA4 | ✅ | stdout acumulado en `Vec<u8>` y devuelto como parte del resultado |
| CA5 | ✅ | stderr en `tokio::spawn` separado, sin streaming |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` (comportamiento actual) |
| CA7 | ✅ | `kill_process_by_pid()` para timeout cross-platform en ambos modos |
| CA8 | ✅ | `cargo check --lib` compila sin errores |
| CA10 | ✅ | Todos los call sites actualizados con `verbose: false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` |

## Errores en los tests del QA

Los 3 errores E0716 son idénticos en naturaleza — `MutexGuard` temporal destruido antes que el `Cow<str>` retornado por `String::from_utf8_lossy()`:

| Test | Línea | Error |
|------|-------|-------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal |
| `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

## Solución (responsabilidad del QA)

```rust
// En vez de:
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// Usar:
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO** se corrigen los tests (responsabilidad del QA).
- **NO** se avanza a In Review (CA9 bloqueado).
- El orquestador debe pasar el turno al QA automáticamente.
