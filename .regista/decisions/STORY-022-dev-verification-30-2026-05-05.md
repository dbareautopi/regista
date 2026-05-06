# STORY-022 — Dev Verification #30 — 2026-05-05

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Verificaciones realizadas

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK |
| `cargo build` | ✅ OK |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

## Código de producción — estado

El código de producción está **completo y correcto**, cubriendo CA1-CA8, CA10-CA11:

- `invoke_with_retry()` (L84): `verbose: bool` como último parámetro (CA1).
- `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado (CA1, CA10).
- `invoke_once()` (L316): `verbose: bool`. `verbose=false` → `wait_with_output()`. `verbose=true` → `invoke_once_verbose()` (CA2, CA6).
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async. `tracing::info!("  │ {}", trimmed)` para líneas no vacías. stdout acumulado en `Vec<u8>`. stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming (CA2, CA3, CA4, CA5).
- `kill_process_by_pid()` (L440): helper para timeout cross-platform (CA7).
- Call sites (`app/plan.rs:157`, `app/pipeline.rs:780`) pasan `false` (CA10).
- `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11).
- `Cargo.toml`: feature `io-util` añadido a tokio (CA2).

## Errores en tests del QA (NO corregidos)

Los 3 errores E0716 persisten sin cambios desde la primera iteración:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

**Causa**: `buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal que se destruye al final del statement, pero `String::from_utf8_lossy()` devuelve un `Cow<str>` que lo referencia. El compilador detecta que el `Cow<str>` (`log_output`) sobrevive al `MutexGuard`.

**Solución exacta** (responsabilidad del QA):

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Reemplazar cada una de las 3 líneas por las 2 líneas anteriores.

## Decisión

- **NO** se corrigen los tests del QA (responsabilidad del QA según instrucciones del proyecto).
- **NO** se avanza la historia a In Review.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
- Esta es la 30ª iteración en la que el Dev rechaza avanzar por los mismos 3 errores.
