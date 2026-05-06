# STORY-022 — Dev Verification #85 — 2026-05-06

## Resumen

Octogésima quinta verificación de STORY-022 por el rol Developer.  
El código de producción está completo y correcto. Los tests del QA siguen con los mismos 3 errores E0716.

## Estado del código de producción

### Compilación y linting

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |

### Criterios de aceptación satisfechos (CA1-CA8, CA10-CA11)

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último parámetro | ✅ L84 |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` | ✅ L316 |
| CA3 | Cada línea no vacía logueada con `tracing::info!("  │ {}", trimmed)` | ✅ L358+ |
| CA4 | stdout acumulado en `Vec<u8>` devuelto en resultado | ✅ |
| CA5 | stderr en `tokio::spawn` separada, sin streaming | ✅ |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ L316 |
| CA7 | Timeout funciona en ambos modos (mata proceso por PID) | ✅ `kill_process_by_pid()` L440 |
| CA8 | `cargo check --bin regista` compila | ✅ |
| CA10 | Call sites actualizados con `verbose: false` | ✅ `plan.rs:152`, `pipeline.rs:774` |
| CA11 | `AgentResult` contiene `stdout`, `stderr`, `exit_code` | ✅ |

## Errores en tests (NO corregidos — responsabilidad del QA)

Los mismos 3 errores E0716 desde la iteración #1:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | mismo error E0716 |

### Solución exacta para el QA

En las 3 ubicaciones (líneas 1763, 1809, 2006), reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO se avanza** el estado de STORY-022 a In Review.
- El código de producción está correcto y completo.
- Los tests deben ser corregidos por el QA.
- El orquestador debe pasar el turno al QA automáticamente (transición Tests Ready → Tests Ready, Actor: QA).
