# STORY-022 — Dev — Verificación 49 (2026-05-06)

## Resultado
❌ Tests del QA no compilan — no se puede avanzar a In Review.

## Verificación del código de producción

### Comandos ejecutados (todos OK)

| Comando | Resultado |
|---------|-----------|
| `cargo check` | OK, sin errores |
| `cargo build` | OK, binario generado |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings |
| `cargo fmt -- --check` | OK, código formateado |
| `cargo test --test architecture` | OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

### CA cubiertos por el código de producción

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último parámetro | ✅ L78 |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` async | ✅ L358 |
| CA3 | Líneas no vacías: `tracing::info!("  │ {}", trimmed)` | ✅ |
| CA4 | stdout acumulado en `Vec<u8>` | ✅ |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ L316 |
| CA7 | Timeout funciona en ambos modos (`kill_process_by_pid`) | ✅ L440 |
| CA8 | `cargo check` compila | ✅ |
| CA9 | `cargo test -- story022` pasa | ❌ Bloqueado por QA |
| CA10 | Call sites actualizados con `verbose` | ✅ plan.rs:152, pipeline.rs:774, tests |
| CA11 | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` | ✅ |

## Errores en tests del QA (NO corregidos — 49ª iteración)

Los 3 tests fallan con **E0716: temporary value dropped while borrowed**:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | ~1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | ~1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | ~2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa

`MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, pero `String::from_utf8_lossy` retorna un `Cow<str>` que puede referenciar el `MutexGuard`. El compilador no puede garantizar que el `Cow<str>` no sea un borrow.

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones.

## Decisión

NO se avanza a In Review. El código de producción está completo y correcto. El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716 en los tests.
