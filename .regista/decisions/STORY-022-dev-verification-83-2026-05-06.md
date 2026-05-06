# STORY-022 — Dev — verificación #83 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716)

## Verificación del código de producción

### Comandos ejecutados
| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` | ✅ OK (0.22s) |
| `cargo build` | ✅ OK (0.29s), binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK (0.32s), 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

### CAs cubiertos por el código de producción

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ `src/infra/agent.rs:84` |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader` + `read_line()` | ✅ `src/infra/agent.rs:316,358` |
| CA3 | Líneas no vacías logueadas con `tracing::info!("  │ {}", trimmed)` | ✅ `src/infra/agent.rs:358` |
| CA4 | stdout acumulado en `Vec<u8>` y devuelto en `Output` | ✅ `src/infra/agent.rs:358` |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ `src/infra/agent.rs:358` |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ `src/infra/agent.rs:316` |
| CA7 | Timeout mata proceso por PID en ambos modos | ✅ `kill_process_by_pid()` en L440 |
| CA8 | `cargo check --lib` compila | ✅ |
| CA9 | Tests pasan | ❌ Bloqueado por QA |
| CA10 | Call sites actualizados con `verbose` | ✅ `plan.rs:152`, `pipeline.rs:774` pasan `false` |
| CA11 | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` | ✅ `src/infra/agent.rs:35` |

### Errores en tests del QA (responsabilidad del QA)

Los 3 errores son idénticos: uso de `MutexGuard` temporal como argumento de `String::from_utf8_lossy()`.
El `MutexGuard` retornado por `buffer.lock().unwrap()` se destruye al final del statement,
mientras que el `Cow<str>` devuelto por `String::from_utf8_lossy` aún lo referencia.

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Solución exacta

Reemplazar en las 3 líneas:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

### Decisión

NO se corrigen los tests. Es responsabilidad del QA.  
NO se avanza el estado a In Review.  
El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
