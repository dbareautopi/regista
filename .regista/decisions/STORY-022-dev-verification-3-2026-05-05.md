# STORY-022 — Dev Verification #3 — 2026-05-05

## Resumen

Tercera verificación de la implementación de STORY-022. El código de producción ya estaba implementado por intentos Dev anteriores. Esta sesión verifica que todo sigue en orden y documenta el estado actual.

## Verificación del código de producción

### Compilación
- `cargo check`: ✅ OK (sin errores)
- `cargo build`: ✅ OK (sin errores)
- `cargo clippy --no-deps`: ✅ OK (0 warnings)
- `cargo fmt -- --check`: ✅ OK (sin diferencias)

### Detalle de la implementación

| Componente | Línea | Descripción |
|---|---|---|
| `Cargo.toml` | — | Feature `io-util` añadido a tokio |
| `invoke_with_retry()` | L78 | `verbose: bool` como último parámetro |
| `invoke_with_retry_blocking()` | L193 | `verbose: bool` propagado |
| `invoke_once()` | L311 | `verbose: bool`. `false` → `wait_with_output()`. `true` → `invoke_once_verbose()` |
| `invoke_once_verbose()` | L358 | `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async |
| Streaming per-line | L390 | `tracing::info!("  │ {}", trimmed)` para cada línea no vacía |
| stdout accumulation | L384 | `accumulated.extend_from_slice(line.as_bytes())` |
| stderr task | L400 | `tokio::spawn` con `read_to_end()`, sin streaming |
| `kill_process_by_pid()` | L440 | Helper cross-platform para timeout |
| `plan.rs` call site | L152 | Pasa `false` |
| `pipeline.rs` call site | L774 | Pasa `false` |

### CAs cubiertos por el código de producción

| CA | Descripción | Estado |
|---|---|---|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ |
| CA2 | `verbose=true` → `BufReader::new()` + `read_line()` | ✅ |
| CA3 | `tracing::info!("  │ {}", trimmed)` por línea no vacía | ✅ |
| CA4 | stdout completo acumulado en `Vec<u8>` | ✅ |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ |
| CA7 | Timeout funciona en ambos modos | ✅ |
| CA8 | `cargo check` compila | ✅ |
| CA10 | Call sites actualizados | ✅ |
| CA11 | `AgentResult` contiene `stdout`, `stderr`, `exit_code` | ✅ |

## Tests del QA: errores E0716

Los tests en `mod story022` (dentro de `src/infra/agent.rs`) fallan al compilar con 3 errores `E0716` (temporary value dropped while borrowed).

### Error 1 — Línea ~1764
**Test**: `ca3_verbose_logs_lines_with_pipe_prefix`
**Código problemático**:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
**Problema**: `buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal que se destruye al final del statement. `String::from_utf8_lossy()` devuelve un `Cow<str>` que referencia el `MutexGuard` ya destruido.

### Error 2 — Línea ~1809
**Test**: `ca3_empty_lines_not_logged`
**Código problemático**: Idéntico al Error 1.

### Error 3 — Línea ~2006
**Test**: `ca5_stderr_not_streamed_to_log`
**Código problemático**: Idéntico al Error 1.

### Solución requerida (responsabilidad del QA)
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

El `binding` vive hasta el final del scope, por lo que `log_output` (el `Cow<str>`) puede referenciarlo sin problemas de lifetime.

## Decisión

**No se avanza a In Review.** Los tests del QA no compilan. Es responsabilidad del QA corregir los 3 errores `E0716` descritos arriba. El código de producción está completo y verificado.
