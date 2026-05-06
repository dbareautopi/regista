# STORY-022 — Developer — 94ª verificación — 2026-05-06

## Resumen

El código de producción para STORY-022 está **completo y correcto** (CA1-CA8, CA10-CA11).
Los tests del QA en `mod story022` (en `src/infra/agent.rs`) no compilan debido a
3 errores E0716.

## Verificaciones de producción

| Verificación | Resultado |
|---|---|
| `cargo check --bin regista` | ✅ OK (0.16s) |
| `cargo clippy --no-deps --bin regista` | ✅ 0 warnings |
| `cargo fmt -- --check` | ✅ formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |

## Código de producción implementado

### `invoke_with_retry()` — parámetro `verbose: bool` (CA1)
- Línea 84: `verbose: bool` como último parámetro.
- Se propaga a `invoke_once()`.

### `invoke_with_retry_blocking()` — parámetro `verbose: bool` (CA1, CA10)
- Línea 199: `verbose: bool` propagado al llamar a `RUNTIME.block_on(invoke_with_retry(...))`.

### `invoke_once()` — dispatch verbose/no-verbose (CA2, CA6)
- Línea 290: `verbose=false` → `child.wait_with_output()` con `tokio::time::timeout`.
- `verbose=true` → `invoke_once_verbose(child, pid, provider, timeout)`.

### `invoke_once_verbose()` — streaming de stdout (CA2-CA5)
- Línea 358: `child.stdout.take()` → `BufReader::new()` → bucle `read_line()`.
- Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)` (CA3).
- Líneas vacías se ignoran (no generan log).
- Stdout completo acumulado en `Vec<u8>` (CA4).
- Stderr leído en `tokio::spawn` separado, `read_to_end()`, sin streaming (CA5).

### `kill_process_by_pid()` — timeout cross-platform (CA7)
- Línea 440: `kill -9` en Unix, `taskkill /F` en Windows.

### Call sites actualizados (CA10)
- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`.
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await`.
- Tests pre-existentes: pasan `false`.

### `AgentResult` (CA11)
- `stdout: String`, `stderr: String`, `exit_code: i32` — sin cambios.

### `Cargo.toml` (CA2)
- `tokio` features incluyen `io-util`.

## Errores en los tests del QA (NO corregidos)

| # | Test | Línea | Error |
|---|---|---|---|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | E0716: temporary value dropped while borrowed |
| 2 | `ca3_empty_lines_not_logged` | 1809 | E0716: temporary value dropped while borrowed |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | E0716: temporary value dropped while borrowed |

### Causa

Las 3 líneas tienen la misma expresión:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

`buffer.lock().unwrap()` retorna un `MutexGuard<Vec<u8>>` temporal.
`String::from_utf8_lossy` retorna `Cow<'_, str>` que puede ser `Cow::Borrowed`
— tomando prestado del `Vec<u8>` dentro del `MutexGuard`. Cuando el temporal
`MutexGuard` se destruye al final del statement, el borrow queda inválido.

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones (líneas 1763, 1809, 2006 de `src/infra/agent.rs`).

## Decisión

**NO se avanza a In Review.** El código de producción está completo, pero CA9
(`cargo test -- story022` pasa) no puede verificarse porque los tests del QA
no compilan. El orquestador debe pasar el turno al QA para que corrija los
3 errores E0716.
