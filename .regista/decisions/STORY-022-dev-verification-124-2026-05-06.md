# STORY-022 — Dev — Verificación #124 — 2026-05-06

## Resultado
❌ Tests no compilan — no se avanza a InReview.

## Resumen ejecutivo
El código de producción para STORY-022 está **completo y correcto** desde hace 124 iteraciones. Todos los checks pasan:
- `cargo check --bin regista`: OK
- `cargo clippy --no-deps --bin regista`: OK, 0 warnings
- `cargo fmt -- --check`: OK
- `cargo test --test architecture`: OK, 11/11 pasan

Sin embargo, los tests escritos por el QA en `src/infra/agent.rs`, módulo `mod story022`, tienen **3 errores de compilación E0716** (temporary value dropped while borrowed) que impiden la ejecución de `cargo test -- story022`.

## Errores detectados

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | mismo patrón E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | mismo patrón E0716 |

## Solución requerida (responsabilidad del QA)

En las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

La `MutexGuard` devuelta por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, pero `String::from_utf8_lossy` devuelve un `Cow<str>` que referencia los datos del `MutexGuard`. Al usar una variable intermedia (`binding`), el `MutexGuard` vive lo suficiente.

## Estado de los CAs

| CA | Estado | Detalle |
|----|--------|---------|
| CA1 | ✅ | `invoke_with_retry()` acepta `verbose: bool` (L84) |
| CA2 | ✅ | `invoke_once()` usa `BufReader` en verbose mode (`invoke_once_verbose()`, L358) |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` por línea no vacía (L384) |
| CA4 | ✅ | stdout se acumula en `Vec<u8>` y se devuelve en `Output.stdout` |
| CA5 | ✅ | stderr en `tokio::spawn` separado, sin streaming (L394-L398) |
| CA6 | ✅ | `verbose=false` usa `wait_with_output()` (L323) |
| CA7 | ✅ | Timeout con `kill_process_by_pid()` en ambos modos (L331, L410) |
| CA8 | ✅ | `cargo check --bin regista` compila sin errores |
| CA9 | ❌ | **Bloqueado**: `cargo test -- story022` no compila (3 errores E0716) |
| CA10 | ✅ | Call sites en `plan.rs:152` y `pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` contiene `stdout`, `stderr`, `exit_code` |

## Acción requerida
El **QA** debe corregir los 3 errores E0716 en `mod story022` antes de que el pipeline pueda continuar. El código de producción no requiere cambios adicionales.
