# STORY-022 — Dev — Decimoctava verificación — 2026-05-05

## Resultado

❌ No se avanza a In Review — los tests del QA en `mod story022` no compilan.

---

## Verificación del código de producción

| Verificación | Resultado |
|---|---|
| `cargo build` | ✅ OK (0.32s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.25s) |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pasan |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

---

## Implementación de producción (completa — CA1-CA8, CA10-CA11)

| CA | Estado | Descripción |
|----|--------|-------------|
| CA1 | ✅ | `invoke_with_retry()` acepta `verbose: bool` como último parámetro |
| CA2 | ✅ | `invoke_once()` con `verbose=true` → `invoke_once_verbose()` con `BufReader::new()` + `read_line()` |
| CA3 | ✅ | Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` |
| CA4 | ✅ | stdout acumulado en `Vec<u8>` y devuelto en `Output` |
| CA5 | ✅ | stderr en `tokio::spawn` separado, `read_to_end()`, sin streaming |
| CA6 | ✅ | `verbose=false` → `wait_with_output()` (comportamiento actual) |
| CA7 | ✅ | `kill_process_by_pid()` extraído para timeout en ambos modos |
| CA8 | ✅ | `cargo build` compila el binario |
| CA10 | ✅ | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` |
| CA11 | ✅ | `AgentResult`: `stdout: String`, `stderr: String`, `exit_code: i32` |

---

## Errores E0716 en tests del QA (NO corregidos)

Los 3 errores `E0716` (temporary value dropped while borrowed) están en el módulo `story022` del archivo `src/infra/agent.rs`.

### Causa raíz

En las 3 ubicaciones, se usa:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` devuelto por `buffer.lock().unwrap()` es un temporal que se destruye al final del statement. Sin embargo, `String::from_utf8_lossy` devuelve un `Cow<str>` que toma prestado de ese `&[u8]` — y el `MutexGuard` (que mantiene el `Vec<u8>`) ya no existe.

### Ubicaciones exactas (18ª iteración)

| # | Test | Línea | Código ofensivo |
|---|------|-------|-----------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Solución requerida (responsabilidad del QA)

Reemplazar cada ocurrencia por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

Esto mantiene vivo el `MutexGuard` (a través de `binding`) mientras `log_output` (el `Cow<str>`) está en uso.

---

## Decisión

- **NO se corrigen los tests del QA** — es su responsabilidad.
- **NO se avanza a In Review** — el orquestador debe pasar el turno al QA.
- El código de producción es completo y correcto. Una vez que el QA corrija los 3 errores E0716, `cargo test -- story022` debería pasar todos los tests.
