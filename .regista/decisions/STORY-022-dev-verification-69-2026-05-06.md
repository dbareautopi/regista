# STORY-022 — Dev — 69ª Verificación (2026-05-06)

## Resultado
❌ Tests no compilan — bloqueado por errores E0716 del QA

## Resumen

La implementación de producción está **completa y correcta**, cubriendo todos los criterios
de aceptación (CA1-CA8, CA10-CA11). Sin embargo, el módulo de tests `story022` contiene
3 errores de compilación E0716 que impiden ejecutar `cargo test`.

## Código de producción verificado

| Componente | Línea | Descripción |
|---|---|---|
| `invoke_with_retry()` | L78 | Acepta `verbose: bool` como último parámetro (CA1) |
| `invoke_with_retry_blocking()` | L199 | Propaga `verbose: bool` (CA1, CA10) |
| `invoke_once()` | L316 | `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6) |
| `invoke_once_verbose()` | L358 | `BufReader` + `read_line()`, `tracing::info!("  │ {}", trimmed)`, `Vec<u8>` acumulado, stderr en `tokio::spawn` (CA2-CA5) |
| `kill_process_by_pid()` | L440 | Helper cross-platform para timeout (CA7) |
| Call site `plan.rs` | L152 | Pasa `false` (CA10) |
| Call site `pipeline.rs` | L774 | Pasa `false` (CA10) |
| `AgentResult` | L30 | Mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11) |
| `Cargo.toml` | L25 | Feature `io-util` en tokio (CA2) |

## Verificaciones automáticas

- `cargo check`: ✅ OK (0.15s)
- `cargo clippy --no-deps --bin regista`: ✅ OK, 0 warnings (0.28s)
- `cargo fmt -- --check`: ✅ OK
- `cargo test --test architecture`: ✅ OK, 11/11 pasan (0.25s)
- `cargo test -- story022`: ❌ NO compila — 3 errores E0716

## Errores en tests del QA

Los 3 errores son **idénticos en naturaleza**: `MutexGuard` temporal destruido
antes que el `Cow<str>` devuelto por `String::from_utf8_lossy()`.

### Error 1 — `ca3_verbose_logs_lines_with_pipe_prefix` (línea 1763)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                         ^^^^^^^^^^^^^^^^^^^^^^
//                                         MutexGuard temporal se destruye aquí
// ...log_output se usa más abajo...
```

### Error 2 — `ca3_empty_lines_not_logged` (línea 1809)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//  Mismo error E0716
```

### Error 3 — `ca5_stderr_not_streamed_to_log` (línea 2006)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//  Mismo error E0716
```

### Solución (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
// ❌ Incorrecto (el MutexGuard se destruye antes de usar log_output)
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ Correcto (el MutexGuard vive mientras log_output se usa)
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza el estado de Tests Ready → In Review. El orquestador debe pasar
el turno al QA para que corrija los 3 errores de compilación E0716 en sus tests.

La implementación de producción es correcta y no requiere cambios.
