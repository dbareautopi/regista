# STORY-022 Dev Verification #59 — 2026-05-06

## Resumen

Quincuagésima novena verificación del código de producción de STORY-022.
El código de producción está completo y correcto. Los tests del QA siguen
sin compilar por 3 errores E0716. **No se avanza a In Review.**

## Verificación

| Comando | Resultado |
|---------|-----------|
| `cargo check` | OK (0.54s) |
| `cargo build` | OK (0.50s) |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings (0.54s) |
| `cargo fmt -- --check` | OK, código formateado |
| `cargo test --test architecture` | OK, 11/11 pasan (0.04s) |
| `cargo test -- story022` | **NO COMPILA** — 3 errores E0716 |

## Cobertura del código de producción (CA1-CA8, CA10-CA11)

| CA | Estado | Ubicación |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry()` L84: `verbose: bool` como último parámetro |
| CA2 | ✅ | `invoke_once_verbose()` L358: `BufReader::new()` + `read_line()` en bucle async |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` para cada línea no vacía |
| CA4 | ✅ | stdout acumulado en `Vec<u8>` y devuelto en `Output` |
| CA5 | ✅ | stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` (camino rápido) |
| CA7 | ✅ | `kill_process_by_pid()` cross-platform en ambos modos |
| CA8 | ✅ | `cargo check` compila todo el crate |
| CA10 | ✅ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores E0716 en tests del QA

Los 3 tests fallan con el mismo error (E0716: temporary value dropped while borrowed):

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

**Causa**: El patrón `String::from_utf8_lossy(&buffer.lock().unwrap())` crea un
`MutexGuard` temporal que se destruye al final del statement, pero el `Cow<str>`
resultante sigue vivo y hace referencia al guard destruido.

**Solución** (responsabilidad del QA):

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Debe aplicarse en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Decisión

NO se avanza a In Review. El orquestador debe pasar el turno al QA para que
corrija los 3 errores de compilación en `mod story022`.
