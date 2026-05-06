# STORY-022 — Dev Verification #118 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (E0716, 3 errores).

---

## Verificación del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` (0.32s) | OK, sin errores |
| `cargo clippy --no-deps --bin regista` (0.26s) | OK, 0 warnings |
| `cargo fmt -- --check` | OK, código formateado |
| `cargo test --test architecture` (0.26s) | OK, 11/11 pasan |
| `cargo test -- story022` | **NO compila** — 3 errores E0716 |

---

## Código de producción — estado completo y correcto

Todas las CA de producción (CA1-CA8, CA10-CA11) están implementadas correctamente:

### CA1: `verbose: bool` en `invoke_with_retry()`
- `invoke_with_retry()` (L84): acepta `verbose: bool` como último parámetro
- `invoke_with_retry_blocking()` (L199): propaga el parámetro

### CA2-CA6: `invoke_once()` con modo verbose
- `invoke_once()` (L290): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async
- `tracing::info!("  │ {}", trimmed)` por línea no vacía, `Vec<u8>` acumulado
- stderr en `tokio::spawn` separado, sin streaming al log

### CA7: Timeout
- `kill_process_by_pid()` (L440): helper cross-platform (kill -9 / taskkill)

### CA10: Call sites
- `app/plan.rs:152`: pasa `false`
- `app/pipeline.rs:774`: pasa `false`
- Tests pre-existentes: pasan `false`

### CA11: AgentResult
- Mantiene `stdout: String`, `stderr: String`, `exit_code: i32`

### Cargo.toml
- `tokio` feature `io-util` presente

---

## Errores E0716 en tests del QA

Los 3 errores son idénticos en su causa raíz: `String::from_utf8_lossy()` recibe una referencia temporal a un `MutexGuard` que se destruye antes que el `Cow<str>` retornado.

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

---

## Decisión
- **NO** se corrigen los tests del QA. Es su responsabilidad.
- **NO** se avanza el estado a In Review.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
