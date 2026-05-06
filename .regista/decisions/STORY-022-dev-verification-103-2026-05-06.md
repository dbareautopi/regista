# STORY-022 — Dev — Verificación 103 — 2026-05-06

## Resultado
❌ Tests no compilan (E0716 x3) — no se avanza a In Review

## Verificaciones del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check --bin regista` | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ 3 errores E0716 de compilación |

## Criterios de aceptación cumplidos (producción)

| CA | Estado | Detalle |
|----|--------|---------|
| CA1 | ✅ | `invoke_with_retry()` (L84): `verbose: bool` como último parámetro |
| CA2 | ✅ | `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` por línea no vacía |
| CA4 | ✅ | `Vec<u8>` acumulado y devuelto como `Output.stdout` |
| CA5 | ✅ | `tokio::spawn` para stderr, sin streaming al log |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` (comportamiento actual) |
| CA7 | ✅ | `kill_process_by_pid()` cross-platform para timeout en ambos modos |
| CA8 | ✅ | `cargo check --bin regista` compila todo el crate sin errores |
| CA9 | ❌ | **BLOQUEADO**: tests del QA no compilan |
| CA10 | ✅ | Call sites en `plan.rs:152` y `pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en tests del QA (NO corregidos)

Los 3 errores son E0716 (temporary value dropped while still borrowed) en el módulo `story022`:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | mismo patrón E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | mismo patrón E0716 |

### Solución exacta (responsabilidad del QA)

En las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Acción

- NO se avanza a In Review
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716
