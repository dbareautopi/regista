# STORY-022 — Octogésima séptima verificación (Dev) — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (errores E0716).

## Estado del código de producción

El código de producción está **completo y correcto** (CA1-CA8, CA10-CA11):

### CA1: `invoke_with_retry()` acepta `verbose: bool` ✅
- Línea 84: `pub async fn invoke_with_retry(..., verbose: bool)`
- Línea 199 (`invoke_with_retry_blocking`): también acepta y propaga `verbose: bool`

### CA2: Modo verbose con `BufReader` + `read_line()` ✅
- `invoke_once()` (L316): delega a `invoke_once_verbose()` cuando `verbose=true`
- `invoke_once_verbose()` (L358): usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async

### CA3: Líneas logueadas con prefijo `  │ ` ✅
- `invoke_once_verbose()`: `tracing::info!("  │ {}", trimmed)` para líneas no vacías

### CA4: stdout acumulado en `Vec<u8>` ✅
- `invoke_once_verbose()`: `accumulated.extend_from_slice(line.as_bytes())` en el bucle

### CA5: stderr en `tokio::spawn` sin streaming ✅
- `invoke_once_verbose()`: `tokio::spawn` separado que usa `reader.read_to_end()`, sin `tracing::info!`

### CA6: Modo no-verbose usa `wait_with_output()` ✅
- `invoke_once()` (L316): `verbose=false` → `child.wait_with_output()`

### CA7: Timeout en ambos modos ✅
- `kill_process_by_pid()` (L440): mata proceso por PID cross-platform
- Timeout vía `tokio::time::timeout` en ambos caminos

### CA8: `cargo check --bin regista` compila ✅
- 0.20s, sin errores

### CA10: Call sites actualizados ✅
- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await`
- Tests pre-existentes: todos pasan `false`

### CA11: `AgentResult` intacto ✅
- Contiene `stdout: String`, `stderr: String`, `exit_code: i32`, `elapsed`, `attempt`, `attempts`

## Verificaciones adicionales
- `cargo clippy --no-deps --bin regista`: OK, 0 warnings
- `cargo fmt -- --check`: OK, código formateado
- `cargo test --test architecture`: OK, 11/11 pasan

## Errores en tests del QA (NO corregidos — 87ª iteración)

Los siguientes 3 tests en `mod story022` tienen error de compilación E0716:

### `ca3_verbose_logs_lines_with_pipe_prefix` (línea 1763)
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                         ^^^^^^^^^^^^^^^^^^^^^^
//                                         MutexGuard temporal destruido antes que Cow<str>
```

### `ca3_empty_lines_not_logged` (línea 1809)
Mismo patrón: `&buffer.lock().unwrap()` es temporal, `Cow<str>` sobrevive al `MutexGuard`.

### `ca5_stderr_not_streamed_to_log` (línea 2006)
Mismo patrón E0716.

### Solución requerida (responsabilidad del QA)
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## CA9: `cargo test -- story022` 
**BLOQUEADO** — no puede verificarse hasta que el QA corrija los 3 errores de compilación E0716.

## Decisión
NO se avanza el estado a In Review. El orquestador debe pasar el turno al QA para que corrija los errores E0716 en sus tests.
