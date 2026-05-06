# STORY-022 — Dev Verification #51 — 2026-05-06

## Resumen

Quincuagésima primera verificación de STORY-022. El código de producción está completo
y correcto, cubriendo todos los criterios de aceptación (CA1-CA8, CA10-CA11). Los tests
del QA en `mod story022` siguen teniendo 3 errores de compilación E0716 que impiden
ejecutar `cargo test -- agent::`.

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check` (0.19s) | ✅ OK, sin errores |
| `cargo build` (0.21s) | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` (0.24s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.29s) | ✅ OK, 11/11 pasan |

## Cobertura de criterios de aceptación

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último argumento | ✅ Implementado (L84) |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` | ✅ Implementado (L334-335, L358) |
| CA3 | Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)` | ✅ Implementado (L389) |
| CA4 | stdout acumulado en `Vec<u8>` y devuelto | ✅ Implementado (L383, L430-432) |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ Implementado (L397-403) |
| CA6 | `verbose=false` → `wait_with_output()` (comportamiento actual) | ✅ Implementado (L337) |
| CA7 | Timeout mata proceso en ambos modos | ✅ Implementado (L337-353, L407-418) |
| CA8 | `cargo check --lib` compila | ✅ Verificado |
| CA9 | `cargo test -- agent::` pasa todos los tests | ❌ Bloqueado por errores del QA |
| CA10 | Call sites actualizados con `verbose` | ✅ `plan.rs:152` pasa `false`, `pipeline.rs:774` pasa `false` |
| CA11 | `AgentResult` con `stdout`, `stderr`, `exit_code` | ✅ Sin cambios |

### Dependencia adicional

- `Cargo.toml`: feature `io-util` añadido a tokio (requerido por `BufReader`) ✅

## Errores E0716 en tests del QA (NO corregidos — responsabilidad del QA, 51ª iteración)

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que el `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

### Solución exacta (responsabilidad del QA)

En las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- NO se avanza a In Review. Los tests del QA no compilan.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
- CA9 no puede verificarse hasta que el QA corrija los errores de compilación.
