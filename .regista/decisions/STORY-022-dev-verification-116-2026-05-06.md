# STORY-022 — Dev Verification 116 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (E0716)

## Verificación del código de producción

### Checks pasados
| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | ✅ OK (0.17s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pasan |

### CA1-CA8, CA10-CA11: OK
- `invoke_with_retry()` acepta `verbose: bool` como último parámetro
- `invoke_once_verbose()` implementa streaming con `BufReader` + `read_line()`
- Líneas stdout logueadas con `tracing::info!("  │ {}", trimmed)`
- stdout completo acumulado en `Vec<u8>` y devuelto en `Output`
- stderr en `tokio::spawn` separada, sin streaming
- `verbose=false` usa `wait_with_output()` (comportamiento actual)
- Timeout funciona en ambos modos (mata proceso por PID)
- Call sites actualizados (plan.rs:152, pipeline.rs:774 → pasan `false`)
- `AgentResult` contiene `stdout`, `stderr`, `exit_code`
- `Cargo.toml` incluye feature `io-util`

### CA9: BLOQUEADO
`cargo test -- infra::agent` no compila por 3 errores E0716 en tests del QA.

## Errores en tests del QA

Los 3 tests usan el patrón:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` temporal de `buffer.lock().unwrap()` se destruye al final del statement,
antes de que se use el `Cow<str>` retornado por `from_utf8_lossy`.

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | E0716: temporary value dropped while borrowed |
| 2 | `ca3_empty_lines_not_logged` | 1809 | E0716: temporary value dropped while borrowed |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | E0716: temporary value dropped while borrowed |

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Aplicar en las 3 ubicaciones.

## Decisión

NO se corrigen los tests (responsabilidad del QA, según convenciones del proyecto).
NO se avanza el estado a In Review.
El orquestador debe pasar el turno al QA.
