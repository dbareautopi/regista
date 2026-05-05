# STORY-022 — Dev Verification #28 — 2026-05-05

## Resultado
❌ **NO se avanza a In Review** — Tests del QA no compilan (3 errores E0716).

## Verificación del código de producción

| Chequeo | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK, sin errores |
| `cargo build` | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Código de producción implementado (CA1-CA8, CA10-CA11)

- **CA1**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (L78).
  `invoke_with_retry_blocking()` también lo acepta y propaga (L193).
- **CA2**: `invoke_once()` tiene parámetro `verbose: bool` (L316). `verbose=true` →
  `invoke_once_verbose()` que usa `child.stdout.take()` + `BufReader::new()` +
  `read_line()` en bucle async (L358).
- **CA3**: Cada línea no vacía de stdout se loguea con `tracing::info!("  │ {}", trimmed)`.
- **CA4**: stdout se acumula en `Vec<u8>` y se devuelve como parte del `Output`.
- **CA5**: stderr se lee en `tokio::spawn` separado con `read_to_end()`, sin streaming.
- **CA6**: `verbose=false` → `wait_with_output()` (comportamiento original, sin cambios).
- **CA7**: `kill_process_by_pid()` (L440) maneja timeout cross-platform en ambos modos.
- **CA8**: `cargo check` compila todo el crate sin errores.
- **CA10**: Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.
- `Cargo.toml`: feature `io-util` añadido a tokio (CA2).

## Errores en los tests del QA (NO corregidos — responsabilidad del QA)

Los mismos 3 errores E0716 que persisten desde hace 27 iteraciones:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

**Solución requerida (responsabilidad del QA)**:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

en las 3 ubicaciones (líneas 1763, 1809, 2006).

## Acción
El orquestador debe pasar el turno al QA para que corrija los 3 errores de compilación.
