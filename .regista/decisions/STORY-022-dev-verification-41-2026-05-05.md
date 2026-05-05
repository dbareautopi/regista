# STORY-022 — dev — 2026-05-05 (verificación #41)

## Resultado
❌ No se avanza a In Review — los tests del QA no compilan.

## Verificaciones del código de producción

| Comando | Resultado |
|---------|-----------|
| `cargo build` | ✅ OK (0.17s), binario generado |
| `cargo test -- story022` | ❌ NO compila — 3 errores E0716 |

## Código de producción (completo y correcto)

El código de producción en `src/infra/agent.rs` implementa correctamente todos los
criterios de aceptación implementables (CA1-CA8, CA10-CA11):

- **CA1**: `invoke_with_retry()` (L78) y `invoke_with_retry_blocking()` (L200) aceptan
  `verbose: bool` como último parámetro.
- **CA2**: `invoke_once()` (L311) con parámetro `verbose: bool`. Cuando `verbose=true`,
  delega en `invoke_once_verbose()` que usa `child.stdout.take()` + `BufReader::new()` +
  `read_line()` en bucle async para streaming línea a línea.
- **CA3**: Cada línea no vacía de stdout se loguea con `tracing::info!("  │ {}", trimmed)`.
- **CA4**: El stdout completo se acumula en un `Vec<u8>` y se devuelve como parte
  del `Output` (y del `AgentResult`).
- **CA5**: stderr se lee en una tarea `tokio::spawn` separada con `read_to_end()`,
  sin streaming al log.
- **CA6**: `verbose=false` usa `wait_with_output()` (comportamiento actual, eficiente).
- **CA7**: El timeout funciona en ambos modos mediante `kill_process_by_pid()` (L440),
  helper cross-platform extraído para reutilización.
- **CA8**: `cargo build` compila sin errores.
- **CA10**: Call sites en `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false`.
- **CA11**: `AgentResult` mantiene `stdout: String`, `stderr: String`, `exit_code: i32`.
- `Cargo.toml`: feature `io-util` añadido a tokio.

## Errores en los tests del QA (NO corregidos)

Los tests del QA en `mod story022` de `src/infra/agent.rs` tienen 3 errores de
compilación `E0716` (temporary value dropped while borrowed):

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz

`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal.  
`String::from_utf8_lossy(&[u8])` devuelve un `Cow<'_, str>`, que en el caso de
UTF-8 válido es `Cow::Borrowed(&str)` y **toma prestado del slice original**.
El `MutexGuard` temporal se destruye al final del statement, pero el `Cow<str>`
aún lo referencia → E0716.

### Solución exacta (responsabilidad del QA)

Reemplazar en las 3 ubicaciones:

```rust
// ❌ Actual (no compila)
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// ✅ Corrección
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

El `let binding` extiende la vida del `MutexGuard` hasta que `log_output` deja
de usarse.

## CA9 bloqueado

`cargo test -- story022` no puede ejecutarse hasta que el QA corrija los 3 errores
de compilación. Los tests individualmente parecen bien diseñados — solo necesitan
el ajuste de lifetime descrito arriba.

## Decisión

**NO se avanza a In Review.** El orquestador debe pasar el turno al QA para que
corrija los 3 errores E0716 en sus tests.
