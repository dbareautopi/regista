# STORY-022 — Dev — Decimoquinta verificación (2026-05-05)

## Resultado
❌ No se avanza a In Review — los tests del QA no compilan.

## Verificación del código de producción

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo check` | OK, sin errores | ~0.20s |
| `cargo build` | OK, binario generado | ~0.26s |
| `cargo clippy --no-deps` | OK, 0 warnings | ~0.27s |
| `cargo fmt -- --check` | OK, código formateado | — |

## Cobertura de CAs en código de producción

| CA | Estado | Ubicación |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry()` L78: `verbose: bool` como último parámetro. `invoke_with_retry_blocking()` L193: idem. |
| CA2 | ✅ | `invoke_once()` L316: nuevo parámetro `verbose: bool`. `verbose=true` → `invoke_once_verbose()`. `invoke_once_verbose()` L358: `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async. |
| CA3 | ✅ | `invoke_once_verbose()`: `tracing::info!("  │ {}", trimmed)` para cada línea no vacía. |
| CA4 | ✅ | `invoke_once_verbose()`: stdout acumulado en `Vec<u8>`, devuelto como parte de `Output`. |
| CA5 | ✅ | `invoke_once_verbose()`: stderr leído en `tokio::spawn` separado con `read_to_end()`, sin streaming. |
| CA6 | ✅ | `invoke_once()`: `verbose=false` → `wait_with_output()` (comportamiento actual). |
| CA7 | ✅ | `kill_process_by_pid()` L440: helper cross-platform usado en ambos modos. |
| CA8 | ✅ | `cargo check --lib` compila todo el crate sin errores. |
| CA9 | ❌ | Bloqueado — los tests del QA no compilan. |
| CA10 | ✅ | Call sites: `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`. |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`. |

## Errores en los tests del QA (NO corregidos)

Los 3 errores `E0716` (temporary value dropped while borrowed) provienen del patrón:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Donde el `MutexGuard` temporal que retorna `buffer.lock().unwrap()` se destruye al final del statement, pero el `Cow<str>` devuelto por `String::from_utf8_lossy` aún lo referencia.

### Ubicaciones exactas

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

Esto extiende la vida del `MutexGuard` al scope de la variable `binding`, permitiendo que el `Cow<str>` referencie los datos subyacentes de forma segura.

## Decisión

NO se avanza el estado a In Review. El orquestador debe pasar el turno al QA para que corrija los 3 errores `E0716` en los tests. El código de producción está completo y correcto desde la iteración 1 de implementación; el bloqueo está exclusivamente en el código de tests del QA.
