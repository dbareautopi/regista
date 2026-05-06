# STORY-022 — Dev Verification #102 — 2026-05-06

## Resultado
❌ Bloqueado — tests del QA no compilan (3 errores E0716)

## Verificaciones del código de producción

| Check | Tiempo | Resultado |
|-------|--------|-----------|
| `cargo check --bin regista` | 0.15s | ✅ OK |
| `cargo clippy --no-deps --bin regista` | 0.27s | ✅ 0 warnings |
| `cargo fmt -- --check` | — | ✅ OK |
| `cargo test --test architecture` | 0.17s | ✅ 11/11 pasan |
| `cargo test -- story022` | — | ❌ NO compila |

## Código de producción implementado

### `invoke_with_retry()` (L84)
- Último parámetro: `verbose: bool` (CA1)
- Se propaga a `invoke_once(provider, instruction_path, &current_prompt, timeout, verbose).await`

### `invoke_with_retry_blocking()` (L199)
- Último parámetro: `verbose: bool` (CA1)
- Se propaga a `RUNTIME.block_on(invoke_with_retry(..., verbose))` (CA10)

### `invoke_once()` (L316)
- Rama `verbose = false`: usa `child.wait_with_output()` con `tokio::time::timeout` (CA6)
- Rama `verbose = true`: delega a `invoke_once_verbose(child, pid, provider, timeout)` (CA2)
- Guarda PID antes del move para timeout cross-platform

### `invoke_once_verbose()` (L358)
- `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async (CA2)
- `tracing::info!("  │ {}", trimmed)` por cada línea no vacía (CA3)
- `Vec<u8>` acumulado como `stdout` en `Output` (CA4)
- stderr en `tokio::spawn` separada, sin streaming, acumulado en `Vec<u8>` (CA5)
- Timeout con `tokio::time::timeout` sobre `child.wait()` (CA7)

### `kill_process_by_pid()` (L440)
- Helper cross-platform: `kill -9` (Unix) / `taskkill` (Windows)
- Usado desde `invoke_once()` e `invoke_once_verbose()` en caso de timeout (CA7)

### Call sites actualizados (CA10)
- `app/plan.rs:152` → `invoke_with_retry_blocking(..., false)`
- `app/pipeline.rs:774` → `invoke_with_retry(..., false).await`
- Tests pre-existentes: pasan `false`

### `AgentResult` (CA11)
- `stdout: String`, `stderr: String`, `exit_code: i32` — sin cambios

### `Cargo.toml`
- `tokio` incluye feature `io-util` (necesario para `BufReader`)

## Errores en los tests del QA

Los 3 errores E0716 son idénticos en naturaleza: `String::from_utf8_lossy(&buffer.lock().unwrap())` 
crea un `MutexGuard` temporal que se destruye antes que el `Cow<str>` retornado por 
`from_utf8_lossy`, causando un dangling reference.

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — E0716 |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — E0716 |

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO** se corrigen los tests del QA (responsabilidad del QA según AGENTS.md).
- **NO** se avanza el estado a In Review.
- El orquestador debe pasar el turno al QA para que corrija los tests.
