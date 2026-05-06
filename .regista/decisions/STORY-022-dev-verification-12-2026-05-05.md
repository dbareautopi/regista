# STORY-022: Decisión de Dev — 12ª verificación

**Fecha**: 2026-05-05
**Actor**: Developer
**Historia**: STORY-022
**Estado actual**: Tests Ready (NO se avanza a In Review)

---

## Verificación del código de producción

| Verificación | Resultado |
|---|---|
| `cargo check` | ✅ OK (0.38s) |
| `cargo build` | ✅ OK (0.34s) |
| `cargo clippy --no-deps` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test -- story022` | ❌ NO compila (3 errores E0716) |

## Implementación (código de producción)

La implementación está completa y cubre todos los CAs que no dependen de los tests del QA:

### Cambios en `Cargo.toml`
- Feature `io-util` añadido a tokio: `tokio = { version = "1", features = [..., "io-util"] }`

### Cambios en `src/infra/agent.rs`

#### `invoke_with_retry()` — nuevo parámetro `verbose: bool` (CA1)
```rust
pub async fn invoke_with_retry(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
    verbose: bool,  // ← NUEVO
) -> anyhow::Result<AgentResult>
```

#### `invoke_with_retry_blocking()` — nuevo parámetro `verbose: bool` (CA1, CA10)
```rust
pub fn invoke_with_retry_blocking(
    ...
    verbose: bool,  // ← NUEVO
) -> anyhow::Result<AgentResult>
```

#### `invoke_once()` — nuevo parámetro `verbose: bool`, routing a modo verbose (CA2, CA6)
```rust
async fn invoke_once(
    ...
    verbose: bool,  // ← NUEVO
) -> anyhow::Result<Output> {
    ...
    if verbose {
        invoke_once_verbose(child, pid, provider, timeout).await
    } else {
        // wait_with_output() — comportamiento actual
    }
}
```

#### `invoke_once_verbose()` — nueva función (CA2, CA3, CA4, CA5)
- `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle async (CA2)
- Cada línea no vacía: `tracing::info!("  │ {}", trimmed)` (CA3)
- stdout acumulado en `Vec<u8>` y devuelto (CA4)
- stderr en `tokio::spawn` separado con `read_to_end()`, sin streaming (CA5)
- Timeout via `tokio::time::timeout` + `kill_process_by_pid()` (CA7)

#### `kill_process_by_pid()` — helper extraído (CA7)
- Cross-platform: `kill -9` en Unix, `taskkill /PID /F` en Windows
- Usado tanto en modo verbose como en `wait_with_output()`

### Call sites actualizados (CA10)
- `src/app/plan.rs:152`: `invoke_with_retry_blocking(..., false)`
- `src/app/pipeline.rs:774`: `invoke_with_retry(..., false).await`

### AgentResult sin cambios (CA11)
```rust
pub struct AgentResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
    pub attempt: u32,
    pub attempts: Vec<AttemptTrace>,
}
```

---

## Errores en tests del QA (NO corregidos)

Los 3 errores E0716 (`temporary value dropped while borrowed`) persisten:

| # | Test | Línea | Código problemático |
|---|---|---|---|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa
`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal que se destruye al final del statement. El `Cow<str>` devuelto por `String::from_utf8_lossy` referencia los datos del `MutexGuard`, pero este ya no existe → E0716.

### Solución (responsabilidad del QA)
En las 3 ubicaciones, reemplazar:
```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```
por:
```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

### Impacto
- CA9 bloqueado: no se puede ejecutar `cargo test` hasta que los tests compilen.
- La historia no puede avanzar a In Review.
- Es la 12ª iteración en que el Dev encuentra el mismo error sin corrección del QA.

---

## Decisión

**NO se avanza a In Review.** El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716 antes de que el Dev pueda verificar CA9.
