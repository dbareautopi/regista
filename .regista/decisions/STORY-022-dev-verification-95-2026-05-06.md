# STORY-022 — Dev — 95ª verificación — 2026-05-06

## Resultado
❌ No se avanza a In Review. Tests del QA no compilan (3 errores E0716).

## Verificación del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` | OK, sin errores |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings |
| `cargo fmt -- --check` | OK, código formateado |
| `cargo test --test architecture` | OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Código de producción — CA1-CA8, CA10-CA11: COMPLETO y CORRECTO

- **CA1**: `invoke_with_retry()` (L84): `verbose: bool` como último parámetro
- **CA10**: `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado
- **CA2, CA6**: `invoke_once()` (L290): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`
- **CA2-CA5**: `invoke_once_verbose()` (L358): `BufReader`, `read_line()`, `tracing::info!("  │ {}", trimmed)`, `Vec<u8>` acumulado, stderr en `tokio::spawn`
- **CA7**: `kill_process_by_pid()` (L440): cross-platform
- **CA10**: Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`
- **CA11**: `AgentResult` mantiene `stdout`, `stderr`, `exit_code`
- **CA8**: `Cargo.toml` tiene feature `io-util` en tokio

## Errores E0716 en tests del QA (NO corregidos — 95ª iteración)

| Test | Línea | Error |
|------|-------|-------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| `ca3_empty_lines_not_logged` | 1809 | mismo error E0716 |
| `ca5_stderr_not_streamed_to_log` | 2006 | mismo error E0716 |

## Solución exacta (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones (líneas 1763, 1809, 2006).

## CA9 bloqueado

`cargo test -- story022` no puede verificarse hasta que el QA corrija los 3 errores de compilación.
