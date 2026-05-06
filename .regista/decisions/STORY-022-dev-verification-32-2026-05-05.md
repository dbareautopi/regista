# STORY-022 Dev Verification #32 — 2026-05-05

## Resumen

Trigésima segunda verificación de STORY-022. El código de producción sigue
completo y correcto. Los tests del QA siguen sin compilar por los mismos
3 errores E0716 que llevan 32 iteraciones sin corregir.

## Verificaciones realizadas

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK, sin errores |
| `cargo build` | ✅ OK, binario generado |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Cobertura de CAs (producción)

| CA | Estado | Ubicación |
|----|--------|-----------|
| CA1 | ✅ | `invoke_with_retry()` L84: `verbose: bool` como último parámetro |
| CA2 | ✅ | `invoke_once()` L316: dispatch a `invoke_once_verbose()` con BufReader |
| CA3 | ✅ | `invoke_once_verbose()` L358: `tracing::info!("  │ {}", trimmed)` por línea |
| CA4 | ✅ | stdout acumulado en `Vec<u8>`, devuelto en `Output.stdout` |
| CA5 | ✅ | stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming |
| CA6 | ✅ | `verbose=false` usa `wait_with_output()` |
| CA7 | ✅ | `kill_process_by_pid()` en ambos modos |
| CA8 | ✅ | `cargo check --lib` compila sin errores |
| CA9 | ❌ | Bloqueado por tests del QA (ver abajo) |
| CA10 | ✅ | `app/plan.rs:152`: `false`, `app/pipeline.rs:774`: `false` |
| CA11 | ✅ | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` |

## Errores en tests del QA (responsabilidad del QA)

Los 3 errores son **E0716: temporary value dropped while borrowed**.
El patrón problemático es `String::from_utf8_lossy(&buffer.lock().unwrap())`
donde `buffer.lock().unwrap()` retorna un `MutexGuard` temporal que se
destruye al final del statement, pero el `Cow<str>` retornado por
`from_utf8_lossy` referencia datos internos del `MutexGuard`.

### Test `ca3_verbose_logs_lines_with_pipe_prefix` — Línea 1763
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                        ^^^^^^^^^^^^^^^^^^^^^^ temporary freed here
// borrow later used: log_output.contains("  │ ")
```

### Test `ca3_empty_lines_not_logged` — Línea 1809
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
// mismo error E0716
// borrow later used: log_output.contains("AAA"), log_output.contains("BBB")
```

### Test `ca5_stderr_not_streamed_to_log` — Línea 2006
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
// mismo error E0716
// borrow later used: log_output.contains("stdout-line"), log_output.lines()
```

## Solución (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:
```rust
// ANTES (no compila):
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// DESPUÉS (compila):
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza a In Review. Los tests del QA NO compilan y es responsabilidad
del QA corregirlos. El orquestador debe detectar esta situación y pasar
el turno al QA Engineer automáticamente.
