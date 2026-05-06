# STORY-022 — Dev — 2026-05-06 (Verificación 104)

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Resumen de verificación

### Producción: ✅ completo y correcto

| Verificación | Resultado |
|-------------|-----------|
| `cargo check --bin regista` | ✅ 0.17s, sin errores |
| `cargo clippy --no-deps --bin regista` | ✅ 0.27s, 0 warnings |
| `cargo fmt -- --check` | ✅ formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |

### CAs de producción cumplidos:

| CA | Descripción | Evidencia |
|----|-----------|-----------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | L78: `verbose: bool` como último parámetro |
| CA1 | `invoke_with_retry_blocking()` propaga `verbose` | L193: `verbose: bool` propagado |
| CA2 | `verbose=true` usa `invoke_once_verbose()` | L316: branch condicional |
| CA3 | Cada línea stdout se loguea con `  │ ` | L385: `tracing::info!("  │ {}", trimmed)` |
| CA4 | stdout acumulado en `Vec<u8>` | L382: `accumulated.extend_from_slice()` |
| CA5 | stderr en `tokio::spawn` sin streaming | L401: `tokio::spawn` con `read_to_end` |
| CA6 | `verbose=false` usa `wait_with_output()` | L316: branch false directo |
| CA7 | Timeout funciona en ambos modos | L316 + L420: `tokio::time::timeout` |
| CA8 | `cargo check` compila | ✅ |
| CA10 | Call sites actualizados | `plan.rs:152`, `pipeline.rs:774` pasan `false` |
| CA11 | `AgentResult` mantiene estructura | `stdout: String, stderr: String, exit_code: i32` |

### Tests del QA: ❌ no compilan

3 errores E0716 (`temporary value dropped while borrowed`):

| # | Test | Línea | Código con error |
|---|------|-------|------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

**Causa**: El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se
destruye al final del statement, pero `String::from_utf8_lossy()` devuelve un `Cow<str>`
que lo referencia. El borrow checker impide compilar.

**Solución (responsabilidad del QA)**:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```
en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Decisión

- **NO** se corrigen los tests — es responsabilidad del QA.
- **NO** se avanza el estado a In Review.
- El orquestador debe pasar el turno al QA automáticamente (transición 5: Tests Ready → Tests Ready).
- El código de producción está completo, correcto, y verificado (compila, clippy, fmt, arquitectura).
- CA9 (`cargo test -- story022`) bloqueado hasta que el QA corrija los 3 errores de compilación.
