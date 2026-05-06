# STORY-022 — Dev Verification #91 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Verificación de código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | ✅ OK (0.16s) |
| `cargo clippy --no-deps --bin regista` | ✅ 0 warnings |
| `cargo fmt -- --check` | ✅ Código formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Estado de los Criterios de Aceptación

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `verbose: bool` en `invoke_with_retry()` | ✅ Implementado (L84) |
| CA2 | `BufReader` + `read_line()` en modo verbose | ✅ Implementado (`invoke_once_verbose()`, L358) |
| CA3 | `tracing::info!("  │ {}", trimmed)` por línea | ✅ Implementado |
| CA4 | stdout acumulado en `Vec<u8>` | ✅ Implementado |
| CA5 | stderr en `tokio::spawn` sin streaming | ✅ Implementado |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ Implementado (L316) |
| CA7 | Timeout funciona en ambos modos | ✅ Implementado (`kill_process_by_pid()`, L440) |
| CA8 | `cargo check` compila | ✅ Verificado |
| CA9 | Tests pasan | ❌ Bloqueado por errores del QA |
| CA10 | Call sites actualizados | ✅ `plan.rs:152` y `pipeline.rs:774` pasan `false` |
| CA11 | `AgentResult` con `stdout`, `stderr`, `exit_code` | ✅ Sin cambios |

## Errores en tests del QA (NO corregidos — responsabilidad del QA)

Los siguientes 3 tests tienen el error E0716 (`temporary value dropped while borrowed`):

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa

`MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, pero `Cow<str>` retornado por `String::from_utf8_lossy` mantiene una referencia al `Vec<u8>` dentro del `MutexGuard`. El borrow checker de Rust (NLL) detecta que la referencia sobrevive al guard.

### Solución (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
// ❌ Actual (no compila)
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ Corrección
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza el estado a In Review. El código de producción está completo y correcto. El orquestador debe pasar el turno al QA para que corrija los 3 errores de compilación en los tests.
