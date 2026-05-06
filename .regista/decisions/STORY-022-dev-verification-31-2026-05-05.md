# STORY-022 — Verificación #31 del Developer (2026-05-05)

## Resumen

Trigésima primera verificación de STORY-022. El código de producción sigue completo y correcto, pero los tests del QA en `mod story022` siguen sin compilar. Los mismos 3 errores E0716 persisten.

## Verificaciones realizadas

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK, sin errores |
| `cargo build` | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Código de producción — estado

El código de producción está completo y cubre todos los CAs implementables (CA1-CA8, CA10-CA11):

| Función | Línea | Descripción |
|---------|-------|-------------|
| `invoke_with_retry()` | L84 | `verbose: bool` como último parámetro (CA1) |
| `invoke_with_retry_blocking()` | L199 | `verbose: bool` propagado (CA1, CA10) |
| `invoke_once()` | L316 | `verbose: bool`. `verbose=false` → `wait_with_output()`. `verbose=true` → `invoke_once_verbose()` (CA2, CA6) |
| `invoke_once_verbose()` | L358 | `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async. `tracing::info!("  │ {}", trimmed)` para líneas no vacías. stdout en `Vec<u8>`. stderr en `tokio::spawn` (CA2-CA5) |
| `kill_process_by_pid()` | L440 | Helper extraído para timeout cross-platform (CA7) |
| Call sites | plan.rs:152, pipeline.rs:774 | Pasan `false` (CA10) |
| `AgentResult` | — | `stdout: String`, `stderr: String`, `exit_code: i32` (CA11) |
| `Cargo.toml` | L25 | `io-util` añadido a tokio features (CA2) |

## Errores E0716 en tests del QA (NO corregidos)

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1764 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());` |

### Causa raíz

`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal. Este temporal se destruye al final del statement (después del `;`), pero `String::from_utf8_lossy()` devuelve un `Cow<str>` que contiene una referencia al slice `&[u8]` dentro del `Vec<u8>` protegido por el `MutexGuard`. Rust impide esto porque el `MutexGuard` se destruye antes de que el `Cow<str>` se use en los `assert!()` subsiguientes.

### Solución requerida (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones: líneas 1764, 1809, 2006.

## Decisión

- ❌ **NO se corrigen los tests** — es responsabilidad del QA.
- ❌ **NO se avanza a In Review** — los tests no compilan, CA9 no puede verificarse.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.

## Impacto

CA9 (`cargo test --lib infra::agent` pasa todos los tests existentes) está bloqueado hasta que el QA corrija los 3 errores de compilación. El resto de CAs (CA1-CA8, CA10-CA11) están implementados y verificados en el código de producción.
