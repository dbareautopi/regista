# STORY-022 — Dev Verification #108 — 2026-05-06

## Resultado
❌ No se avanza a In Review — los tests del QA no compilan (3 errores E0716).

## Verificación del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` | ✅ OK (0.18s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.24s) |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pasan (0.05s) |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

## Cumplimiento del código de producción (CA1-CA8, CA10-CA11)

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último parámetro | ✅ L84 |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` | ✅ L358 |
| CA3 | Líneas stdout con prefijo `  │ ` vía `tracing::info!` | ✅ L390 |
| CA4 | Stdout acumulado en `Vec<u8>` y devuelto en `Output` | ✅ L378, L415 |
| CA5 | Stderr en `tokio::spawn` separada, sin streaming | ✅ L399-L404 |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ L327 |
| CA7 | Timeout mata proceso por PID en ambos modos | ✅ L326, L412, L440 |
| CA8 | `cargo check --bin regista` compila | ✅ |
| CA10 | Call sites (`plan.rs:152`, `pipeline.rs:774`) pasan `false` | ✅ |
| CA11 | `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` | ✅ |

## Errores en tests del QA (NO corregidos — 108ª iteración)

### Error E0716: temporary value dropped while borrowed

Los 3 tests usan el patrón:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Donde `buffer.lock().unwrap()` devuelve un `MutexGuard` temporal que se destruye al final del statement, pero `String::from_utf8_lossy` devuelve un `Cow<str>` que lo referencia. El borrow checker rechaza esto.

### Tests afectados

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

### CA9 bloqueado

`cargo test -- story022` no puede verificarse hasta que el QA corrija los 3 errores de compilación.

## Conclusión

El código de producción está completo y correcto. Los errores están exclusivamente en los tests del QA (módulo `story022`). Siguiendo el protocolo del orquestador, NO se corrigen los tests del QA y NO se avanza a In Review. El orquestador debe pasar el turno al QA.
