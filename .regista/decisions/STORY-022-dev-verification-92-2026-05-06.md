# STORY-022 — Dev Verification #92 — 2026-05-06

## Resultado: Bloqueado — 3 errores de compilación E0716 en tests del QA

## Verificaciones realizadas

| Verificación | Resultado |
|---|---|
| `cargo check --bin regista` | ✅ OK (0.42s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.46s) |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan (0.43s) |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |
| `cargo test` (todos) | ❌ `could not compile regista (bin "regista" test) due to 3 previous errors` |

## Código de producción: completo y correcto

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` (L78) acepta `verbose: bool` como último parámetro | ✅ |
| CA1 | `invoke_with_retry_blocking()` (L199) propaga `verbose: bool` | ✅ |
| CA2 | `invoke_once()` (L290): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` | ✅ |
| CA3 | `invoke_once_verbose()` (L358): `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async | ✅ |
| CA3 | `tracing::info!("  │ {}", trimmed)` por línea no vacía | ✅ |
| CA4 | Stdout acumulado en `Vec<u8>` y devuelto como parte del resultado | ✅ |
| CA5 | Stderr leído en `tokio::spawn` separada, sin streaming al log | ✅ |
| CA6 | `verbose=false` usa `wait_with_output()` (comportamiento actual) | ✅ |
| CA7 | Timeout funciona en ambos modos (`kill_process_by_pid` cross-platform) | ✅ |
| CA8 | `cargo check --bin regista` compila sin errores | ✅ |
| CA10 | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` | ✅ |
| CA10 | Call sites en tests pre-existentes pasan `false` | ✅ |
| CA11 | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` | ✅ |
| — | `Cargo.toml` incluye feature `io-util` en tokio | ✅ |
| CA9 | `cargo test -- story022` pasa todos los tests | ❌ Bloqueado por errores de compilación en los tests |

## Errores de compilación en tests del QA (NO corregidos)

| # | Test | Línea | Error |
|---|------|-------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — `MutexGuard` temporal destruido antes que `Cow<str>` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | idem E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | idem E0716 |

### Solución exacta (responsabilidad del QA)

En las 3 ubicaciones (líneas 1763, 1809, 2006), reemplazar:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

- **NO se corrigen los tests** — es trabajo del QA.
- **NO se avanza a In Review** — los tests no compilan.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
