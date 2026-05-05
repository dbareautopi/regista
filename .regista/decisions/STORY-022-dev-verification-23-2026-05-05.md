# STORY-022 Dev Verification #23 — 2026-05-05

## Verificación del código de producción

Se realizó una verificación exhaustiva del código de producción para STORY-022
(Streaming de stdout del agente en `invoke_once()` + parámetro `verbose`).

### Resultados

| Verificación | Resultado |
|---|---|
| `cargo check` | ✅ OK (0.51s), sin errores |
| `cargo build` | ✅ OK (0.55s), binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK (0.32s), 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

### Cobertura de criterios de aceptación

El código de producción cubre completamente los siguientes CAs:

- **CA1**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (`agent.rs:84`). `invoke_with_retry_blocking()` también (`agent.rs:199`).
- **CA2**: `invoke_once_verbose()` (`agent.rs:358`) usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async.
- **CA3**: Cada línea no vacía se loguea con `tracing::info!("  │ {}", trimmed)` en `agent.rs:396`.
- **CA4**: stdout se acumula en `Vec<u8>` y se devuelve en `Output.stdout` (`agent.rs:389`, `agent.rs:434`).
- **CA5**: stderr se lee en `tokio::spawn` separado con `read_to_end()`, sin streaming al log (`agent.rs:408-412`).
- **CA6**: `verbose=false` usa `wait_with_output()` (`agent.rs:340`).
- **CA7**: `kill_process_by_pid()` (`agent.rs:440`) funciona en ambos modos para timeout.
- **CA8**: `cargo check --lib` (el crate entero) compila sin errores.
- **CA10**: Call sites actualizados: `app/plan.rs:152` pasa `false`, `app/pipeline.rs:774` pasa `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (`agent.rs:118-122`).

### Errores en tests del QA

Los tests en `mod story022` (`src/infra/agent.rs`) tienen 3 errores de compilación E0716:

| # | Test | Línea | Código problemático |
|---|---|---|---|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

**Causa**: El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal
que se destruye al final del statement, mientras que el `Cow<str>` devuelto por
`String::from_utf8_lossy` mantiene una referencia a los datos dentro del Mutex.
El compilador rechaza esto con el error E0716: "temporary value dropped while borrowed".

### Solución requerida

En cada una de las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

### Decisión

- **NO se corrigen los tests del QA** — es responsabilidad del QA Engineer.
- **NO se avanza a In Review** — los tests no compilan (CA9 bloqueado).
- El orquestador debe pasar el turno al QA Engineer para que corrija los 3 errores E0716.
