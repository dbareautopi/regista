# STORY-022 — Sexta verificación del Developer — 2026-05-05

## Resumen

Verificación completa del código de producción y los tests del QA (`mod story022`).

## Estado del código de producción

El código de producción implementa completamente todos los CAs de STORY-022:

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ L78 |
| CA2 | `verbose=true` usa `BufReader::new()` + `read_line()` | ✅ `invoke_once_verbose()` L358 |
| CA3 | Líneas logueadas con prefijo `  │ ` | ✅ L392: `tracing::info!("  │ {}", trimmed)` |
| CA4 | stdout acumulado en `Vec<u8>` y devuelto | ✅ L379: `accumulated.extend_from_slice()` |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ L399-404 |
| CA6 | `verbose=false` usa `wait_with_output()` | ✅ `invoke_once()` L341 |
| CA7 | Timeout funciona en ambos modos | ✅ `kill_process_by_pid()` L440 |
| CA8 | `cargo check --lib` compila | ✅ OK |
| CA10 | Call sites actualizados | ✅ `plan.rs:152`, `pipeline.rs:774` pasan `false` |
| CA11 | `AgentResult` tiene stdout/stderr/exit_code | ✅ struct sin cambios |

### Verificaciones ejecutadas

- `cargo check`: ✅ OK (sin errores)
- `cargo build`: ✅ OK (sin errores)
- `cargo clippy --no-deps`: ✅ OK (0 warnings)
- `cargo fmt -- --check`: ✅ OK (formateado correctamente)
- `Cargo.toml`: ✅ feature `io-util` presente en tokio

### Funciones implementadas

- `invoke_with_retry()` (L78): `verbose: bool` como último parámetro, propagado a `invoke_once()`
- `invoke_with_retry_blocking()` (L193): `verbose: bool` propagado a `invoke_with_retry()`
- `invoke_once()` (L311): dispatch según `verbose` → `invoke_once_verbose()` o `wait_with_output()`
- `invoke_once_verbose()` (L358): `BufReader::new()` + `read_line()` + `tracing::info!("  │ {}", trimmed)`
- `kill_process_by_pid()` (L440): helper cross-platform para timeout

### Call sites

- `src/app/plan.rs:152`: `invoke_with_retry_blocking(... , false)`
- `src/app/pipeline.rs:774`: `invoke_with_retry(... , false).await`

---

## Estado de los tests del QA (`mod story022`)

Los tests NO compilan. **3 errores E0716** (`temporary value dropped while borrowed`):

### Error 1 — L1763: test `ca3_verbose_logs_lines_with_pipe_prefix`
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                        ^^^^^^^^^^^^^^^^^^^^^^
//                                        MutexGuard temporal se destruye,
//                                        pero el Cow<str> lo referencia
```

### Error 2 — L1809: test `ca3_empty_lines_not_logged`
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                        ^^^^^^^^^^^^^^^^^^^^^^
//                                        Mismo patrón E0716
```

### Error 3 — L2006: test `ca5_stderr_not_streamed_to_log`
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                        ^^^^^^^^^^^^^^^^^^^^^^
//                                        Mismo patrón E0716
```

### Solución exacta (responsabilidad del QA)

Reemplazar en cada una de las 3 ubicaciones:

```rust
// ANTES (no compila):
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// DESPUÉS (compila):
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

---

## Decisión

- **NO se avanza a In Review**: los tests del QA no compilan (3× E0716).
- **NO se modifican los tests**: es responsabilidad del QA corregir los errores de borrow-checker en su propio código de test.
- **CA9 bloqueado**: `cargo test --lib infra::agent` no puede pasar si los tests de `mod story022` no compilan.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
