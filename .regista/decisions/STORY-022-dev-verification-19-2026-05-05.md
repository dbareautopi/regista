# STORY-022 — Dev — Decimonovena verificación — 2026-05-05

❌ No se avanza a In Review — los tests del QA en `mod story022` no compilan.

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check` | ✅ OK (0.16s) |
| `cargo build` | ✅ OK (0.17s) |
| `cargo clippy --no-deps --bin regista` | ✅ 0 warnings (0.27s) |
| `cargo fmt -- --check` | ✅ Formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Código de producción: completo y correcto

El código de producción implementa correctamente todos los CA de STORY-022:

### `Cargo.toml`
- Feature `io-util` añadido a tokio (necesario para `BufReader`/`AsyncBufReadExt`).

### `infra/agent.rs`
- **`invoke_once()`** (L316): nuevo parámetro `verbose: bool`.
  - `verbose=false` → `child.wait_with_output()` vía `tokio::time::timeout` (comportamiento actual, sin cambios).
  - `verbose=true` → delega a `invoke_once_verbose()`.
- **`invoke_once_verbose()`** (L358): streaming línea a línea.
  - `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
  - Cada línea no vacía: `tracing::info!("  │ {}", trimmed)`.
  - stdout acumulado en `Vec<u8>` y devuelto en `Output`.
  - stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming.
- **`kill_process_by_pid()`** (L440): helper extraído para timeout cross-platform.
- **`invoke_with_retry()`** (L78): `verbose: bool` como último parámetro, propagado a `invoke_once()`.
- **`invoke_with_retry_blocking()`** (L193): `verbose: bool` propagado a `invoke_with_retry()`.

### Call sites
- `app/plan.rs:152`: pasa `false` (no necesita streaming).
- `app/pipeline.rs:774`: pasa `false` (no necesita streaming).

### `AgentResult`
- Mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11).

## Errores en tests del QA (`mod story022`)

Los 3 errores `E0716` (temporary value dropped while borrowed) están en el módulo `story022` del archivo `src/infra/agent.rs`.

Las 3 líneas afectadas usan el mismo patrón:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

En esta expresión, `buffer.lock().unwrap()` retorna un `MutexGuard<Vec<u8>>` temporal. `String::from_utf8_lossy` toma `&[u8]` por referencia (vía `Deref` del `MutexGuard`). El `MutexGuard` temporal se destruye al final del statement, pero el `Cow<str>` devuelto por `from_utf8_lossy` lo referencia, causando E0716.

### Ubicaciones exactas

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución requerida (responsabilidad del QA)

En las 3 ubicaciones, reemplazar por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Esto es exactamente lo que sugiere el compilador (`rustc --explain E0716`).

## Conclusión

- El código de producción es completo y correcto.
- Los tests del QA tienen 3 errores de compilación que **no son responsabilidad del Developer**.
- CA9 (`cargo test --lib infra::agent`) está bloqueado por los errores en los tests.
- **NO se avanza a In Review.** El orquestador debe pasar el turno al QA (transición: Tests Ready → Tests Ready, actor: QA, fix tests).
