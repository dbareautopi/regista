# STORY-022 — Dev — verificación #97 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan

## Verificación del código de producción

### `cargo check --bin regista`
OK, sin errores.

### `cargo clippy --no-deps --bin regista`
OK, 0 warnings.

### `cargo fmt -- --check`
OK, código formateado.

### `cargo test --test architecture`
OK, 11/11 tests pasan.

### `cargo test -- story022`
NO compila — 3 errores E0716 en `mod story022`.

## Código de producción — estado

CA1-CA8, CA10-CA11: COMPLETOS Y CORRECTOS.

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry(verbose: bool)` como último parámetro | ✅ OK |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` | ✅ OK |
| CA3 | `tracing::info!("  │ {}", trimmed)` por línea no vacía | ✅ OK |
| CA4 | stdout acumulado en `Vec<u8>` y devuelto | ✅ OK |
| CA5 | stderr en `tokio::spawn` sin streaming | ✅ OK |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ OK |
| CA7 | Timeout funciona en ambos modos (`kill_process_by_pid`) | ✅ OK |
| CA8 | `cargo check --lib` compila | ✅ OK |
| CA9 | `cargo test --lib infra::agent` pasa | ❌ BLOQUEADO (tests QA no compilan) |
| CA10 | Call sites actualizados con `verbose` | ✅ OK |
| CA11 | `AgentResult` contiene `stdout`, `stderr`, `exit_code` | ✅ OK |

- `invoke_with_retry()` (L84): `verbose: bool` como último parámetro (CA1)
- `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado (CA1, CA10)
- `invoke_once()` (L290): dispatch `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6)
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` bucle async, `tracing::info!("  │ {}", trimmed)` por línea no vacía, `Vec<u8>` acumulado, stderr en `tokio::spawn` sin streaming (CA2-CA5)
- `kill_process_by_pid()` (L440): helper cross-platform para timeout (CA7)
- Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10)
- Call sites en tests pre-existentes pasan `false` (CA10)
- `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11)
- `Cargo.toml`: feature `io-util` en tokio (CA2)

## Errores en tests del QA

Errores E0716 (temporary value dropped while borrowed) — NO corregidos (responsabilidad del QA, 97ª iteración):

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | mismo error E0716 |

## Solución exacta (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones (líneas 1763, 1809, 2006).

## CA9 bloqueado
`cargo test -- story022` no puede verificarse hasta que el QA corrija los 3 errores de compilación.

## Acción
NO se avanza a In Review. El orquestador debe pasar el turno al QA.
