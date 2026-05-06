# STORY-022 — Dev — Verificación décima — 2026-05-05

## Resumen
Décima iteración de verificación de STORY-022 (Streaming de stdout del agente). El código de producción está completo y compila correctamente, pero los tests del QA en `mod story022` siguen teniendo 3 errores `E0716` que bloquean `cargo test`.

## Verificaciones realizadas

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK (0.30s, sin errores) |
| `cargo build` | ✅ OK (0.20s, binario generado) |
| `cargo clippy --no-deps` | ✅ OK (0.23s, 0 warnings) |
| `cargo fmt -- --check` | ✅ OK (código formateado) |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Implementación de producción

La implementación cubre todos los CAs no-test:

- **CA1**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (L78)
- **CA2**: `invoke_once_verbose()` usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async (L358)
- **CA3**: `tracing::info!("  │ {}", trimmed)` para cada línea no vacía de stdout (L394)
- **CA4**: stdout acumulado en `Vec<u8>` y devuelto en `Output` (L387)
- **CA5**: stderr leído en `tokio::spawn` separado con `read_to_end()`, sin streaming al log (L401)
- **CA6**: `verbose=false` usa `wait_with_output()` — comportamiento actual (L340)
- **CA7**: `kill_process_by_pid()` extraído como helper para timeout cross-platform (L440)
- **CA8**: `cargo check` compila ✅
- **CA10**: Call sites actualizados: `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`

## Errores en tests del QA (`mod story022`)

Los mismos 3 errores `E0716` («temporary value dropped while borrowed») persisten desde iteraciones anteriores. NO se corrigen porque es responsabilidad del QA.

| # | Línea | Test | Error |
|---|-------|------|-------|
| 1 | 1763 | `ca3_verbose_logs_lines_with_pipe_prefix` | `String::from_utf8_lossy(&buffer.lock().unwrap())` — el `MutexGuard` temporal se destruye antes que el `Cow<str>` devuelto por `from_utf8_lossy` |
| 2 | 1809 | `ca3_empty_lines_not_logged` | Mismo patrón: `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | 2006 | `ca5_stderr_not_streamed_to_log` | Mismo patrón: `String::from_utf8_lossy(&buffer.lock().unwrap())` |

**Solución requerida (responsabilidad del QA):**
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```
en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Decisión

**NO se avanza el estado a In Review.** El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716 en sus tests.
