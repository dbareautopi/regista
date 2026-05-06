# STORY-022 — developer — 2026-05-06T00:00:00

## Resultado
❌ Tests no compilan — no se avanza a In Review

## Verificación del código de producción (CA1-CA8, CA10-CA11)

### Comprobaciones superadas

| Comando | Resultado |
|---------|-----------|
| `cargo check --bin regista` | ✅ OK |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ OK, 11/11 |

### Código de producción completo y correcto

- `invoke_with_retry()` (L84): `verbose: bool` como último parámetro (CA1) ✅
- `invoke_with_retry_blocking()` (L199): `verbose: bool` propagado (CA1, CA10) ✅
- `invoke_once()` (L316): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6) ✅
- `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async, `tracing::info!("  │ {}", trimmed)` por línea no vacía, `Vec<u8>` acumulado, stderr en `tokio::spawn` sin streaming (CA2-CA5) ✅
- `kill_process_by_pid()` (L440): helper cross-platform para timeout (CA7) ✅
- Call sites en `app/plan.rs` y `app/pipeline.rs` pasan `false` (CA10) ✅
- Call sites en tests pre-existentes pasan `false` (CA10) ✅
- `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11) ✅
- `Cargo.toml`: feature `io-util` en tokio (CA2) ✅

## Tests del QA que NO compilan

Errores E0716: "temporary value dropped while borrowed"

| Test | Línea | Código problemático |
|------|-------|---------------------|
| `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa

`buffer.lock().unwrap()` retorna un `MutexGuard<Vec<u8>>` temporal. Al pasar `&` sobre este temporal, `String::from_utf8_lossy` obtiene una referencia que apunta a datos protegidos por el `MutexGuard`. El `MutexGuard` temporal se destruye al final del `let`, pero el `Cow<str>` retornado por `from_utf8_lossy` (variante `Borrowed`) sigue referenciando los datos ya liberados.

### Solución exacta (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

En las 3 ubicaciones: líneas 1763, 1809, 2006.

## Acción del Developer

NO se corrigen los tests (responsabilidad del QA).  
NO se avanza a In Review.  
El orquestador debe pasar el turno al QA.
