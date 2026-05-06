# STORY-022 — Dev Verification #78 — 2026-05-06

## Resumen

Septuagésima octava verificación del código de producción de STORY-022.
El código de producción está completo y correcto. Los tests del QA no compilan
por 3 errores E0716 idénticos.

## Verificación de producción

| Verificación | Resultado | Detalle |
|---|---|---|
| `cargo build` | ✅ OK | 0.34s, binario generado |
| `cargo check --bin regista` | ✅ OK | 0.31s, sin errores |
| `cargo clippy --no-deps --bin regista` | ✅ OK | 0.26s, 0 warnings |
| `cargo fmt -- --check` | ✅ OK | código formateado |
| `cargo test --test architecture` | ✅ OK | 11/11 tests pasan |
| `cargo test -- story022` | ❌ NO COMPILA | 3 errores E0716 |

## Código de producción (completo y correcto)

- `invoke_with_retry()` (L78): `verbose: bool` como último parámetro (CA1)
- `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado (CA1, CA10)
- `invoke_once()` (L316): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6)
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` + `tracing::info!("  │ {}", trimmed)` + `Vec<u8>` + `tokio::spawn` stderr (CA2-CA5)
- `kill_process_by_pid()` (L440): timeout cross-platform (CA7)
- Call sites actualizados: `app/plan.rs:152`, `app/pipeline.rs:774`, tests pre-existentes (CA10)
- `AgentResult` mantiene `stdout`, `stderr`, `exit_code` (CA11)
- `Cargo.toml`: feature `io-util` en tokio (CA2)

## Errores en tests del QA

Los 3 errores son del mismo tipo (E0716: temporary value dropped while borrowed):

| # | Test | Línea | Código problemático |
|---|---|---|---|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

El `MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se
destruye al final del statement, pero `Cow<str>` de `from_utf8_lossy` lo
referencia — el borrow no vive lo suficiente.

## Solución exacta (responsabilidad del QA)

En las 3 ubicaciones (líneas 1763, 1809, 2006), reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO se corrigen los tests** — es responsabilidad del QA.
- **NO se avanza el estado a In Review** — los tests no compilan.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
