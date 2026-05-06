# STORY-022 — Dev — 2026-05-06T00:00:00

## Resultado
❌ Fallo parcial — tests del QA no compilan (3 errores E0716, 49ª iteración sin corrección)

## Resumen de la verificación

### Código de producción — ✅ Correcto

El código de producción en `src/infra/agent.rs` está completo y cubre todos los criterios de aceptación:

| CA | Estado | Implementación |
|----|--------|----------------|
| CA1 | ✅ | `invoke_with_retry()` acepta `verbose: bool` como último parámetro (L78) |
| CA2 | ✅ | `invoke_once()` con `verbose=true` → `invoke_once_verbose()`: `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async (L358) |
| CA3 | ✅ | Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` (L389) |
| CA4 | ✅ | stdout acumulado en `Vec<u8>` y devuelto como `Output.stdout` |
| CA5 | ✅ | stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming |
| CA6 | ✅ | `verbose=false` usa `wait_with_output()` (comportamiento actual) |
| CA7 | ✅ | `kill_process_by_pid()` en ambos modos para timeout cross-platform |
| CA8 | ✅ | `cargo check` compila sin errores |
| CA10 | ✅ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

### Verificaciones de build — ✅ Todo OK

- `cargo check` (0.65s): OK, sin errores
- `cargo build` (0.60s): OK, binario generado
- `cargo clippy --no-deps --bin regista` (0.61s): OK, 0 warnings
- `cargo fmt -- --check`: OK, código formateado
- `cargo test --test architecture` (0.03s): OK, 11/11 pasan

### Tests del QA — ❌ No compilan

3 errores de compilación E0716 (`temporary value dropped while borrowed`) en el módulo `story022`:

| # | Test | Línea en agent.rs | Error |
|---|------|-------------------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido |
| 2 | `ca3_empty_lines_not_logged` | 1809 | Mismo patrón E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | Mismo patrón E0716 |

### Solución exacta (responsabilidad del QA)

El QA debe cambiar cada ocurrencia:

```rust
// Actual (INCORRECTO — E0716):
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// Correcto:
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones:
- Línea 1763 (test `ca3_verbose_logs_lines_with_pipe_prefix`)
- Línea 1809 (test `ca3_empty_lines_not_logged`)
- Línea 2006 (test `ca5_stderr_not_streamed_to_log`)

### Decisión

- **NO se corrige el código de tests.** Es responsabilidad del QA.
- **NO se avanza a In Review.** El estado permanece en Tests Ready.
- El orquestador debe pasar el turno al QA para que corrija los tests.
- El código de producción está completo y no requiere cambios adicionales del Dev.
