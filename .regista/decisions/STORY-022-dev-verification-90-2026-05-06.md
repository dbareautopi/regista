# STORY-022 — Developer — 20260506T000000

## Resultado
❌ Tests QA no compilan — NO se avanza a In Review

## Verificación del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` (0.17s) | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` (0.27s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.29s) | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila |

## Código de producción (completo y correcto)

CA1-CA8, CA10-CA11 están implementados:

- **CA1**: `invoke_with_retry()` (L84) acepta `verbose: bool` como último parámetro. `invoke_with_retry_blocking()` (L199) lo propaga.
- **CA2**: `invoke_once()` (L316) ramifica: `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()`.
- **CA3**: `invoke_once_verbose()` (L358) usa `BufReader` línea a línea, emite `tracing::info!("  │ {}", trimmed)` por cada línea no vacía.
- **CA4**: Stdout acumulado en `Vec<u8>`, devuelto en `Output`.
- **CA5**: Stderr leído en `tokio::spawn` sin streaming, acumulado en `Vec<u8>`.
- **CA6**: Modo no-verbose usa `wait_with_output()` (comportamiento actual).
- **CA7**: `kill_process_by_pid()` (L440) cross-platform para timeout en ambos modos.
- **CA8**: `cargo check --bin regista` OK.
- **CA10**: Call sites en `plan.rs:152` y `pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.

## Errores en los tests del QA (NO corregidos)

Los 3 tests fallan con E0716 (`temporary value dropped while borrowed`):

| Test | Línea | Error |
|------|-------|-------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| `ca3_empty_lines_not_logged` | 1809 | mismo error E0716 |
| `ca5_stderr_not_streamed_to_log` | 2006 | mismo error E0716 |

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Acción

NO se avanza a In Review. El orquestador debe pasar el turno al QA para que corrija los 3 errores de compilación en `mod story022`.
