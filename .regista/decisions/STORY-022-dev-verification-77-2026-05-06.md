# STORY-022 — Dev — 20260506T000000

## Resultado
❌ Bloqueado por tests del QA (3 errores E0716)

## Resumen

Septuagésima séptima verificación de STORY-022. El código de producción está completo y correcto, cubriendo todos los criterios de aceptación (CA1-CA8, CA10-CA11). Sin embargo, **3 tests del QA no compilan** debido a un error E0716 (temporary value dropped while borrowed).

## Código de producción (completo y correcto)

| Componente | Ubicación | Descripción |
|-----------|-----------|-------------|
| `invoke_with_retry()` | `infra/agent.rs:84` | Acepta `verbose: bool` como último parámetro (CA1) |
| `invoke_with_retry_blocking()` | `infra/agent.rs:199` | Propaga `verbose: bool` (CA1, CA10) |
| `invoke_once()` | `infra/agent.rs:316` | `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6) |
| `invoke_once_verbose()` | `infra/agent.rs:358` | `BufReader` + `read_line()`, streaming con prefijo `  │ `, stderr en `tokio::spawn` (CA2-CA5) |
| `kill_process_by_pid()` | `infra/agent.rs:440` | Helper cross-platform para timeout (CA7) |
| Call site plan.rs | `app/plan.rs:152` | Pasa `false` (CA10) |
| Call site pipeline.rs | `app/pipeline.rs:774` | Pasa `false` (CA10) |
| `AgentResult` | `infra/agent.rs:30` | Mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11) |
| `Cargo.toml` | raíz | Feature `io-util` en tokio (CA2) |

## Verificaciones

- ✅ `cargo check` (0.16s): OK, sin errores
- ✅ `cargo clippy --no-deps --bin regista` (0.25s): OK, 0 warnings
- ✅ `cargo fmt -- --check`: OK, código formateado
- ✅ `cargo test --test architecture` (0.39s): OK, 11/11 pasan
- ✅ `cargo build` (0.41s): OK, binario generado
- ❌ `cargo test -- story022`: NO compila (3 errores E0716)

## Errores en tests del QA (NO corregidos)

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | Mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | Mismo error E0716 |

## Solución requerida (responsabilidad del QA)

En las 3 ubicaciones afectadas (líneas 1763, 1809, 2006), reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza a **In Review**. CA9 (`cargo test -- story022`) está bloqueado hasta que el QA corrija los 3 errores de compilación. El orquestador debe pasar el turno al QA automáticamente (transición Tests Ready → Tests Ready, fix).
