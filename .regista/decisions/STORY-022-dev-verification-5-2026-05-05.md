# STORY-022 — Developer Verification #5 — 2026-05-05

## Resumen

Quinta verificación completa del código de producción para STORY-022.  
El código de producción está completo y correcto. Los tests del QA (`mod story022`) no compilan por 3 errores E0716.

---

## Verificaciones realizadas

### Compilación

```bash
$ cargo check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
```

### Build

```bash
$ cargo build
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
```

### Clippy

```bash
$ cargo clippy --no-deps
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```
0 warnings.

### Formato

```bash
$ cargo fmt -- --check
(sin output — formateado correctamente)
```

### Tests del QA

```bash
$ cargo test -- story022
error[E0716]: temporary value dropped while borrowed
```

3 errores E0716 (ver abajo).

---

## Implementación de producción (ya existente)

### `Cargo.toml`
```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "process", "time", "fs", "io-util"] }
```

### `invoke_once()` (L316)
- Parámetro `verbose: bool` como 5º argumento después de `timeout`.
- `verbose=false` → `child.wait_with_output()` con `tokio::time::timeout`.
- `verbose=true` → delega a `invoke_once_verbose()`.

### `invoke_once_verbose()` (L358)
- `child.stdout.take()` → `BufReader::new()` → `read_line()` en bucle async.
- Cada línea no vacía: `tracing::info!("  │ {}", trimmed)`.
- stdout completo acumulado en `Vec<u8>`.
- `child.stderr.take()` → `tokio::spawn` con `read_to_end()`, sin streaming al log.
- Timeout vía `tokio::time::timeout(child.wait())`.
- `kill_process_by_pid()` para matar el proceso en timeout.

### `kill_process_by_pid()` (L440)
- Helper cross-platform: `kill -9` (Unix), `taskkill` (Windows).

### `invoke_with_retry()` (L78)
- `verbose: bool` como 6º parámetro (después de `opts`).
- Propagado a `invoke_once()`.

### `invoke_with_retry_blocking()` (L193)
- `verbose: bool` como 6º parámetro.
- Propagado a `invoke_with_retry()` vía `RUNTIME.block_on()`.

### Call sites actualizados
- `src/app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`
- `src/app/pipeline.rs:774`: `invoke_with_retry(..., false).await`

---

## Cobertura de CAs por la implementación de producción

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ |
| CA2 | `BufReader::new()` + `read_line()` en bucle async | ✅ |
| CA3 | `tracing::info!("  │ {}", trimmed)` para líneas no vacías | ✅ |
| CA4 | stdout acumulado en `Vec<u8>` devuelto en resultado | ✅ |
| CA5 | stderr en `tokio::spawn`, sin streaming, acumulado | ✅ |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ |
| CA7 | timeout funciona en ambos modos | ✅ |
| CA8 | `cargo check` compila | ✅ |
| CA9 | tests del QA pasan | ❌ (3 E0716) |
| CA10 | call sites actualizados con `verbose` | ✅ |
| CA11 | `AgentResult` tiene `stdout`, `stderr`, `exit_code` | ✅ |

---

## Errores en los tests del QA

Los 3 errores son idénticos en naturaleza (`E0716: temporary value dropped while borrowed`):

### Error 1 — L1763 (`ca3_verbose_logs_lines_with_pipe_prefix`)
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement.  
`from_utf8_lossy` devuelve `Cow<'_, str>` que referencia el `&[u8]` del `MutexGuard`.  
Cuando el `MutexGuard` se destruye, el `Cow<str>` queda con una referencia colgante.

### Error 2 — L1809 (`ca3_empty_lines_not_logged`)
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
Mismo error.

### Error 3 — L2006 (`ca5_stderr_not_streamed_to_log`)
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
Mismo error.

### Solución exacta (responsabilidad del QA)

Reemplazar:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Esto extiende la vida del `MutexGuard` a través de la variable `binding`, que vive hasta el final del bloque (igual que `log_output`).

---

## Decisión

**NO se avanza a In Review.** El código de producción está completo y correcto, pero los tests del QA no compilan. La corrección de los 3 errores E0716 es responsabilidad del QA Engineer. El orquestador debe pasar el turno al QA.
