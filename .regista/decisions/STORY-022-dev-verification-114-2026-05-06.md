# STORY-022 — Dev Verification 114 — 2026-05-06

## Resultado
❌ Bloqueado — 3 tests del QA no compilan (E0716)

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | ✅ OK (0.33s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.35s) |
| `cargo fmt -- --check` | ✅ OK, formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan (0.35s) |

## Código de producción (CA1-CA8, CA10-CA11): completo y correcto

| Función | Línea | Descripción |
|---------|-------|-------------|
| `invoke_with_retry()` | 84 | `verbose: bool` como último parámetro (CA1) |
| `invoke_with_retry_blocking()` | 199 | `verbose: bool` propagado (CA1, CA10) |
| `invoke_once()` | 290 | `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6) |
| `invoke_once_verbose()` | 358 | `child.stdout.take()` + `BufReader::new()` + `read_line()` async, `tracing::info!("  │ {}", trimmed)`, `Vec<u8>` acumulado, stderr en `tokio::spawn` (CA2-CA5) |
| `kill_process_by_pid()` | 440 | Helper cross-platform para timeout (CA7) |

## Call sites actualizados

| Archivo | Línea | Valor |
|---------|-------|-------|
| `app/plan.rs` | 152 | `false` |
| `app/pipeline.rs` | 774 | `false` |
| Tests pre-existentes | varios | `false` |

## `Cargo.toml`

Feature `io-util` presente en tokio: ✅

## Errores en tests del QA (NO corregidos)

Los siguientes 3 tests en `mod story022` fallan con **error E0716** (temporary value dropped while borrowed):

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz

`buffer.lock().unwrap()` retorna un `MutexGuard<Vec<u8>>` temporal que se destruye al final del statement, antes de que el `Cow<str>` devuelto por `String::from_utf8_lossy()` sea usado. El borrow checker de Rust detecta correctamente que la referencia interna del `Cow<str>` apunta a memoria ya liberada.

### Solución (responsabilidad del QA)

En cada una de las 3 ubicaciones, reemplazar:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Acción recomendada

1. El orquestador debe pasar el turno al QA
2. QA debe aplicar la corrección en las 3 líneas indicadas
3. Verificar con `cargo test --bin regista -- infra::agent::story022`

---

*Documento generado automáticamente por el rol Dev durante la 114ª verificación de STORY-022.*
