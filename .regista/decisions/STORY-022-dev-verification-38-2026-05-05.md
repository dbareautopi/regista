# STORY-022: Trigésima octava verificación (2026-05-05)

## Verificación del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` | ✅ OK, sin errores |
| `cargo build` | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |

## Resumen de la implementación de producción

El código de producción en `src/infra/agent.rs` está completo y cubre CA1-CA8, CA10-CA11:

- **Línea ~78**: `invoke_with_retry()` acepta `verbose: bool` como último parámetro (CA1)
- **Línea ~193**: `invoke_with_retry_blocking()` acepta `verbose: bool`, lo propaga (CA1, CA10)
- **Línea ~316**: `invoke_once()` acepta `verbose: bool`. `false` → `wait_with_output()`. `true` → `invoke_once_verbose()` (CA2, CA6)
- **Línea ~358**: `invoke_once_verbose()` con `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async. Cada línea no vacía: `tracing::info!("  │ {}", trimmed)`. stdout acumulado en `Vec<u8>`. stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming (CA2, CA3, CA4, CA5)
- **Línea ~440**: `kill_process_by_pid()` helper extraído para timeout cross-platform en ambos modos (CA7)
- **Call sites**: `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10)
- **`AgentResult`**: mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11)
- **`Cargo.toml`**: feature `io-util` añadido a tokio (CA2)

## Errores en tests del QA (NO corregidos)

`cargo test -- story022` NO compila. 3 errores E0716 en `mod story022`:

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que el `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error E0716 |

### Causa raíz

En las 3 líneas, `buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal que se destruye al final del statement, pero `String::from_utf8_lossy` devuelve un `Cow<str>` que lo referencia. El borrow checker de Rust correctamente rechaza esto como E0716.

### Solución requerida (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO se corrigen los tests**: responsabilidad del QA.
- **NO se avanza a In Review**: CA9 bloqueado, `cargo test` no puede verificarse hasta que el QA corrija los 3 errores de compilación.
- El orquestador debe pasar el turno al QA automáticamente.
