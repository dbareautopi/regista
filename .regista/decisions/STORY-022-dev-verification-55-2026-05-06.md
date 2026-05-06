# STORY-022 — dev — 20260506T000000Z

## Resultado
❌ Fallo parcial — tests del QA no compilan (E0716 x3), código de producción completo y correcto.

## Verificación del código de producción

### CA1: `invoke_with_retry()` acepta `verbose: bool`
✅ `src/infra/agent.rs:84` — `verbose: bool` como último parámetro.

### CA2: `verbose=true` → `BufReader::new()` + `read_line()` en bucle async
✅ `src/infra/agent.rs:358-430` — `invoke_once_verbose()`:
- `child.stdout.take()` obtiene el pipe
- `BufReader::new(stdout)` envuelve el stream
- `reader.read_line(&mut line)` en bucle `loop {}`
- EOF detectado con `Ok(0) => break`

### CA3: `tracing::info!("  │ {}", trimmed)`
✅ `src/infra/agent.rs:378` — cada línea no vacía (`!trimmed.is_empty()`) se loguea con prefijo `  │ `.

### CA4: stdout acumulado en Vec<u8>
✅ `src/infra/agent.rs:373` — `accumulated.extend_from_slice(line.as_bytes())`.
✅ Devuelto como `Output.stdout` al terminar.

### CA5: stderr en `tokio::spawn` separado, sin streaming
✅ `src/infra/agent.rs:392-397` — `stderr_handle = tokio::spawn(...)` con `read_to_end()`. Sin llamadas a `tracing::info!`.

### CA6: `verbose=false` → `wait_with_output()`
✅ `src/infra/agent.rs:334` — `child.wait_with_output()` dentro de `tokio::time::timeout`.

### CA7: timeout funciona en ambos modos
✅ `src/infra/agent.rs:440-457` — `kill_process_by_pid()` cross-platform.
✅ Modo no-verbose (L334): `tokio::time::timeout(timeout, child.wait_with_output())`.
✅ Modo verbose (L400): `tokio::time::timeout(timeout, child.wait())`.

### CA8: `cargo check` compila
✅ `cargo check` (0.26s): sin errores.
✅ `cargo build` (0.34s): binario generado.
✅ `cargo clippy --no-deps --bin regista` (0.39s): 0 warnings.
✅ `cargo fmt -- --check`: formateado.

### CA10: call sites actualizados
✅ `src/app/plan.rs:152` — `invoke_with_retry_blocking(..., false)`.
✅ `src/app/pipeline.rs:774` — `invoke_with_retry(..., false)`.
✅ Tests pre-existentes pasan `false`.

### CA11: AgentResult mantiene campos
✅ `stdout: String`, `stderr: String`, `exit_code: i32`.

## Errores en tests del QA (NO corregidos)

Los tests del módulo `story022` no compilan por 3 errores E0716 idénticos:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal se destruye antes que el `Cow<str>` prestado |
| 2 | `ca3_empty_lines_not_logged` | 1809 | Ídem |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | Ídem |

### Causa raíz
`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal. `String::from_utf8_lossy()` toma `&[u8]` prestado de ese temporal. El temporal se destruye al final de la sentencia, pero el `Cow<str>` devuelto por `from_utf8_lossy` mantiene la referencia viva. El borrow checker lo rechaza (E0716).

### Solución para el QA
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```
Aplicar en las 3 ubicaciones: líneas 1763, 1809, 2006.

## CA9 bloqueado
`cargo test -- agent` no se puede ejecutar hasta que el QA corrija los 3 errores de compilación.

## Conclusión
El código de producción está completo y correcto. No se avanza a In Review. El orquestador debe pasar el turno al QA para que corrija los tests.
