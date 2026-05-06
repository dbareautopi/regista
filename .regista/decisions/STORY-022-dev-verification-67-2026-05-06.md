# STORY-022: Verification #67 — 2026-05-06

## Resumen

Sexagésima séptima verificación del código de producción de STORY-022.
El código de producción está completo y correcto. Los tests del QA
no compilan por 3 errores E0716 idénticos a iteraciones anteriores.

## Estado de la implementación

### Código de producción (completo y correcto)

| Archivo | Función | Descripción |
|---------|---------|-------------|
| `src/infra/agent.rs:78` | `invoke_with_retry()` | Acepta `verbose: bool` como último parámetro (CA1) |
| `src/infra/agent.rs:193` | `invoke_with_retry_blocking()` | Propaga `verbose: bool` (CA1, CA10) |
| `src/infra/agent.rs:311` | `invoke_once()` | `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6) |
| `src/infra/agent.rs:358` | `invoke_once_verbose()` | Streaming línea a línea con `BufReader`, `tracing::info!("  │ {}", trimmed)`, stderr en `tokio::spawn` (CA2-CA5) |
| `src/infra/agent.rs:440` | `kill_process_by_pid()` | Helper cross-platform para timeout (CA7) |
| `src/app/plan.rs:152` | call site | Pasa `false` (CA10) |
| `src/app/pipeline.rs:774` | call site | Pasa `false` (CA10) |
| `Cargo.toml` | tokio features | `io-util` añadido (CA2) |

### Resultados de verificación

| Comando | Resultado |
|---------|-----------|
| `cargo check` (0.30s) | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` (0.31s) | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` (0.28s) | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

### Errores en tests del QA

Los 3 errores son el mismo patrón E0716: `MutexGuard` temporal destruido
antes que el `Cow<str>` devuelto por `String::from_utf8_lossy()`.

| # | Test | Línea | Expresión problemática |
|---|------|-------|----------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## CAs cubiertos

| CA | Estado | Notas |
|----|--------|-------|
| CA1 | ✅ Implementado | `verbose: bool` como último parámetro en `invoke_with_retry()` y `invoke_with_retry_blocking()` |
| CA2 | ✅ Implementado | `BufReader` + `read_line()` en `invoke_once_verbose()`, depende de `io-util` |
| CA3 | ✅ Implementado | `tracing::info!("  │ {}", trimmed)` por línea no vacía |
| CA4 | ✅ Implementado | stdout acumulado en `Vec<u8>` |
| CA5 | ✅ Implementado | stderr en `tokio::spawn` sin streaming |
| CA6 | ✅ Implementado | `verbose=false` usa `wait_with_output()` |
| CA7 | ✅ Implementado | `kill_process_by_pid()` en ambos modos |
| CA8 | ✅ Verificado | `cargo check` OK |
| CA9 | ❌ Bloqueado | Tests del QA no compilan (E0716 × 3) |
| CA10 | ✅ Implementado | Call sites en plan.rs y pipeline.rs pasan `false` |
| CA11 | ✅ Implementado | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` |

## Decisión

NO se avanza a In Review. El código de producción está completo y correcto.
Los tests del QA tienen 3 errores de compilación E0716 que son
responsabilidad del QA corregir. El orquestador debe pasar el turno
al QA Engineer.
