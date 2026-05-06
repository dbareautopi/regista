# STORY-022 — Dev — 2026-05-05 (verificación #7)

## Resultado

❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716).

## Verificaciones del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` | ✅ OK (sin errores) |
| `cargo build` | ✅ OK (binario generado) |
| `cargo clippy --no-deps` | ✅ OK (0 warnings) |
| `cargo fmt -- --check` | ✅ OK (formateado correctamente) |
| `cargo test -- story022` | ❌ NO compila (3× E0716) |

## Implementación de producción (completa)

La implementación cubre CA1-CA8, CA10-CA11:

- **CA1**: `invoke_with_retry()` y `invoke_with_retry_blocking()` aceptan `verbose: bool` como último argumento.
- **CA2**: Cuando `verbose=true`, `invoke_once()` llama a `invoke_once_verbose()` que usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
- **CA3**: Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)`.
- **CA4**: stdout se acumula en `Vec<u8>` y se devuelve como parte del `Output`.
- **CA5**: stderr se lee en `tokio::spawn` separado con `read_to_end()`, sin streaming al log.
- **CA6**: Cuando `verbose=false`, se usa `wait_with_output()` (comportamiento actual).
- **CA7**: Timeout funciona en ambos modos vía `kill_process_by_pid()` (helper extraído, cross-platform).
- **CA8**: `cargo check` compila todo el crate sin errores.
- **CA10**: Call sites actualizados: `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.

## Errores en tests del QA

Los siguientes 3 tests no compilan por E0716 (`temporary value dropped while borrowed`):

### 1. `ca3_verbose_logs_lines_with_pipe_prefix` (línea ~1763)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, pero `Cow<str>` devuelto por `from_utf8_lossy` aún lo referencia.

### 2. `ca3_empty_lines_not_logged` (línea ~1809)

Mismo patrón:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

### 3. `ca5_stderr_not_streamed_to_log` (línea ~2006)

Mismo patrón:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

### Solución requerida (responsabilidad del QA)

En las 3 ubicaciones, reemplazar:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza a In Review. El código de producción está completo y correcto. Los tests del QA contienen errores de compilación que deben ser corregidos por el QA. El orquestador debe pasar el turno al QA para que aplique las 3 correcciones E0716.
