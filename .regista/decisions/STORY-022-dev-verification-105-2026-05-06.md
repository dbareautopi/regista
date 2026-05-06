# STORY-022 — Dev — 2026-05-06T00:00:00

## Resultado
❌ Tests del QA no compilan — no se avanza a In Review.

## Verificación del código de producción

### Compilación
- `cargo check`: OK (0.28s)
- `cargo clippy --no-deps --bin regista`: OK, 0 warnings
- `cargo fmt -- --check`: OK

### Tests de arquitectura
- `cargo test --test architecture`: 11/11 pasan

### Implementación (CA1-CA8, CA10-CA11 satisfechos)

| CA | Estado | Ubicación |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry()` L84: `verbose: bool` último parámetro |
| CA1 | ✅ | `invoke_with_retry_blocking()` L199: `verbose: bool` propagado |
| CA2 | ✅ | `invoke_once()` L316: `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` |
| CA2 | ✅ | `invoke_once_verbose()` L358: `child.stdout.take()` + `BufReader::new()` + `read_line()` bucle async |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` por línea no vacía |
| CA4 | ✅ | stdout acumulado en `Vec<u8>`, devuelto en `Output` |
| CA5 | ✅ | stderr en `tokio::spawn`, sin streaming, acumulado en `Vec<u8>` |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` |
| CA7 | ✅ | `kill_process_by_pid()` cross-platform en timeout (ambos modos) |
| CA8 | ✅ | `cargo check` compila sin errores |
| CA10 | ✅ | `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA10 | ✅ | Tests pre-existentes pasan `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` |
| tokio | ✅ | `Cargo.toml`: feature `io-util` habilitado |

## Errores en tests del QA (NO corregidos)

Los 3 errores son idénticos: `E0716: temporary value dropped while borrowed`.

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                        ^^^^^^^^^^^^^^^^^^^^^^ 
//                                        MutexGuard temporal destruido,
//                                        Cow<str> lo sigue referenciando
```

### Ubicaciones exactas

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();   // ← extiende la vida del MutexGuard
let log_output = String::from_utf8_lossy(&binding);
```

## CA9: bloqueado

`cargo test -- story022` no puede ejecutarse hasta que el QA corrija los 3 errores E0716.

## Decisión

NO se avanza el estado a In Review. El orquestador debe pasar el turno al QA
para que corrija los tests. La implementación de producción está completa y
verificada (build, clippy, fmt, architecture tests).
