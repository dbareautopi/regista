# STORY-022 — Dev Verification #16 — 2026-05-05

## Resultado
⚠️ Tests NO compilan — responsabilidad del QA. NO se avanza a In Review.

## Verificaciones de producción (todas OK)

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK (0.15s) |
| `cargo build` | ✅ OK (0.14s) |
| `cargo clippy --no-deps` | ✅ OK, 0 warnings (0.26s) |
| `cargo fmt -- --check` | ✅ OK, código formateado |

## Código de producción (completo, cubre CA1-CA8, CA10-CA11)

- **`Cargo.toml`**: feature `io-util` añadido a tokio (CA2).
- **`invoke_with_retry()` (L78)**: `verbose: bool` como último parámetro (CA1).
- **`invoke_with_retry_blocking()` (L193)**: `verbose: bool` propagado (CA1, CA10).
- **`invoke_once()` (L316)**: nuevo parámetro `verbose: bool`. `verbose=false` → `wait_with_output()`. `verbose=true` → `invoke_once_verbose()` (CA2, CA6).
- **`invoke_once_verbose()` (L358)**: `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async. Cada línea no vacía: `tracing::info!("  │ {}", trimmed)`. stdout acumulado en `Vec<u8>`. stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming (CA2, CA3, CA4, CA5).
- **`kill_process_by_pid()` (L440)**: helper extraído para timeout cross-platform en ambos modos (CA7).
- **Call sites**: `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10).
- **`AgentResult`**: mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11).

## Errores en tests del QA (NO corregidos — responsabilidad del QA, 16ª iteración)

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | E0716: `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que el `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | E0716: `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | E0716: `String::from_utf8_lossy(&buffer.lock().unwrap())` — mismo error |

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

### CA9 bloqueado

`cargo test` no puede ejecutarse hasta que el QA corrija los 3 errores de compilación E0716.

## Decisión

NO se avanza a In Review. El orquestador debe pasar el turno al QA para que corrija los errores E0716 en los tests.
