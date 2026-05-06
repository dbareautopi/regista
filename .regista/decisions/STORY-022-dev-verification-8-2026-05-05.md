# STORY-022 — Dev Verification #8 — 2026-05-05

## Resultado

❌ **No se avanza a In Review** — los tests del QA (`mod story022`) siguen sin compilar.

---

## Verificación del código de producción

### Compilación y linting

| Check | Resultado | Detalle |
|-------|-----------|---------|
| `cargo build` | ✅ OK | 0.15s, binario generado |
| `cargo clippy --no-deps` | ✅ OK | 0 warnings, 0.23s |
| `cargo fmt -- --check` | ✅ OK | Código formateado correctamente |

### CAs cubiertos por producción

| CA | Estado | Evidencia |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry(..., verbose: bool)` |
| CA2 | ✅ | `invoke_once_verbose()`: `BufReader::new()` + `read_line()` en bucle |
| CA3 | ✅ | `tracing::info!("  │ {}", trimmed)` para líneas no vacías |
| CA4 | ✅ | stdout acumulado en `Vec<u8>`, devuelto en `Output.stdout` |
| CA5 | ✅ | stderr en `tokio::spawn` separado con `read_to_end()` |
| CA6 | ✅ | Rama `verbose=false` → `wait_with_output()` |
| CA7 | ✅ | `kill_process_by_pid()` en timeout, ambos modos |
| CA8 | ✅ | `cargo check` / `cargo build` compilan |
| CA10 | ✅ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` tiene `stdout: String`, `stderr: String`, `exit_code: i32` |

### CA9 — Bloqueado

`cargo test` no puede ejecutarse porque los tests del QA no compilan.

---

## Errores en tests del QA

Los 3 errores E0716 (`temporary value dropped while borrowed`) en `mod story022`:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

**Causa raíz**: `buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal.
`String::from_utf8_lossy()` toma `&[u8]` prestado del `MutexGuard`.
El `MutexGuard` se destruye al final del statement, pero el `Cow<str>` devuelto
por `from_utf8_lossy` aún lo referencia → E0716.

**Solución exacta** (responsabilidad del QA):

```rust
// ❌ Incorrecto actual:
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ Corrección requerida:
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

---

## Decisión

No se corrigen los tests del QA (protocolo Dev: "NO los corrijas. Es trabajo del QA").

No se avanza el estado a In Review. El orquestador debe pasar el turno al QA
para que aplique las 3 correcciones E0716.

---

## Historial de verificaciones

Esta es la **8ª verificación consecutiva** del Dev reportando los mismos 3 errores.
Las 7 verificaciones anteriores (documentadas en `STORY-022-dev-verification-{1..7}-2026-05-05.md`)
reportaron exactamente los mismos errores con la misma solución.
