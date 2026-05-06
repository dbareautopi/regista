# STORY-022 — Dev Verification #4 — 2026-05-05

## Resultado
❌ No se avanza a In Review — los tests del QA no compilan (3 errores E0716)

## Verificación del código de producción

Se verificó que la implementación de producción está completa y correcta:

| Verificación | Resultado |
|---|---|
| `cargo check` | ✅ Sin errores |
| `cargo build` | ✅ Sin errores |
| `cargo clippy --no-deps` | ✅ 0 warnings |
| `cargo fmt -- --check` | ✅ Formateado correctamente |

### Resumen de la implementación

1. **`Cargo.toml`**: feature `io-util` añadido a tokio para `AsyncBufReadExt` y `AsyncReadExt`.

2. **`invoke_with_retry()`** (L78): acepta `verbose: bool` como último parámetro, propagado a `invoke_once()`.

3. **`invoke_with_retry_blocking()`** (L193): acepta `verbose: bool`, propagado a `invoke_with_retry()`.

4. **`invoke_once()`** (L316): nuevo parámetro `verbose: bool`.
   - `verbose = false` → `wait_with_output()` (comportamiento actual, eficiente).
   - `verbose = true` → `invoke_once_verbose()` (streaming línea a línea).

5. **`invoke_once_verbose()`** (L358): implementación completa del streaming:
   - `child.stdout.take()` + `BufReader::new()` + bucle `read_line()`.
   - Cada línea no vacía: `tracing::info!("  │ {}", trimmed)`.
   - stdout acumulado en `Vec<u8>` y devuelto en `Output.stdout`.
   - stderr leído en `tokio::spawn` separado con `read_to_end()`, sin streaming.
   - Timeout funciona correctamente vía `tokio::time::timeout` + `kill_process_by_pid()`.

6. **`kill_process_by_pid()`** (L440): helper cross-platform para matar procesos por PID.

7. **Call sites actualizados**:
   - `app/plan.rs:152`: pasa `false` (no necesita streaming en planificación).
   - `app/pipeline.rs:774`: pasa `false` (no necesita streaming en pipeline).

### CAs cubiertos por la implementación de producción

| CA | Estado | Notas |
|---|---|---|
| CA1 | ✅ | `invoke_with_retry()` acepta `verbose: bool` |
| CA2 | ✅ | `invoke_once_verbose()` usa `BufReader::new()` + `read_line()` |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` para líneas no vacías |
| CA4 | ✅ | stdout acumulado en `Vec<u8>`, devuelto en `Output` |
| CA5 | ✅ | stderr en `tokio::spawn`, sin streaming |
| CA6 | ✅ | `verbose=false` usa `wait_with_output()` |
| CA7 | ✅ | timeout funciona en ambos modos |
| CA8 | ✅ | `cargo check` compila sin errores |
| CA10 | ✅ | call sites actualizados con `verbose: false` |
| CA11 | ✅ | `AgentResult` conserva `stdout`, `stderr`, `exit_code` |

## Errores en los tests del QA

Los tests en `mod story022` de `src/infra/agent.rs` tienen **3 errores de compilación E0716** (temporary value dropped while borrowed):

### Error 1 — L1763: `ca3_verbose_logs_lines_with_pipe_prefix`
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, pero `Cow<str>` devuelto por `from_utf8_lossy` lo referencia. `log_output` se usa después en los `assert!`.

### Error 2 — L1809: `ca3_empty_lines_not_logged`
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
Mismo problema que el Error 1.

### Error 3 — L2006: `ca5_stderr_not_streamed_to_log`
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
Mismo problema que el Error 1.

### Solución requerida (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Esto garantiza que el `MutexGuard` (`binding`) viva al menos tanto como `log_output`.

## Decisión

**No se avanza a In Review.** La corrección de los 3 errores E0716 en los tests es responsabilidad del QA. El orquestador deberá pasar el turno al QA para que corrija los tests y vuelva a poner la historia en `Tests Ready`.

## CA9 pendiente

`cargo test --lib infra::agent` no puede ejecutarse porque el binario de tests no compila debido a los 3 errores E0716. Una vez que el QA los corrija, el Developer deberá re-ejecutar los tests y verificar que pasan.
