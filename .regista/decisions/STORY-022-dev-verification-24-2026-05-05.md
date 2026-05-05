# STORY-022 — Dev Verification 24 — 2026-05-05

## Resultado
❌ No se avanza a In Review — los tests del QA no compilan (24ª iteración sin corrección).

## Verificaciones realizadas

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo check` | OK, 0 errores | 0.20s |
| `cargo build` | OK, binario generado | 0.15s |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings | 0.25s |
| `cargo fmt -- --check` | OK, código formateado | — |
| `cargo test --test architecture` | OK, 11/11 pasan | 0.05s |

## Código de producción — estado

El código de producción cubre todos los CAs implementables (CA1-CA8, CA10-CA11):

- **CA1**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (L78).
- **CA2/CA6**: `invoke_once()` con rama `invoke_once_verbose()` que usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
- **CA3**: Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)`.
- **CA4**: stdout acumulado en `Vec<u8>` y devuelto en `Output`.
- **CA5**: stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming.
- **CA7**: `kill_process_by_pid()` extraído para timeout cross-platform en ambos modos.
- **CA8**: `cargo check` compila sin errores.
- **CA10**: Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.
- **Cargo.toml**: feature `io-util` añadido a tokio.

## Errores E0716 en tests del QA

Los mismos 3 errores persisten (24ª iteración sin corrección):

| Test | Línea | Error |
|------|-------|-------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

Las 3 líneas tienen exactamente el mismo patrón: el `MutexGuard` temporal devuelto por `buffer.lock().unwrap()` se destruye al final del statement, mientras que el `Cow<str>` devuelto por `String::from_utf8_lossy` aún lo referencia.

## Solución requerida (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones (líneas 1763, 1809, 2006).

## CA9 bloqueado

`cargo test` no puede verificarse hasta que el QA corrija los 3 errores de compilación.

## Acción

NO se avanza a In Review. El orquestador debe pasar el turno al QA.
