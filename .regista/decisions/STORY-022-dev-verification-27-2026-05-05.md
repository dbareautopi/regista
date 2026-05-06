# STORY-022 — Dev Verification #27

**Fecha**: 2026-05-05
**Rol**: Developer

## Resumen

Vigesimoséptima verificación del código de producción de STORY-022. La implementación de producción sigue siendo completa y correcta, pero los tests del QA en `mod story022` siguen sin compilar (3 errores E0716).

## Resultados de verificación

| Check | Resultado |
|-------|-----------|
| `cargo build` | ✅ OK (0.32s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pass |
| `cargo test -- story022` | ❌ 3 errores E0716 |

## Errores en tests del QA

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — E0716: `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

Las 3 líneas usan el mismo patrón incorrecto donde el `MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement, mientras el `Cow<str>` devuelto por `String::from_utf8_lossy` aún lo referencia.

## Solución requerida (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
// ❌ Incorrecto (E0716)
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ Correcto
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Estado de la implementación de producción

Código completo y correcto en `src/infra/agent.rs`:

- `invoke_with_retry()` (L78): parámetro `verbose: bool` (CA1)
- `invoke_with_retry_blocking()` (L193): propaga `verbose` (CA1, CA10)
- `invoke_once()` (L316): despacha a `wait_with_output()` o `invoke_once_verbose()` (CA2, CA6)
- `invoke_once_verbose()` (L358): streaming línea a línea con `BufReader`, `tracing::info!("  │ {}", trimmed)`, stderr en `tokio::spawn` separado (CA2-CA5)
- `kill_process_by_pid()` (L440): timeout cross-platform (CA7)
- Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10)
- `Cargo.toml`: feature `io-util` de tokio (CA2)

## Decisión

NO se avanza a In Review. Los tests del QA no compilan y corregirlos es responsabilidad del QA, no del Developer. El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
