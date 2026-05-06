# STORY-022 — dev — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA tienen errores de compilación E0716

## Verificación del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK, sin errores |
| `cargo build` | ✅ OK (0.33s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila |

## Código de producción implementado (CA1-CA8, CA10-CA11)

### `invoke_with_retry()` (L78)
- Nuevo parámetro `verbose: bool` como último argumento (CA1 ✅)
- Se propaga a `invoke_once()` en cada invocación

### `invoke_with_retry_blocking()` (L193)
- Nuevo parámetro `verbose: bool` propagado al wrapper async (CA1 ✅, CA10 ✅)

### `invoke_once()` (L316)
- Nuevo parámetro `verbose: bool`
- `verbose=false`: usa `wait_with_output()` (comportamiento actual, más eficiente) (CA6 ✅)
- `verbose=true`: delega en `invoke_once_verbose()` (CA2 ✅)

### `invoke_once_verbose()` (L358)
- `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async (CA2 ✅)
- Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` (CA3 ✅)
- stdout acumulado en `Vec<u8>` y devuelto como parte del resultado (CA4 ✅)
- stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming al log (CA5 ✅)

### `kill_process_by_pid()` (L440)
- Helper extraído para timeout cross-platform (Linux: `kill -9`, Windows: `taskkill /F`) (CA7 ✅)
- Usado por ambos modos (verbose y no-verbose)

### Call sites actualizados (CA10 ✅)
| Archivo | Línea | Valor |
|---------|-------|-------|
| `app/plan.rs` | 158 | `false` |
| `app/pipeline.rs` | 780 | `false` |
| Tests pre-existentes (L657, L686, L720, L864) | varios | `false` |

### `AgentResult` (CA11 ✅)
Mantiene `stdout: String`, `stderr: String`, `exit_code: i32`

### `Cargo.toml` (CA2 ✅)
Feature `io-util` añadido a tokio

## Errores en tests del QA (NO corregidos)

Los 3 tests con error E0716 usan el patrón:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` retornado por `buffer.lock().unwrap()` es un temporal que se destruye
al final del statement, pero `String::from_utf8_lossy()` retorna `Cow<str>` que toma
prestado el `MutexGuard`.

### Tests afectados

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | E0716: `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | E0716: `MutexGuard` temporal destruido antes que `Cow<str>` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | E0716: `MutexGuard` temporal destruido antes que `Cow<str>` |

### Solución requerida (responsabilidad del QA)

En las 3 ubicaciones (líneas 1763, 1809, 2006), reemplazar:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión
NO se avanza a In Review. CA9 bloqueado hasta que el QA corrija los 3 errores E0716.
El orquestador debe pasar el turno al QA para que corrija los tests.
