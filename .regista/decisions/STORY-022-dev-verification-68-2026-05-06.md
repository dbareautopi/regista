# STORY-022 — Dev Verification #68 (2026-05-06)

## Resumen

Sexagésima octava verificación de STORY-022. El código de producción está completo y correcto desde la iteración 49, cubriendo CA1-CA8 y CA10-CA11. Los tests del QA siguen teniendo 3 errores E0716.

## Verificaciones realizadas

| Check | Resultado |
|-------|-----------|
| `cargo check` | OK (0.33s) |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings (0.30s) |
| `cargo fmt -- --check` | OK, código formateado |
| `cargo test --test architecture` | OK, 11/11 pasan (0.22s) |
| `cargo test -- story022` | NO compila: 3 errores E0716 |

## Código de producción verificado

### `invoke_with_retry()` (L78)
- Parámetro `verbose: bool` como último argumento (CA1 ✓)
- Propagado a `invoke_once()` en el bucle de reintentos

### `invoke_with_retry_blocking()` (L193)
- Parámetro `verbose: bool` propagado a la versión async (CA1 ✓, CA10 ✓)

### `invoke_once()` (L316)
- Parámetro `verbose: bool`
- `verbose=false` → `wait_with_output()` (CA6 ✓)
- `verbose=true` → `invoke_once_verbose()` (CA2 ✓)

### `invoke_once_verbose()` (L358)
- `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async (CA2 ✓)
- `tracing::info!("  │ {}", trimmed)` por línea no vacía (CA3 ✓)
- stdout acumulado en `Vec<u8>` (CA4 ✓)
- stderr en `tokio::spawn` separado, sin streaming (CA5 ✓)

### `kill_process_by_pid()` (L440)
- Helper cross-platform para timeout en ambos modos (CA7 ✓)

### Call sites
- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)` (CA10 ✓)
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await` (CA10 ✓)
- Tests pre-existentes en `mod tests`: todos pasan `false` (CA10 ✓)

### `AgentResult`
- `stdout: String`, `stderr: String`, `exit_code: i32` (CA11 ✓)

### `Cargo.toml`
- Feature `io-util` presente en tokio (CA2 ✓)

## Errores E0716 en tests del QA (NO corregidos)

Los 3 errores de compilación persisten en `mod story022`:

| Test | Línea | Error |
|------|-------|-------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| `ca3_empty_lines_not_logged` | 1809 | Mismo error E0716 |
| `ca5_stderr_not_streamed_to_log` | 2006 | Mismo error E0716 |

### Causa raíz

`String::from_utf8_lossy(&[u8])` devuelve `Cow<'_, str>`, que toma prestado del slice de entrada si es UTF-8 válido. Al pasar `&buffer.lock().unwrap()`, el `MutexGuard<Vec<u8>>` es un temporal que se destruye al final del statement, dejando el `Cow<str>` con una referencia colgante.

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Estado

- **CA9 bloqueado**: `cargo test -- story022` no puede verificarse hasta que el QA corrija los 3 errores de compilación.
- **NO se avanza a In Review**. El orquestador debe pasar el turno al QA.
