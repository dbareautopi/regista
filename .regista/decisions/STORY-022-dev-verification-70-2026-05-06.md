# STORY-022 — Dev — Verificación #70 — 2026-05-06

## Resultado
❌ Tests del QA no compilan (mismos 3 errores E0716, 70ª iteración sin corrección).

## Verificaciones de código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK (0.14s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.28s) |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan (0.04s) |
| `cargo test -- story022` | ❌ NO compila |

## CAs cubiertos por el código de producción

- **CA1**: `invoke_with_retry()` y `invoke_with_retry_blocking()` aceptan `verbose: bool` como último parámetro.
- **CA2**: `invoke_once()` con `verbose=true` usa `invoke_once_verbose()` que implementa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
- **CA3**: Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)`.
- **CA4**: stdout se acumula en `Vec<u8>` y se devuelve como parte del `Output`.
- **CA5**: stderr se lee en `tokio::spawn` separado, sin streaming, acumulado en `Vec<u8>`.
- **CA6**: `verbose=false` usa `wait_with_output()` (comportamiento actual).
- **CA7**: `kill_process_by_pid()` maneja timeout cross-platform en ambos modos.
- **CA8**: `cargo check --lib` compila sin errores.
- **CA10**: Call sites en `plan.rs:152` y `pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.
- **Cargo.toml**: feature `io-util` añadido a tokio.

## Errores en tests del QA (3 × E0716)

Los 3 errores son idénticos en naturaleza: `String::from_utf8_lossy(&buffer.lock().unwrap())` — el `MutexGuard` temporal se destruye antes que el `Cow<str>`.

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución exacta (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones.

## Acción

NO se avanza a In Review. El orquestador debe pasar el turno al QA para que corrija los tests.
