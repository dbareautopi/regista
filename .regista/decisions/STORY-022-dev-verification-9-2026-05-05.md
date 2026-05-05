# STORY-022 — Dev Verification #9 — 2026-05-05

## Estado

El código de producción está completo y compila sin errores. Los tests del QA en `mod story022` tienen 3 errores E0716 que impiden la compilación. No se avanza a In Review.

## Verificación del código de producción

### Compilación y linting

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo check` | OK, sin errores | 0.16s |
| `cargo clippy --no-deps` | OK, 0 warnings | 0.28s |
| `cargo build` | OK, binario generado | — |
| `cargo fmt -- --check` | OK, formateado | — |

### Implementación de producción

| CA | Descripción | Estado | Ubicación |
|----|-------------|--------|-----------|
| CA1 | `verbose: bool` en `invoke_with_retry()` | ✅ | `agent.rs:78` |
| CA2 | `BufReader` + `read_line()` en modo verbose | ✅ | `agent.rs:358-415` (`invoke_once_verbose()`) |
| CA3 | `tracing::info!("  │ {}", trimmed)` | ✅ | `agent.rs:395` |
| CA4 | stdout acumulado en `Vec<u8>` | ✅ | `agent.rs:387-390` |
| CA5 | stderr en `tokio::spawn` sin streaming | ✅ | `agent.rs:418-424` |
| CA6 | `wait_with_output()` en modo no-verbose | ✅ | `agent.rs:345-347` |
| CA7 | Timeout funciona en ambos modos | ✅ | `kill_process_by_pid()` + `tokio::time::timeout` |
| CA8 | `cargo check --lib` compila | ✅ | Verificado |
| CA10 | Call sites actualizados con `verbose` | ✅ | `plan.rs:152` (`false`), `pipeline.rs:774` (`false`) |
| CA11 | `AgentResult` mantiene campos | ✅ | `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en tests del QA

Los 3 errores son del mismo tipo (E0716: temporary value dropped while borrowed):

### Error 1 — Línea 1763
- **Test**: `ca3_verbose_logs_lines_with_pipe_prefix`
- **Código**: `String::from_utf8_lossy(&buffer.lock().unwrap())`
- **Problema**: El `MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, mientras el `Cow<str>` retornado por `from_utf8_lossy` aún lo referencia.

### Error 2 — Línea 1809
- **Test**: `ca3_empty_lines_not_logged`
- **Código**: `String::from_utf8_lossy(&buffer.lock().unwrap())`
- **Problema**: Idéntico al Error 1.

### Error 3 — Línea 2006
- **Test**: `ca5_stderr_not_streamed_to_log`
- **Código**: `String::from_utf8_lossy(&buffer.lock().unwrap())`
- **Problema**: Idéntico al Error 1.

### Solución requerida (responsabilidad del QA)

En las 3 líneas, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza el estado a In Review. Los tests del QA no compilan. El orquestador debe detectar esta situación y pasar el turno al QA para que corrija los 3 errores E0716.

## CA9 bloqueado

`cargo test --lib infra::agent` no puede verificarse porque la suite completa no compila debido a los errores en `mod story022`.
