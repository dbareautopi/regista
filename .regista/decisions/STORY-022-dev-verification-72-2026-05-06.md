# STORY-022 — Dev — 2026-05-06 (verificación 72)

## Resultado
❌ Tests no compilan. Código de producción correcto.

## Verificación del código de producción

### Compilación y linting
- `cargo check`: OK, sin errores (0.16s)
- `cargo clippy --no-deps --bin regista`: OK, 0 warnings
- `cargo fmt -- --check`: OK, código formateado
- `cargo test --test architecture`: OK, 11/11 pasan

### Cobertura de CAs de producción
Todos los criterios de aceptación que competen al código de producción están implementados:

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último argumento | ✅ |
| CA2 | `verbose=true` → `invoke_once_verbose()` con `BufReader::new()` + `read_line()` | ✅ |
| CA3 | `tracing::info!("  │ {}", trimmed)` para cada línea no vacía | ✅ |
| CA4 | stdout acumulado en `Vec<u8>` y devuelto en `Output` | ✅ |
| CA5 | stderr en `tokio::spawn` separado, sin streaming al log | ✅ |
| CA6 | `verbose=false` → `wait_with_output()` (comportamiento actual) | ✅ |
| CA7 | `kill_process_by_pid()` cross-platform para timeout en ambos modos | ✅ |
| CA8 | `cargo check` compila sin errores | ✅ |
| CA10 | Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` | ✅ |
| CA11 | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` | ✅ |

### Estructura del código de producción
```
invoke_with_retry(provider, instruction, prompt, limits, opts, verbose)
  └─ invoke_once(provider, instruction, prompt, timeout, verbose)
       ├─ verbose=false → child.wait_with_output() + timeout
       └─ verbose=true  → invoke_once_verbose(child, pid, provider, timeout)
            ├─ tokio::spawn → BufReader::new(stdout) → read_line() loop
            │   └─ tracing::info!("  │ {}", trimmed) por línea no vacía
            ├─ tokio::spawn → stderr.read_to_end() (sin streaming)
            └─ kill_process_by_pid(pid) en timeout
```

## Errores en tests del QA

3 errores E0716 (temporary value dropped while borrowed) en `mod story022`:

| # | Test | Línea | Código ofensivo |
|---|------|-------|-----------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz
`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal. `String::from_utf8_lossy` toma `&[u8]` prestado de ese temporal y devuelve un `Cow<str>` que puede ser `Cow::Borrowed`, referenciando al temporal. El temporal se destruye al final del `let`, pero `log_output` se usa después → E0716.

### Solución (responsabilidad del QA)
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión
- **NO** se corrigen los tests (responsabilidad del QA).
- **NO** se avanza el estado a `In Review` (CA9 bloqueado).
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
