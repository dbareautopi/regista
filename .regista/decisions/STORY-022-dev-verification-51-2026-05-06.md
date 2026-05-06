# STORY-022 — Dev Verification #51 — 2026-05-06

## Resultado
❌ Tests del QA no compilan — NO se avanza a In Review

## Verificación del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` (0.17s) | ✅ OK, sin errores |
| `cargo build` (0.17s) | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` (0.29s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.04s) | ✅ OK, 11/11 pasan |

## Cobertura de CAs por código de producción

| CA | Estado | Ubicación |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry()` L79: `verbose: bool` como último parámetro |
| CA2 | ✅ | `invoke_once_verbose()` L358: `BufReader::new()` + `read_line()` |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` en `invoke_once_verbose()` |
| CA4 | ✅ | stdout acumulado en `Vec<u8>`, devuelto en `Output` |
| CA5 | ✅ | stderr en `tokio::spawn` separado con `read_to_end()` |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` |
| CA7 | ✅ | `kill_process_by_pid()` cross-platform en ambos modos |
| CA8 | ✅ | `cargo check` pasa |
| CA9 | ❌ | Bloqueado: tests del QA no compilan |
| CA10 | ✅ | `plan.rs:152` y `pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` conserva `stdout`, `stderr`, `exit_code` |

## Errores en tests del QA (NO corregidos)

Los 3 errores E0716 provienen del mismo anti-patrón:

```rust
// ❌ INCORRECTO (el MutexGuard temporal muere antes que el Cow<str>)
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ CORRECTO
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

### Tests afectados

| Test | Línea |
|------|-------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| `ca3_empty_lines_not_logged` | 1809 |
| `ca5_stderr_not_streamed_to_log` | 2006 |

## Decisión

NO se corrige el código de tests — es responsabilidad del QA.
NO se avanza a In Review. El orquestador debe pasar el turno al QA.
