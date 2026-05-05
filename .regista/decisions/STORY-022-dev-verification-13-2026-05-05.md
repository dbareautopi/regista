# STORY-022 — Dev — Verification 13 — 2026-05-05

## Resultado
❌ Fallo parcial — tests del QA no compilan

## Verificación de código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK (0.17s) |
| `cargo build` | ✅ OK (0.14s) |
| `cargo clippy --no-deps` | ✅ OK (0 warnings) |
| `cargo fmt -- --check` | ✅ OK |

## Cobertura de CAs por el código de producción

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` como último argumento | ✅ |
| CA2 | `verbose=true` → `BufReader::new()` + `read_line()` en bucle async | ✅ |
| CA3 | `tracing::info!("  │ {}", trimmed)` para líneas no vacías | ✅ |
| CA4 | stdout completo acumulado en `Vec<u8>` y devuelto en `Output` | ✅ |
| CA5 | stderr en `tokio::spawn` separado, sin streaming | ✅ |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ |
| CA7 | timeout funciona en ambos modos vía `kill_process_by_pid()` | ✅ |
| CA8 | `cargo check --lib` compila | ✅ |
| CA9 | `cargo test --lib infra::agent` pasa | ❌ Bloqueado por tests |
| CA10 | Call sites actualizados con `verbose` | ✅ |
| CA11 | `AgentResult` contiene `stdout`, `stderr`, `exit_code` | ✅ |

## Errores en tests del QA (`mod story022`)

Los 3 errores son idénticos en naturaleza: `E0716` — `temporary value dropped while borrowed`.

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

El `MutexGuard` devuelto por `buffer.lock().unwrap()` se destruye al final del statement, 
pero el `Cow<str>` devuelto por `String::from_utf8_lossy` aún lo referencia.

### Ubicaciones exactas

| # | Test | Línea |
|---|------|-------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 |
| 2 | `ca3_empty_lines_not_logged` | 1809 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 |

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 líneas:

```rust
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

Por:

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Detalles de la implementación

### `invoke_once()` (L316)
```rust
async fn invoke_once(
    provider: &dyn AgentProvider,
    instruction: &Path,
    prompt: &str,
    timeout: Duration,
    verbose: bool,
) -> anyhow::Result<Output>
```
- `verbose=false` → `child.wait_with_output()` con `tokio::time::timeout` 
- `verbose=true` → delega en `invoke_once_verbose()`

### `invoke_once_verbose()` (L358)
```rust
async fn invoke_once_verbose(
    mut child: tokio::process::Child,
    pid: Option<u32>,
    provider: &dyn AgentProvider,
    timeout: Duration,
) -> anyhow::Result<Output>
```
- `child.stdout.take()` → `BufReader::new()` → bucle `read_line()`
- Cada línea no vacía: `tracing::info!("  │ {}", trimmed)`
- stdout acumulado en `Vec<u8>`
- stderr: `tokio::spawn` con `read_to_end()`, sin streaming
- Timeout: `kill_process_by_pid(pid)`

### `invoke_with_retry()` (L78)
```rust
pub async fn invoke_with_retry(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
    verbose: bool,   // ← último parámetro
) -> anyhow::Result<AgentResult>
```

### `invoke_with_retry_blocking()` (L193)
```rust
pub fn invoke_with_retry_blocking(
    ...
    verbose: bool,
) -> anyhow::Result<AgentResult>
```

### Call sites
- `app/plan.rs:152`: `false` (no necesita streaming)
- `app/pipeline.rs:774`: `false` (no necesita streaming)

### Dependencias
- `Cargo.toml`: feature `io-util` añadido a tokio

## Decisión
**NO se avanza a In Review.** Los 3 errores E0716 son responsabilidad del QA.
El orquestador debe pasar el turno al QA automáticamente.
