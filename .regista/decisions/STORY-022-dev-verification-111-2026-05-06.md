# STORY-022: Dev Verification #111 — 2026-05-06

## Resumen

Centésima decimoprimera verificación de STORY-022. El código de producción está completo y correcto. Los tests del QA tienen 3 errores de compilación E0716 que impiden su ejecución.

## Verificaciones del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | ✅ OK, sin errores |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Código de producción: estado completo y correcto

| CA | Estado | Detalle |
|----|--------|---------|
| CA1 | ✅ | `invoke_with_retry()` (L84) acepta `verbose: bool` como último parámetro |
| CA1 | ✅ | `invoke_with_retry_blocking()` (L199) propaga `verbose: bool` |
| CA2 | ✅ | `invoke_once()` (L316): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` |
| CA3 | ✅ | `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async |
| CA4 | ✅ | Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` |
| CA5 | ✅ | Stdout acumulado como `Vec<u8>` devuelto en `Output` |
| CA6 | ✅ | Stderr leído en `tokio::spawn` separado, sin streaming |
| CA7 | ✅ | `verbose=false` usa `wait_with_output()` (comportamiento original) |
| CA8 | ✅ | Timeout funciona en ambos modos (mata proceso por PID) |
| CA10 | ✅ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en los tests del QA (NO corregidos — responsabilidad del QA)

Los 3 errores son de tipo E0716 (`temporary value dropped while borrowed`) con el mismo patrón:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                         ^^^^^^^^^^^^^^^^^^^^^^
// temporary MutexGuard<Vec<u8>> dropped while Cow<str> still borrows it
```

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | E0716: `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | E0716: mismo patrón |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | E0716: mismo patrón |

### Solución exacta (debe aplicar el QA)

En las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- ✅ Código de producción completo y correcto (CA1-CA8, CA10-CA11 verificados).
- ❌ CA9 bloqueado: los tests del QA no compilan.
- ❌ NO se avanza a In Review.
- 🔄 El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
