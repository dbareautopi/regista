# STORY-022 — Developer Verification #43 — 2026-05-05

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Verificación del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` (0.13s) | ✅ OK, sin errores |
| `cargo build` (0.14s) | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` (0.24s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.26s) | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Implementación del código de producción

### CA1: `invoke_with_retry()` acepta `verbose: bool`
- `invoke_with_retry()` (L83): último parámetro `verbose: bool`
- `invoke_with_retry_blocking()` (L193): `verbose: bool` propagado

### CA2: `invoke_once_verbose()` con BufReader + read_line
- `invoke_once()` (L311): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async

### CA3: Cada línea no vacía logueada con `tracing::info!("  │ {}", trimmed)`
- Implementado en el bucle de lectura stdout dentro de `invoke_once_verbose()`

### CA4: stdout acumulado en Vec<u8>
- stdout_handle retorna el Vec<u8> acumulado; se usa en el Output

### CA5: stderr en tokio::spawn separado, sin streaming
- stderr_handle lee con `read_to_end()` en una tarea spawn separada; sin logueo

### CA6: verbose=false usa wait_with_output()
- Rama `else` de `invoke_once()` usa `child.wait_with_output()` con timeout

### CA7: Timeout cross-platform en ambos modos
- `kill_process_by_pid()` (L440): helper extraído con soporte unix/windows
- Usado en ambas ramas (verbose y no-verbose) cuando el timeout se agota

### CA8: `cargo check --lib` compila
- ✅ Verificado

### CA10: Call sites actualizados con verbose
- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await`

### CA11: AgentResult mantiene stdout, stderr, exit_code
- `AgentResult.stdout: String`, `AgentResult.stderr: String`, `AgentResult.exit_code: i32`

## Errores en los tests del QA (NO corregidos)

Los mismos 3 tests llevan fallando desde la iteración #1 (43 iteraciones sin corrección):

| Test | Línea | Error E0716 |
|---|---|---|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `MutexGuard` temporal destruido antes que `Cow<str>` |
| `ca3_empty_lines_not_logged` | 1809 | Mismo error E0716 |
| `ca5_stderr_not_streamed_to_log` | 2006 | Mismo error E0716 |

### Causa raíz
`String::from_utf8_lossy` devuelve `Cow<str>` que toma prestado el argumento. Al pasar `&buffer.lock().unwrap()`, el `MutexGuard` es un temporal que se destruye al final del statement, pero el `Cow<str>` (asignado a `log_output`) aún lo referencia.

### Solución exacta (3 ubicaciones)
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

En lugar de:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

## Decisión
NO se avanza a In Review. El orquestador debe asignar el turno al QA para corregir los 3 errores de compilación. Si los tests no se corrigen, `cargo test -- story022` nunca pasará (CA9).
