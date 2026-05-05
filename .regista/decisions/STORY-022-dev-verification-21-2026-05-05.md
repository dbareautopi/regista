# STORY-022 — Dev — Verificación #21 — 2026-05-05

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Verificación del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check` | OK (sin errores) |
| `cargo build` | OK (binario generado) |
| `cargo clippy --no-deps --bin regista` | OK (0 warnings) |
| `cargo fmt -- --check` | OK (formateado) |
| `cargo test --test architecture` | OK (11/11 pasan) |
| `cargo test -- story022` | **NO compila** (3 errores E0716) |

## Implementación de producción (completa y correcta)

La implementación cubre todos los CAs implementables (CA1-CA8, CA10-CA11):

- **CA1**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (L78). `invoke_with_retry_blocking()` también (L193).
- **CA2**: `invoke_once()` tiene parámetro `verbose: bool`. `verbose=true` → `invoke_once_verbose()` con `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
- **CA3**: Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)`.
- **CA4**: stdout acumulado en `Vec<u8>` y devuelto en `Output`.
- **CA5**: stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming al log.
- **CA6**: `verbose=false` → `wait_with_output()` (comportamiento actual, más eficiente).
- **CA7**: `kill_process_by_pid()` extraído para timeout cross-platform en ambos modos.
- **CA8**: `cargo check` compila sin errores.
- **CA10**: Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.
- **Dependencia**: feature `io-util` añadido a tokio en `Cargo.toml`.

## Errores en tests del QA

Los 3 errores E0716 son idénticos en naturaleza: `temporary value dropped while borrowed`.

### Error 1 — `ca3_verbose_logs_lines_with_pipe_prefix` (L1763)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se destruye
al final del statement, pero el `Cow<str>` devuelto por `String::from_utf8_lossy`
mantiene una referencia a los datos protegidos por el mutex.

### Error 2 — `ca3_empty_lines_not_logged` (L1809)

Mismo patrón que el error 1.

### Error 3 — `ca5_stderr_not_streamed_to_log` (L2006)

Mismo patrón que el error 1.

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

Esto extiende la vida del `MutexGuard` lo suficiente para que el `Cow<str>`
pueda usarse sin referencias colgantes.

## Decisión

**NO se avanza a In Review.** El código de producción está completo y correcto,
pero los tests del QA no compilan. Es responsabilidad del QA corregir los 3
errores E0716. El orquestador debe pasar el turno al QA automáticamente
(transición `Tests Ready → Tests Ready`).
