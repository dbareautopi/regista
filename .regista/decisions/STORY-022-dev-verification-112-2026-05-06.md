# STORY-022 — Dev Verification #112 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716).

## Verificaciones realizadas

| Check | Resultado | Detalle |
|-------|-----------|---------|
| `cargo check --bin regista` | ✅ OK | 0.27s, sin errores |
| `cargo clippy --no-deps --bin regista` | ✅ OK | 0.21s, 0 warnings |
| `cargo fmt -- --check` | ✅ OK | Código formateado |
| `cargo test --test architecture` | ✅ OK | 11/11 pasan |
| `cargo test -- story022` | ❌ NO COMPILA | 3 errores E0716 |

## Código de producción — Estado

Completo y correcto. Todos los criterios de aceptación del Dev están satisfechos:

### CA1: `verbose: bool` en `invoke_with_retry()` y `invoke_with_retry_blocking()`
- `invoke_with_retry()` (L84): acepta `verbose: bool` como último parámetro.
- `invoke_with_retry_blocking()` (L199): propaga `verbose: bool` a `invoke_with_retry()`.

### CA2: Modo verbose con `BufReader` + `read_line()`
- `invoke_once()` (L290): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`.
- `invoke_once_verbose()` (L358): usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.

### CA3: Logging con prefijo `  │ `
- Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)`.
- Líneas vacías no generan entradas de log.

### CA4: stdout acumulado en `Vec<u8>`
- El `Vec<u8>` acumulado se devuelve como parte del `Output`.
- `AgentResult.stdout` contiene el `String` decodificado.

### CA5: stderr en `tokio::spawn` separada
- stderr se lee en una tarea `tokio::spawn` independiente.
- Sin streaming al log.
- Acumulado en `Vec<u8>` y devuelto en el `Output`.

### CA6: Modo no-verbose usa `wait_with_output()`
- Comportamiento actual preservado sin cambios.
- Ambos modos producen el mismo stdout para un mismo proceso.

### CA7: Timeout funciona en ambos modos
- `kill_process_by_pid()` (L440): helper cross-platform.
- Timeout en modo verbose mata el proceso vía PID.
- Timeout en modo no-verbose usa el mismo mecanismo.

### CA8: Compilación
- `cargo check --bin regista`: OK, sin errores.

### CA10: Call sites actualizados
- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`.
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await`.

### CA11: `AgentResult` preserva estructura
- `stdout: String`, `stderr: String`, `exit_code: i32`.
- Todos los campos públicamente accesibles.

## Errores en tests del QA (NO corregidos)

Los tests compilaban en iteraciones anteriores pero el refactor del QA introdujo 3 errores E0716 
(`temporary value dropped while borrowed`). El mismo patrón en 3 ubicaciones:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz
`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal.
`String::from_utf8_lossy()` toma `&[u8]` y devuelve `Cow<str>` que referencia el slice.
El `MutexGuard` se destruye al final del statement, invalidando la referencia dentro del `Cow`.

### Solución (responsabilidad del QA)
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```
Aplicar en las 3 ubicaciones (líneas 1763, 1809, 2006).

## CA9 bloqueado
`cargo test -- story022` no puede ejecutarse hasta que el QA corrija los 3 errores de compilación.

## Acción requerida
El orquestador debe aplicar la transición **Tests Ready → Tests Ready (QA fix)** 
para devolver el turno al QA Engineer.
