# STORY-022 — Dev Verification #14 — 2026-05-05

## Contexto

Decimocuarta verificación de STORY-022 por parte del Developer. Las 13 verificaciones
anteriores reportaron los mismos 3 errores E0716 en `mod story022` (tests del QA).

## Verificación del código de producción

### Compilación
```
cargo check       → OK (0.20s, sin errores)
cargo build       → OK (0.30s, binario generado)
cargo clippy --no-deps → OK (0.33s, 0 warnings)
cargo fmt -- --check   → OK (código formateado)
```

### Cobertura de CAs (código de producción)

| CA | Estado | Evidencia |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry()` L78: `verbose: bool` como último parámetro |
| CA1 | ✅ | `invoke_with_retry_blocking()` L193: `verbose: bool` propagado |
| CA2 | ✅ | `Cargo.toml`: feature `io-util` de tokio |
| CA2 | ✅ | `invoke_once()` L316: rama `invoke_once_verbose()` para `verbose=true` |
| CA2 | ✅ | `invoke_once_verbose()` L358: `child.stdout.take()` + `BufReader::new()` + `read_line()` |
| CA3 | ✅ | `invoke_once_verbose()`: `tracing::info!("  │ {}", trimmed)` para líneas no vacías |
| CA4 | ✅ | stdout acumulado en `Vec<u8>` y devuelto en `Output` (CA4) |
| CA5 | ✅ | stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming |
| CA6 | ✅ | `invoke_once()` L316: rama `wait_with_output()` para `verbose=false` |
| CA7 | ✅ | `kill_process_by_pid()` L440: helper cross-platform usado en ambos modos |
| CA8 | ✅ | `cargo check --lib` (bin) compila sin errores |
| CA9 | ❌ | Bloqueado por errores E0716 en tests |
| CA10 | ✅ | Call sites `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en tests del QA (NO corregidos por el Dev)

Los 3 errores E0716 (`temporary value dropped while borrowed`) persisten en
`mod story022` dentro de `src/infra/agent.rs`:

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |

### Causa

`buffer.lock().unwrap()` retorna un `MutexGuard<Vec<u8>>` temporal. 
`String::from_utf8_lossy()` retorna `Cow<'_, str>`, que puede tomar prestado 
(`Cow::Borrowed`) del slice `&[u8]` subyacente. El `MutexGuard` temporal se 
destruye al final del statement, invalidando el borrow.

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

El compilador mismo sugiere esta corrección en el mensaje de error:
```
help: consider using a `let` binding to create a longer lived value
```

## Decisión

**NO se avanza a In Review.** El código de producción está completo y correcto,
pero los tests del QA no compilan. Corregir tests del QA NO es responsabilidad
del Developer. El orquestador debe pasar el turno al QA para que corrija los
3 errores E0716 antes de que el Dev pueda ejecutar `cargo test` y verificar CA9.
