# STORY-022 — Verificación de implementación #35 — 2026-05-05

## Estado
❌ Tests del QA no compilan — turno debe pasar al QA.

## Verificación de código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check` | ✅ OK (0.20s) |
| `cargo build` | ✅ OK (0.39s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.42s) |
| `cargo fmt -- --check` | ✅ OK, formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ 3 errores E0716 |

## Código de producción — cobertura CA1-CA8, CA10-CA11

- **CA1**: `invoke_with_retry()` (L78): `verbose: bool` como último parámetro ✅
- **CA1**: `invoke_with_retry_blocking()` (L193): `verbose: bool` propagado ✅
- **CA2**: `invoke_once()` (L316): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` ✅
- **CA2**: `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async ✅
- **CA3**: Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` ✅
- **CA4**: stdout acumulado en `Vec<u8>` y devuelto en `Output` ✅
- **CA5**: stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming ✅
- **CA6**: `verbose=false` → `wait_with_output()` (comportamiento actual) ✅
- **CA7**: `kill_process_by_pid()` (L440) para timeout en ambos modos ✅
- **CA8**: `cargo check` pasa ✅
- **CA10**: Call sites `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` ✅
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` ✅
- **Cargo.toml**: `io-util` en features de tokio ✅

## Errores E0716 en tests del QA (NO corregidos — responsabilidad del QA)

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

### Solución exacta (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```
en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Decisión

NO se avanza a In Review. El código de producción está completo y correcto.
Los 3 errores de compilación están en los tests escritos por QA y deben ser corregidos por QA.
El orquestador debe pasar el turno al QA para que corrija los tests.
