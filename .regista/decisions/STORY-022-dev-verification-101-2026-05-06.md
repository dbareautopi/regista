# STORY-022 — Dev — 2026-05-06 (verificación #101)

## Resultado
❌ Fallo parcial — tests del QA no compilan

## Verificaciones del código de producción

| Check | Resultado | Tiempo |
|-------|-----------|--------|
| `cargo check --bin regista` | ✅ OK, sin errores | 0.28s |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings | 0.42s |
| `cargo fmt -- --check` | ✅ OK, formateado | — |
| `cargo test --test architecture` | ✅ 11/11 pasan | 0.37s |
| `cargo test -- story022` | ❌ NO compila | — |

## Código de producción — estado completo

### CA1: `verbose: bool` en `invoke_with_retry()`
- `invoke_with_retry()` (L84): último parámetro `verbose: bool`
- `invoke_with_retry_blocking()` (L199): propagado correctamente

### CA2-CA5: Modo verbose con BufReader
- `invoke_once()` (L316): dispatch `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async
- `tracing::info!("  │ {}", trimmed)` por línea no vacía (CA3)
- `Vec<u8>` acumulado y devuelto en `Output` (CA4)
- stderr en `tokio::spawn` separada sin streaming (CA5)

### CA6: Modo no-verbose
- `wait_with_output()` — comportamiento actual sin cambios

### CA7: Timeout
- `kill_process_by_pid()` (L440): cross-platform (kill -9 / taskkill)

### CA10: Call sites actualizados
- `app/plan.rs:152`: `false`
- `app/pipeline.rs:774`: `false`
- Tests pre-existentes: `false`

### CA11: AgentResult
- `stdout: String`, `stderr: String`, `exit_code: i32` — sin cambios

## Errores en tests del QA

Los tests del módulo `story022` tienen 3 errores de compilación E0716
("temporary value dropped while borrowed") en las siguientes ubicaciones:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa
`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal.
`String::from_utf8_lossy()` toma `&[u8]` prestado del `MutexGuard`.
El `MutexGuard` se destruye al final del statement, pero el `Cow<str>`
resultante sigue vivo y contiene una referencia al `Vec<u8>` interno.
→ E0716: temporary value dropped while borrowed.

### Solución (responsabilidad del QA)
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones.

## Acción del Dev
NO se corrige el código de tests (responsabilidad del QA).
NO se avanza a In Review — el orquestador debe pasar el turno al QA.
