# STORY-022 — Dev Verification #44 — 2026-05-05

## Resultado
❌ No se avanza a In Review — los tests del QA tienen errores de compilación E0716.

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check` | ✅ OK, sin errores |
| `cargo build` | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Cobertura de criterios de aceptación (producción)

| CA | Descripción | Línea | Estado |
|----|-------------|-------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | L84 | ✅ |
| CA1 | `invoke_with_retry_blocking()` propaga `verbose` | L200 | ✅ |
| CA2 | `invoke_once()` usa `child.stdout.take()` + `BufReader` + `read_line()` en verbose | L316, L358 | ✅ |
| CA3 | Cada línea no vacía logueada con `tracing::info!("  │ {}", trimmed)` | L358-395 | ✅ |
| CA4 | stdout acumulado en `Vec<u8>` y devuelto en `Output` | L358-395 | ✅ |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | L358-395 | ✅ |
| CA6 | `verbose=false` usa `wait_with_output()` | L316-330 | ✅ |
| CA7 | Timeout funciona en ambos modos (kill_process_by_pid) | L330, L440 | ✅ |
| CA8 | `cargo check --lib` compila sin errores | — | ✅ |
| CA10 | Call sites en `plan.rs:152` y `pipeline.rs:774` pasan `false` | — | ✅ |
| CA11 | `AgentResult` tiene `stdout`, `stderr`, `exit_code` | — | ✅ |

## Errores en tests del QA

Los 3 tests con errores E0716 son:

### 1. `ca3_verbose_logs_lines_with_pipe_prefix` (línea 1763)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

**Error**: `MutexGuard` temporal destruido al final del statement, pero `Cow<str>` de `from_utf8_lossy` sobrevive.

### 2. `ca3_empty_lines_not_logged` (línea 1809)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

**Error**: mismo E0716.

### 3. `ca5_stderr_not_streamed_to_log` (línea 2006)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

**Error**: mismo E0716.

## Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Separar la adquisición del lock del `from_utf8_lossy` para que el `MutexGuard` viva lo suficiente.

## Acción

NO se avanza a In Review. El orquestador debe pasar el turno al QA para corregir los 3 errores E0716 en `mod story022`.
