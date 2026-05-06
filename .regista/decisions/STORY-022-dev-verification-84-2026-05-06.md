# STORY-022 — Dev Verification #84 — 2026-05-06

## Resultado
❌ Tests del QA NO compilan — bloqueado, turno para QA.

## Verificaciones de producción

| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | ✅ OK (0.38s) |
| `cargo build` | ✅ OK (0.29s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ OK, 11/11 |
| `cargo test -- story022` | ❌ NO compila — 3 x E0716 |

## Código de producción: estado

El código de producción cubre CA1-CA8 y CA10-CA11 completamente:

- **CA1**: `invoke_with_retry(L84)` y `invoke_with_retry_blocking(L199)` aceptan `verbose: bool` como último parámetro.
- **CA2, CA6**: `invoke_once(L316)` — `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`.
- **CA2-CA5**: `invoke_once_verbose(L358)` — `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async, `tracing::info!("  │ {}", trimmed)` por línea no vacía, `Vec<u8>` acumulado, stderr en `tokio::spawn` sin streaming.
- **CA7**: `kill_process_by_pid(L440)` — helper cross-platform para timeout.
- **CA8**: `cargo check --bin regista` compila sin errores.
- **CA10**: Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.

## Errores en tests del QA (3 x E0716)

Los 3 errores son idénticos en naturaleza — `MutexGuard` temporal destruido antes de que `Cow<str>` de `String::from_utf8_lossy` pueda usarlo:

| # | Test | Línea | Expresión problemática |
|---|------|-------|----------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Solución (responsabilidad del QA)

En cada una de las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Acción

NO se avanza a In Review. El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
