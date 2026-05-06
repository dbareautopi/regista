# STORY-022 — Decisión Dev (verificación #46)

**Fecha**: 2026-05-06
**Actor**: Dev
**Decisión**: NO avanzar a In Review — los tests del QA no compilan.

---

## Resumen

Verificación #46 del código de producción para STORY-022. El código de producción
está completo y correcto (todas las verificaciones pasan), pero los tests del QA
siguen teniendo 3 errores de compilación E0716.

## Estado del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` | OK (0.32s) |
| `cargo build` | OK (0.24s), binario generado |
| `cargo clippy --no-deps --bin regista` | OK (0.28s), 0 warnings |
| `cargo fmt -- --check` | OK |
| `cargo test --test architecture` | OK, 11/11 pasan |

### Cobertura de CAs en producción

| CA | Estado | Evidencia |
|----|--------|-----------|
| CA1 | ✓ | `invoke_with_retry()` (L78) acepta `verbose: bool` como último parámetro |
| CA2 | ✓ | `invoke_once_verbose()` (L358) usa `child.stdout.take()` + `BufReader::new()` |
| CA3 | ✓ | `tracing::info!("  │ {}", trimmed)` en cada línea no vacía |
| CA4 | ✓ | stdout acumulado en `Vec<u8>` y devuelto en `Output` |
| CA5 | ✓ | stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming |
| CA6 | ✓ | `verbose=false` → `wait_with_output()` (sin cambios) |
| CA7 | ✓ | `kill_process_by_pid()` (L440) para timeout cross-platform |
| CA8 | ✓ | `cargo check` compila sin errores |
| CA9 | ✗ | Bloqueado por errores de compilación en tests del QA |
| CA10 | ✓ | Call sites en plan.rs:152 y pipeline.rs:774 pasan `false` |
| CA11 | ✓ | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` |

## Errores en tests del QA

Los mismos 3 errores E0716 que han persistido durante 46 iteraciones:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `MutexGuard` temporal destruido — `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | Mismo patrón E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | Mismo patrón E0716 |

### Solución exacta (trivial)

En las 3 ubicaciones (líneas 1763, 1809, 2006), reemplazar:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Es una corrección de 2 líneas por ubicación (6 líneas en total). El `MutexGuard`
temporal (`MutexGuard<'_, Vec<u8>>`) se destruye al final del statement, pero
`from_utf8_lossy` devuelve un `Cow<str>` que lo referencia. La variable `binding`
extiende la vida del guard hasta después del uso de `log_output`.

## Decisión

- **NO** se corrigen los tests (responsabilidad del QA)
- **NO** se avanza el estado a `In Review`
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716
