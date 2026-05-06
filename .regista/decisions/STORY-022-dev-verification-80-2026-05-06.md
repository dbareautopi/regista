# STORY-022 — Dev Verification #80 — 2026-05-06

## Resultado
❌ Tests del QA no compilan — no se avanza a In Review.

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check` (0.34s) | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` (0.26s) | ✅ 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` (0.05s) | ✅ 11/11 |
| `cargo test -- story022` | ❌ NO compila |

## Criterios de aceptación cubiertos

| CA | Estado | Evidencia |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry(L84)` y `invoke_with_retry_blocking(L199)` aceptan `verbose: bool` |
| CA2 | ✅ | `invoke_once()` → `invoke_once_verbose()` usa `BufReader` + `read_line()` |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` por línea no vacía |
| CA4 | ✅ | `Vec<u8>` acumulado devuelto en `Output.stdout` |
| CA5 | ✅ | stderr en `tokio::spawn`, acumulado en `Vec<u8>`, sin streaming |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` |
| CA7 | ✅ | `kill_process_by_pid()` cross-platform en timeout |
| CA8 | ✅ | `cargo check --lib` compila |
| CA9 | ❌ | Bloqueado: tests del QA no compilan |
| CA10 | ✅ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` tiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en tests del QA

Los 3 tests afectados usan `String::from_utf8_lossy(&buffer.lock().unwrap())` que produce un `MutexGuard` temporal que se destruye antes que el `Cow<str>` retornado.

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Corrección necesaria (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza a In Review. El orquestador debe pasar el turno al QA (transición Tests Ready → Tests Ready, fix QA).

## Stack

- regista v0.24.0
- Rust 1.78.0
- tokio con feature `io-util`
