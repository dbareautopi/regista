# STORY-022 — Dev — Centésima verificación — 2026-05-06

## Resultado
❌ NO se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Verificaciones del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` | ✅ OK (0.15s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.26s) |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan (0.06s) |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

## Código de producción: completo y correcto (CA1-CA8, CA10-CA11)

- `invoke_with_retry()` (L84): `verbose: bool` como último parámetro (CA1)
- `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado (CA1, CA10)
- `invoke_once()` (L316): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6)
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async, `tracing::info!("  │ {}", trimmed)` por línea no vacía, `Vec<u8>` acumulado, stderr en `tokio::spawn` sin streaming (CA2-CA5)
- `kill_process_by_pid()` (L440): helper cross-platform para timeout (CA7)
- Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10)
- Call sites en tests pre-existentes pasan `false` (CA10)
- `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11)
- `Cargo.toml`: feature `io-util` en tokio (CA2)

## Errores en tests del QA (NO corregidos por el Dev)

Los 3 errores son idénticos: `String::from_utf8_lossy(&buffer.lock().unwrap())` donde el `MutexGuard` temporal se destruye antes que el `Cow<str>` prestado.

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | E0716: temporary value dropped while borrowed |
| 2 | `ca3_empty_lines_not_logged` | 1809 | E0716: temporary value dropped while borrowed |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | E0716: temporary value dropped while borrowed |

### Solución exacta (responsabilidad del QA)

En las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Acción requerida

El orquestador debe pasar el turno al QA para que corrija los 3 errores de compilación.
Una vez corregidos, `cargo test -- story022` debería pasar y la historia avanzará a In Review.
