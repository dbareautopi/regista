# STORY-022 — Dev Verification #45 — 2026-05-05

## Resultado
❌ Tests NO compilan — no se avanza a In Review.

## Verificación del código de producción

| Verificación | Resultado |
|---|---|
| `cargo build` | ✅ OK (0.18s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ OK (11/11) |
| `cargo test -- story022` | ❌ NO compila |

## Cobertura de CAs en producción

| CA | Estado | Implementación |
|----|--------|----------------|
| CA1 | ✅ | `invoke_with_retry()` (L78) y `invoke_with_retry_blocking()` (L193) aceptan `verbose: bool` |
| CA2 | ✅ | `invoke_once_verbose()` (L358) usa `child.stdout.take()` + `BufReader::new()` + `read_line()` |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` para cada línea no vacía |
| CA4 | ✅ | stdout acumulado en `Vec<u8>` y devuelto en `Output` |
| CA5 | ✅ | stderr en `tokio::spawn` separado, sin streaming al log |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` (comportamiento actual) |
| CA7 | ✅ | `kill_process_by_pid()` cross-platform en timeout para ambos modos |
| CA8 | ✅ | `cargo build` compila todo el crate |
| CA10 | ✅ | Todos los call sites actualizados: `app/plan.rs:152`, `app/pipeline.rs:774`, tests pre-existentes (L657, L686, L720, L864) |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores de compilación en tests (responsabilidad del QA)

Los 3 errores E0716 son idénticos al patrón documentado en las 44 iteraciones anteriores:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal |

### Solución exacta (responsabilidad del QA)

En las 3 ubicaciones, cambiar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## CA9 bloqueado

`cargo test -- story022` no puede verificarse hasta que el QA corrija los 3 errores de compilación E0716.

## Acción requerida
El orquestador debe pasar el turno al QA para que corrija los tests.
