# STORY-022 — Dev — 117ª verificación — 2026-05-06

## Resultado
❌ CA9 bloqueado — los tests del QA no compilan (3 errores E0716)

## Verificaciones del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check --bin regista` (0.16s) | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` (0.30s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.05s) | ✅ OK, 11/11 pasan |

## Estado del código de producción (CA1-CA8, CA10-CA11)

- `invoke_with_retry()` (L84): `verbose: bool` como último parámetro (CA1).
- `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado (CA1, CA10).
- `invoke_once()` (L290): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6).
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async, `tracing::info!("  │ {}", trimmed)` por línea no vacía, `Vec<u8>` acumulado, stderr en `tokio::spawn` sin streaming (CA2-CA5).
- `kill_process_by_pid()` (L440): helper cross-platform para timeout (CA7).
- Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10).
- Call sites en tests pre-existentes pasan `false` (CA10).
- `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11).
- `Cargo.toml`: feature `io-util` en tokio (CA2).

## Errores E0716 en tests del QA (NO corregidos — responsabilidad del QA, 117ª iteración)

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | mismo error E0716 |

### Solución exacta (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Decisión
NO se avanza a In Review. El orquestador debe pasar el turno al QA.
