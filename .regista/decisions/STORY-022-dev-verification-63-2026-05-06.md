# STORY-022 Dev Verification #63 — 2026-05-06

## Resumen

Sexagésima tercera verificación del código de producción para STORY-022. El código de producción está completo y correcto desde la primera iteración, pero los tests del QA tienen 3 errores de compilación E0716 que impiden ejecutarlos.

## Estado del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` | OK, 0.24s |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings |
| `cargo fmt -- --check` | OK |
| `cargo test --test architecture` | OK, 11/11 |
| `cargo test -- story022` | NO compila |

## CAs cubiertos por el código de producción

| CA | Estado | Evidencia |
|----|--------|-----------|
| CA1 | ✓ | `invoke_with_retry()` (L84) y `invoke_with_retry_blocking()` (L199) aceptan `verbose: bool` |
| CA2 | ✓ | `invoke_once()` delega a `invoke_once_verbose()` con `BufReader::new()` + `read_line()` |
| CA3 | ✓ | `tracing::info!("  │ {}", trimmed)` por cada línea no vacía |
| CA4 | ✓ | stdout acumulado en `Vec<u8>` vía `accumulated.extend_from_slice()` |
| CA5 | ✓ | stderr leído en `tokio::spawn` separado con `read_to_end()`, sin streaming |
| CA6 | ✓ | `verbose=false` → `child.wait_with_output()` con `tokio::time::timeout` |
| CA7 | ✓ | `kill_process_by_pid()` extraído, llamado en rama `Err(_elapsed)` de ambos modos |
| CA8 | ✓ | `cargo check` compila sin errores |
| CA9 | ✗ | Bloqueado por errores E0716 en tests del QA |
| CA10 | ✓ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✓ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en tests del QA

Los 3 errores son idénticos (E0716: temporary value dropped while borrowed) y ocurren al usar `String::from_utf8_lossy(&buffer.lock().unwrap())` donde el `MutexGuard` temporal se destruye antes que el `Cow<str>`:

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución requerida (responsabilidad del QA)

En cada una de las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO** se corrigen los tests. Es responsabilidad del QA.
- **NO** se avanza el estado de Tests Ready a In Review.
- El orquestador debe detectar la entrada en el Activity Log y pasar el turno al QA para la transición `Tests Ready → Tests Ready` (fix de tests, transición #5).
