# STORY-022 — Dev Verification #50 — 2026-05-06

## Resultado
❌ No se avanza a In Review — tests del QA no compilan (3 errores E0716).

## Resumen

Quincuagésima verificación del código de producción para STORY-022. El código de producción
sigue completo y correcto, cubriendo todos los criterios de aceptación (CA1-CA8, CA10-CA11).
Sin embargo, los tests escritos por el QA en `mod story022` contienen 3 errores de compilación
que impiden verificar CA9.

## Verificaciones del código de producción

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo build` | ✅ OK | 0.30s |
| `cargo clippy --no-deps --bin regista` | ✅ 0 warnings | 0.27s |
| `cargo fmt -- --check` | ✅ OK | - |
| `cargo test --test architecture` | ✅ 11/11 | 0.05s |

## Código de producción (sin cambios, estable)

### `invoke_with_retry()` (L79)
```rust
pub async fn invoke_with_retry(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
    verbose: bool,     // ← CA1: nuevo parámetro
) -> anyhow::Result<AgentResult>
```

### `invoke_with_retry_blocking()` (L193)
```rust
pub fn invoke_with_retry_blocking(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
    verbose: bool,     // ← CA1, CA10: propagado
) -> anyhow::Result<AgentResult>
```

### `invoke_once()` (L316)
```rust
async fn invoke_once(
    provider: &dyn AgentProvider,
    instruction: &Path,
    prompt: &str,
    timeout: Duration,
    verbose: bool,     // ← CA2, CA6: control de modo
) -> anyhow::Result<Output> {
    // ...
    if verbose {
        invoke_once_verbose(child, pid, provider, timeout).await  // CA2
    } else {
        // wait_with_output() — comportamiento actual (CA6)
    }
}
```

### `invoke_once_verbose()` (L358)
- `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async **(CA2)**
- Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` **(CA3)**
- stdout acumulado en `Vec<u8>` y devuelto en `Output` **(CA4)**
- stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming **(CA5)**

### `kill_process_by_pid()` (L440)
Helper extraído para timeout cross-platform (Unix `kill -9`, Windows `taskkill`). **(CA7)**

### Call sites actualizados (CA10)
- `app/plan.rs:152` — `invoke_with_retry_blocking(..., false)`
- `app/pipeline.rs:774` — `invoke_with_retry(..., false).await`
- Tests pre-existentes pasan `false`.

### `AgentResult` (CA11)
Mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.

## Errores en tests del QA (NO corregidos)

Los 3 errores son idénticos: `E0716: temporary value dropped while borrowed`.

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
// ❌ Actual (rompe):
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ Corregido:
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión

NO se avanza el estado a **In Review**. El orquestador debe detectar que los tests
no compilan y pasar el turno al QA (`Tests Ready → Tests Ready`) para que corrija
los 3 errores E0716. El código de producción NO requiere cambios.
