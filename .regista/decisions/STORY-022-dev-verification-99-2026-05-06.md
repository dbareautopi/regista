# STORY-022 Dev Verification #99 — 2026-05-06

## Resumen

Nonagésima novena verificación del código de producción para STORY-022.  
El código de producción está completo y correcto. Los tests del QA tienen 3 errores E0716  
que impiden la compilación. NO se corrigen (responsabilidad del QA). NO se avanza a In Review.

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | OK (0.31s) |
| `cargo clippy --no-deps --bin regista` | OK, 0 warnings |
| `cargo fmt -- --check` | OK |
| `cargo test --test architecture` | 11/11 OK |

## CAs cubiertos por el código de producción

| CA | Estado | Detalle |
|----|--------|---------|
| CA1 | ✅ | `invoke_with_retry()` (L78): `verbose: bool` como último parámetro |
| CA2 | ✅ | `invoke_once()` (L311): `verbose=true` → `invoke_once_verbose()` con `child.stdout.take()` + `BufReader` |
| CA3 | ✅ | `invoke_once_verbose()` (L358): `tracing::info!("  │ {}", trimmed)` por cada línea no vacía |
| CA4 | ✅ | `invoke_once_verbose()`: stdout acumulado en `Vec<u8>` y devuelto |
| CA5 | ✅ | `invoke_once_verbose()`: stderr en `tokio::spawn` separado, sin streaming |
| CA6 | ✅ | `invoke_once()`: `verbose=false` → `wait_with_output()` |
| CA7 | ✅ | `kill_process_by_pid()` (L440): cross-platform, usado en ambos modos |
| CA8 | ✅ | `cargo check --bin regista` compila sin errores |
| CA10 | ✅ | `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`; `invoke_with_retry_blocking()` recibe y propaga `verbose` |
| CA11 | ✅ | `AgentResult` contiene `stdout: String`, `stderr: String`, `exit_code: i32` |

## Errores en los tests del QA (NO corregidos)

Los 3 tests fallan con **E0716: temporary value dropped while borrowed**.  
El patrón problemático es el mismo en los 3:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se destruye  
al final del statement, pero `from_utf8_lossy` retorna un `Cow<str>` que lo referencia.

### Tests afectados

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- ❌ NO se corrigen los tests (protocolo Dev: es responsabilidad del QA)
- ❌ NO se avanza el estado a In Review
- ✅ Se documenta en Activity Log de STORY-022.md
- 🔄 El orquestador debe pasar el turno al QA (transición QA: Tests Ready → Tests Ready, corrección de tests)
