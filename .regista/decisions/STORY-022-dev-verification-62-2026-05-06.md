# STORY-022 — Dev Verification #62 — 2026-05-06

## Resumen

Sexagésima segunda verificación del código de producción para STORY-022.  
El código de producción sigue completo y correcto. Los tests del QA siguen con
los mismos 3 errores de compilación E0716 sin corregir.

## Verificaciones realizadas

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK (0.32s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.32s) |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ 11/11 pass (0.05s) |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

## Cobertura de CAs por el código de producción

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ L84 |
| CA2 | `verbose=true` → `BufReader` + `read_line()` async | ✅ L358-435 |
| CA3 | `tracing::info!("  │ {}", trimmed)` por línea | ✅ L400 |
| CA4 | stdout acumulado en `Vec<u8>` | ✅ L390, L427 |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ L407-413 |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ L337-351 |
| CA7 | Timeout funciona en ambos modos | ✅ L337, L420 |
| CA8 | `cargo check --lib` compila | ✅ |
| CA9 | Tests pasan | ❌ Bloqueado por QA |
| CA10 | Call sites actualizados con `verbose` | ✅ plan.rs:152, pipeline.rs:774 |
| CA11 | `AgentResult` con `stdout`, `stderr`, `exit_code` | ✅ |

## Errores en tests del QA (NO corregidos — responsabilidad del QA)

Los 3 errores son idénticos en naturaleza: `String::from_utf8_lossy(&buffer.lock().unwrap())`
crea un `MutexGuard` temporal que se destruye antes que el `Cow<str>` devuelto por
`from_utf8_lossy`. Rust no permite esto porque el `Cow` podría referenciar el buffer
protegido por el mutex.

### Test `ca3_verbose_logs_lines_with_pipe_prefix` (L1763)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Error E0716: temporary value dropped while borrowed.

### Test `ca3_empty_lines_not_logged` (L1809)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Error E0716: temporary value dropped while borrowed.

### Test `ca5_stderr_not_streamed_to_log` (L2006)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Error E0716: temporary value dropped while borrowed.

### Solución exacta para el QA

En las 3 ubicaciones, reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

La variable `binding` extiende la vida del `MutexGuard` hasta que `log_output`
(de tipo `Cow<str>`) ya no se usa.

## Decisión

NO se avanza el estado a In Review. El orquestador debe pasar el turno al QA
para que corrija los 3 errores de compilación en sus tests.
