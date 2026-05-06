# STORY-022 — Dev Verification #66 (2026-05-06)

## Resumen

Verificación completa del código de producción para STORY-022.
El código de producción es correcto y completo. Los tests del QA tienen
3 errores E0716 que impiden la compilación del módulo `mod story022`.

## Verificaciones realizadas

| Comando | Tiempo | Resultado |
|---------|--------|-----------|
| `cargo check` | 5.57s | ✅ OK, sin errores |
| `cargo clippy --no-deps --bin regista` | 0.29s | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | instant | ✅ OK, código formateado |
| `cargo test --test architecture` | 0.38s | ✅ OK, 11/11 pasan |
| `cargo test -- story022` | — | ❌ NO compila (3 errores E0716) |

## Código de producción

La implementación cubre CA1-CA8 y CA10-CA11:

### `invoke_with_retry()` (L78)

```rust
pub async fn invoke_with_retry(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
    verbose: bool,  // ← CA1: nuevo parámetro
) -> anyhow::Result<AgentResult> {
    // ...
    invoke_once(provider, instruction_path, &current_prompt, timeout, verbose).await
    // ...
}
```

### `invoke_with_retry_blocking()` (L199)

```rust
pub fn invoke_with_retry_blocking(
    // ...
    verbose: bool,
) -> anyhow::Result<AgentResult> {
    RUNTIME.block_on(invoke_with_retry(..., verbose))
}
```

### `invoke_once()` (L316)

```rust
async fn invoke_once(
    // ...
    verbose: bool,
) -> anyhow::Result<Output> {
    if verbose {
        invoke_once_verbose(child, pid, provider, timeout).await  // CA2
    } else {
        child.wait_with_output()  // CA6: comportamiento actual
    }
}
```

### `invoke_once_verbose()` (L358)

- CA2: `BufReader::new(child.stdout.take().unwrap())` + `read_line()` en bucle async
- CA3: `tracing::info!("  │ {}", trimmed)` por línea no vacía
- CA4: `Vec<u8>` acumulado vía `extend_from_slice(line.as_bytes())`
- CA5: stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming

### `kill_process_by_pid()` (L440)

Helper cross-platform (Unix: `kill -9`, Windows: `taskkill /F`). CA7: el timeout
funciona en ambos modos — mata por PID cuando el child ya fue movido.

### Call sites

- `app/plan.rs:152`: `invoke_with_retry_blocking(..., false)` ✅
- `app/pipeline.rs:774`: `invoke_with_retry(..., false).await` ✅

### AgentResult

```rust
pub struct AgentResult {
    pub stdout: String,      // CA11
    pub stderr: String,      // CA11
    pub exit_code: i32,      // CA11
}
```

### Cargo.toml

```toml
tokio = { ..., features = ["io-util"] }  # CA2
```

## Errores en tests del QA

Los siguientes 3 tests en `mod story022` no compilan (error E0716).
**NO se corrigen — es responsabilidad del QA.**

### Error 1: `ca3_verbose_logs_lines_with_pipe_prefix` (L1763)

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
//                                         ^^^^^^^^^^^^^^^^^^^^^^
//                                         MutexGuard temporal destruido
```

**Fix:**
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

### Error 2: `ca3_empty_lines_not_logged` (L1809)

Ídem — mismo patrón `String::from_utf8_lossy(&buffer.lock().unwrap())`.

### Error 3: `ca5_stderr_not_streamed_to_log` (L2006)

Ídem — mismo patrón `String::from_utf8_lossy(&buffer.lock().unwrap())`.

## Conclusión

- **Código de producción**: ✅ completo y correcto
- **Tests del QA**: ❌ 3 errores E0716 que bloquean CA9
- **Acción**: NO se avanza a In Review. El orquestador debe pasar el turno al QA.
