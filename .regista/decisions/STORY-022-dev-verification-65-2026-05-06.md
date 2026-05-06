# STORY-022 — Developer — 2026-05-06T00:00:00

## Resultado
❌ Tests del QA no compilan — NO se avanza a In Review

## Verificación del código de producción

### Checks estáticos
| Comando | Resultado |
|---------|-----------|
| `cargo check` (0.47s) | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` (0.50s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.04s) | ✅ OK, 11/11 pasan |

### Cobertura de CAs (producción)
| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último parámetro | ✅ `src/infra/agent.rs:84` |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` | ✅ `src/infra/agent.rs:316-358` |
| CA3 | Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)` | ✅ `src/infra/agent.rs:383` |
| CA4 | Stdout acumulado en `Vec<u8>` y devuelto en `Output` | ✅ `src/infra/agent.rs:370-395` |
| CA5 | Stderr en `tokio::spawn` separado, sin streaming | ✅ `src/infra/agent.rs:398-403` |
| CA6 | `verbose=false` → `wait_with_output()` (sin cambios) | ✅ `src/infra/agent.rs:323-339` |
| CA7 | Timeout cross-platform con `kill_process_by_pid()` | ✅ `src/infra/agent.rs:440-456` |
| CA8 | `cargo check --lib` compila | ✅ |
| CA10 | Call sites actualizados con `verbose` | ✅ `app/plan.rs:152`, `app/pipeline.rs:774` |
| CA11 | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` | ✅ `src/infra/agent.rs:34-48` |

### Dependencia tokio `io-util`
✅ `Cargo.toml`: `tokio = { version = "1", features = [..., "io-util"] }`

## Errores en tests del QA (NO corregidos — responsabilidad del QA)

### Error E0716: temporary value dropped while borrowed

Los 3 tests comparten el mismo bug: `String::from_utf8_lossy(&buffer.lock().unwrap())` donde
el `MutexGuard` temporal de `lock().unwrap()` se destruye antes que el `Cow<str>` retornado
por `from_utf8_lossy`, causando un borrow dangling.

| # | Test | Línea | Código con error |
|---|------|-------|------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |

### Solución (responsabilidad del QA)

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

NO se avanza a In Review. El orquestador debe pasar el turno al QA para que corrija
los 3 errores E0716 en los tests del módulo `story022`.
