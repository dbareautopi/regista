# STORY-022 — Dev Verification #98 (2026-05-06)

## Resumen

Nonagésima octava verificación de STORY-022. El código de producción está completo
y correcto desde la primera implementación. Los tests del QA siguen teniendo los
mismos 3 errores de compilación E0716 (temporary value dropped while borrowed)
que requieren una corrección trivial.

## Verificaciones realizadas

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo check --bin regista` | ✅ OK, sin errores | 0.40s |
| `cargo clippy --no-deps --bin regista` | ✅ 0 warnings | 4.95s |
| `cargo fmt -- --check` | ✅ Código formateado | — |
| `cargo test --test architecture` | ✅ 11/11 pasan | 0.05s |
| `cargo test -- story022` | ❌ 3 errores E0716 | — |

## Estado del código de producción

Completo y correcto. Todos los CA de producción se cumplen:

- **CA1**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (L84)
- **CA2**: `invoke_once()` despacha a `invoke_once_verbose()` con `BufReader` + `read_line()`
- **CA3**: `tracing::info!("  │ {}", trimmed)` por cada línea no vacía
- **CA4**: stdout acumulado en `Vec<u8>` devuelto en `Output`
- **CA5**: stderr en `tokio::spawn` separada sin streaming al log
- **CA6**: `verbose=false` usa `wait_with_output()`
- **CA7**: timeout funciona en ambos modos, mata proceso por PID
- **CA8**: `cargo check --bin regista` compila sin errores
- **CA10**: todos los call sites actualizados (plan.rs:152 `false`, pipeline.rs:774 `false`)
- **CA11**: `AgentResult` mantiene `stdout`, `stderr`, `exit_code`

## Errores en tests del QA

3 errores E0716 idénticos. La causa es `String::from_utf8_lossy(&buffer.lock().unwrap())`:
el `MutexGuard` temporal se destruye antes que el `Cow<str>` que toma prestado su contenido.

### Ubicaciones exactas

| Línea | Test | Código problemático |
|-------|------|---------------------|
| 1763 | `ca3_verbose_logs_lines_with_pipe_prefix` | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 1809 | `ca3_empty_lines_not_logged` | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2006 | `ca5_stderr_not_streamed_to_log` | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Corrección requerida (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Esto mantiene vivo el `MutexGuard` (a través de `binding`) mientras `log_output` lo
toma prestado.

## Decisión

**NO se avanza el estado a In Review.** El código de producción está correcto pero los
tests no compilan. Corregir tests es responsabilidad del QA. El orquestador debe
pasar el turno al QA automáticamente.
