# STORY-022 — Dev Implementation — 2026-05-05

## Resumen

Implementación completa del streaming de stdout + parámetro `verbose` en el sistema de invocación de agentes.

## Decisiones de diseño

### 1. Feature `io-util` de tokio

Se añadió `io-util` a los features de tokio en `Cargo.toml`. Necesario para:
- `tokio::io::BufReader` (lectura línea a línea de stdout)
- `tokio::io::AsyncBufReadExt::read_line()`
- `tokio::io::AsyncReadExt::read_to_end()` (lectura completa de stderr)

### 2. Separación verbose/no-verbose en `invoke_once()`

La función `invoke_once()` ahora acepta `verbose: bool` y bifurca:
- `verbose=false` → `child.wait_with_output()` (comportamiento existente, más eficiente, sin overhead)
- `verbose=true` → `invoke_once_verbose()` (nueva función privada)

Motivo: mantener la ruta rápida para el caso común y aislar la complejidad del streaming.

### 3. Streaming de stdout en `invoke_once_verbose()`

Implementación:
1. `child.stdout.take()` → `Option<ChildStdout>`
2. `BufReader::new(stdout)` → lectura línea a línea
3. Bucle `loop { reader.read_line(&mut line).await }`:
   - `Ok(0)` → EOF, salir
   - `Ok(_)` → acumular `line.as_bytes()` en `Vec<u8>`, loguear con `tracing::info!("  │ {}", trimmed)` si no está vacía
4. El `Vec<u8>` acumulado se devuelve como parte de `std::process::Output`

### 4. Captura de stderr en `invoke_once_verbose()`

- `child.stderr.take()` → `Option<ChildStderr>`
- `tokio::spawn(async { reader.read_to_end(&mut buf).await; buf })`
- Sin streaming al log (solo se acumula en `Vec<u8>`)
- Se espera (`await`) la tarea después de que el proceso hijo termina

### 5. Timeout en modo verbose

- Se usa `tokio::time::timeout(timeout, child.wait()).await`
- En timeout: se mata el proceso con `kill_process_by_pid(pid)` y se devuelve error
- Las tareas de stdout/stderr no se esperan en caso de timeout (el proceso muerto cierra los pipes)

### 6. Helper `kill_process_by_pid()`

Extraído a función separada para evitar duplicación entre la ruta verbose y no-verbose.
Cross-platform: `kill -9` (Unix) / `taskkill` (Windows).

### 7. Propagación del parámetro `verbose`

- `invoke_with_retry()` → acepta `verbose: bool`, lo pasa a `invoke_once()`
- `invoke_with_retry_blocking()` → acepta `verbose: bool`, lo pasa a `invoke_with_retry()`
- Call sites en `app/plan.rs` y `app/pipeline.rs` → pasan `false` (no requieren streaming)

### 8. `AgentResult` sin cambios

La estructura `AgentResult` mantiene sus campos `stdout: String`, `stderr: String`, `exit_code: i32`.
No fue necesario modificarla.

## Archivos modificados

| Archivo | Cambio |
|---------|--------|
| `Cargo.toml` | Añadido `io-util` a features de tokio |
| `src/infra/agent.rs` | `invoke_once()` + `verbose`, `invoke_once_verbose()` nueva, `kill_process_by_pid()` nuevo, firmas actualizadas de `invoke_with_retry()` e `invoke_with_retry_blocking()` |
| `src/app/plan.rs` | `invoke_with_retry_blocking()` → añadido `false` |
| `src/app/pipeline.rs` | `invoke_with_retry()` → añadido `false` |

## Verificación

- `cargo check` → OK, sin errores
- `cargo build` → OK, sin errores
- `cargo fmt` → OK, sin cambios
- `cargo clippy` (non-test) → OK, sin warnings

## Issue: Tests del QA no compilan

Los tests en `mod story022` tienen 3 errores de compilación `E0716`:

1. **`ca3_verbose_logs_lines_with_pipe_prefix`** (línea ~1758):
   ```rust
   let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
   ```
   El `MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, pero `from_utf8_lossy` devuelve `Cow<str>` que lo referencía.

2. **`ca3_empty_lines_not_logged`** (línea ~1810): mismo patrón.

3. **`ca5_stderr_not_streamed_to_log`** (línea ~2010): mismo patrón.

**Solución esperada** (a cargo del QA):
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

La implementación del Developer **no** corrige estos tests porque es responsabilidad del QA.
El estado de la historia se mantiene en **Tests Ready** hasta que el QA corrija los tests.
