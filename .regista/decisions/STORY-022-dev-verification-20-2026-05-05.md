# STORY-022 — Vigésima verificación del Developer

**Fecha**: 2026-05-05
**Rol**: Developer
**Historia**: STORY-022 — Streaming de stdout del agente en `invoke_once()` + parámetro `verbose`

## Resumen

Verificación completa del código de producción para STORY-022. El código de producción
está completo y correcto, cubriendo todos los criterios de aceptación implementables
(CA1-CA8, CA10-CA11). Sin embargo, los tests del QA en `mod story022` siguen sin compilar
debido a 3 errores E0716 idénticos, 20 iteraciones después de que el QA los escribiera.

## Verificaciones de producción

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo check` | OK, sin errores | 4.69s |
| `cargo build` | OK, binario generado | 0.41s |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings | 0.39s |
| `cargo fmt -- --check` | OK, código formateado | — |
| `cargo test --test architecture` | OK, 11/11 pasan | 0.34s |

## Código de producción implementado

### `Cargo.toml`
- Feature `io-util` añadido a tokio (requerido para `BufReader`, `AsyncBufReadExt`, `AsyncReadExt`).

### `infra/agent.rs` — `invoke_with_retry()` (L78)
- Nuevo parámetro `verbose: bool` como último argumento.
- Propagado a `invoke_once()`.

### `infra/agent.rs` — `invoke_with_retry_blocking()` (L193)
- Nuevo parámetro `verbose: bool`, propagado a `invoke_with_retry()`.

### `infra/agent.rs` — `invoke_once()` (L316)
- Nuevo parámetro `verbose: bool`.
- `verbose = false`: usa `child.wait_with_output()` (comportamiento actual, sin cambios).
- `verbose = true`: delega en `invoke_once_verbose()`.
- Timeout vía `tokio::time::timeout` en ambas ramas.

### `infra/agent.rs` — `invoke_once_verbose()` (L358)
- `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
- Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)`.
- stdout acumulado en `Vec<u8>` y devuelto como parte del `Output`.
- stderr leído en `tokio::spawn` separado con `read_to_end()`, sin streaming.
- Timeout vía `tokio::time::timeout` sobre `child.wait()` + `kill_process_by_pid()`.

### `infra/agent.rs` — `kill_process_by_pid()` (L440)
- Helper extraído para timeout cross-platform.
- Unix: `kill -9 <pid>`, Windows: `taskkill /PID <pid> /F`.

### Call sites actualizados (CA10)
- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`.
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await`.

### `AgentResult` (CA11)
- Mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.

## Errores en tests del QA

Los siguientes 3 tests en `mod story022` de `src/infra/agent.rs` no compilan
con error `E0716: temporary value dropped while borrowed`:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz

`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal que se destruye
al final del statement, pero `String::from_utf8_lossy()` devuelve un `Cow<str>` que
referencia los datos del `MutexGuard`. El borrow checker detecta que el `Cow<str>`
sobrevive al `MutexGuard` y rechaza la compilación.

### Solución requerida

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

El `let binding` extiende la vida del `MutexGuard` hasta que `log_output` (un `Cow<str>`)
ya no se usa.

### Responsabilidad

La corrección de estos tests es responsabilidad del **QA Engineer**. El Developer
NO debe modificar los tests. El orquestador debe pasar el turno al QA para que
corrija los 3 errores E0716.

## Decisión

NO se avanza el estado a **In Review**. La historia permanece en **Tests Ready**.
El código de producción está completo y correcto, pero la verificación CA9
(`cargo test --lib infra::agent`) está bloqueada hasta que el QA corrija los
tests.

El orquestador (regista) debe detectar esta situación automáticamente y pasar
el turno al QA Engineer.
