# STORY-022 — QA Test Review — 2026-05-05

## Resumen

Revisión de tests unitarios existentes en `mod story022` de `src/infra/agent.rs`.
Verificación de cobertura contra los 11 criterios de aceptación (CAs).

**Resultado**: ✅ Los 25 tests existentes cubren todos los CAs testeables como tests unitarios.

---

## Matriz de cobertura

| CA | Tests | Descripción | Tipo |
|----|-------|-------------|------|
| **CA1** | 3 | `invoke_with_retry` acepta `verbose: bool` | Firma/compilación |
| **CA2** | 3 | Streaming línea a línea con `BufReader` + `read_line()` | Comportamiento |
| **CA3** | 2 | Logueo con prefijo `  │ `, sin líneas vacías | Comportamiento |
| **CA4** | 2 | stdout acumulado en `Vec<u8>` y en `AgentResult` | Comportamiento |
| **CA5** | 4 | stderr en `tokio::spawn`, capturado pero no stremeado | Comportamiento |
| **CA6** | 2 | `wait_with_output()` en modo no-verbose, mismo stdout | Comportamiento |
| **CA7** | 3 | Timeout en verbose, no-verbose, y caso sin timeout | Comportamiento |
| **CA8** | 0 | `cargo check --lib` | Verificación Developer |
| **CA9** | 0 | `cargo test --lib infra::agent` | Verificación Developer |
| **CA10** | 1 | Firma de call sites en pipeline | Firma/compilación |
| **CA11** | 5 | `AgentResult` tiene `stdout` (String), `stderr` (String), `exit_code` (i32) | Estructura |

**Total: 25 tests para 9 CAs testeables (CA8 y CA9 son verificaciones del Developer).**

---

## Detalle de tests por CA

### CA1 — Parámetro `verbose: bool`
- `ca1_invoke_with_retry_accepts_verbose_false` — async, `EchoProvider`, verifica stdout
- `ca1_invoke_with_retry_accepts_verbose_true` — async, `EchoProvider`, verifica stdout
- `ca1_invoke_with_retry_blocking_accepts_verbose` — sync wrapper, `EchoProvider`

### CA2 — Streaming con `BufReader`
- `ca2_verbose_true_handles_multiline_output` — `PrintfProvider`, 3 líneas con `\n`
- `ca2_verbose_true_handles_single_line` — `EchoProvider`, una línea
- `ca2_verbose_true_handles_empty_output` — `ShProvider` con `true`, stdout vacío

### CA3 — Logueo con prefijo `  │ `
- `ca3_verbose_logs_lines_with_pipe_prefix` — `tracing_subscriber` + `CaptureWriter`, verifica prefijo y contenido
- `ca3_empty_lines_not_logged` — `PrintfProvider` con línea vacía (`\n\n`), verifica que no se loguean vacíos

### CA4 — stdout acumulado
- `ca4_verbose_accumulates_full_stdout_in_output` — `PrintfProvider`, verifica `output.stdout` (Vec<u8>)
- `ca4_agent_result_contains_stdout_verbose_true` — `EchoProvider` vía `invoke_with_retry`, verifica `AgentResult.stdout`

### CA5 — stderr en tarea separada
- `ca5_stderr_captured_in_verbose_mode` — `ShProvider` + `echo >&2`, captura stderr
- `ca5_stderr_captured_in_non_verbose_mode` — mismo pero con `verbose=false`
- `ca5_stderr_empty_when_no_stderr_output` — `EchoProvider`, stderr vacío
- `ca5_stderr_not_streamed_to_log` — `tracing_subscriber` + verifica que stderr NO tiene prefijo `  │ `

### CA6 — `wait_with_output()` en no-verbose
- `ca6_non_verbose_works_correctly` — `EchoProvider` con `verbose=false`
- `ca6_both_modes_produce_same_stdout` — comparación exacta verbose vs no-verbose

### CA7 — Timeout
- `ca7_timeout_in_verbose_mode` — `SleepProvider 10s` + timeout 100ms, verifica error y rapidez
- `ca7_timeout_in_non_verbose_mode` — mismo con `verbose=false`
- `ca7_no_timeout_completes_in_verbose_mode` — `SleepProvider 0.1s` + timeout 5s, verifica éxito

### CA10 — Call sites
- `ca10_call_signature_matches_pipeline_usage` — `PiProvider` con firma idéntica a `pipeline.rs`

### CA11 — AgentResult
- `ca11_agent_result_has_stdout_stderr_exit_code` — valores concretos
- `ca11_agent_result_stdout_is_owned_string` — `.lines()` accesible
- `ca11_agent_result_stderr_is_string` — con y sin stderr
- `ca11_agent_result_exit_code_is_i32` — códigos negativos y cero
- `ca11_agent_result_all_fields_publicly_accessible` — todos los campos

---

## Decisiones de diseño de tests

### D1: Providers de prueba mínimos (no mocks)
Los tests usan `EchoProvider`, `PrintfProvider`, `ShProvider`, y `SleepProvider`.  
Son implementaciones reales del trait `AgentProvider` que ejecutan binarios del sistema.  
**Ventajas**: no requieren `mockall`, no falsean el comportamiento del pipe real, verifican el camino completo (`spawn` → `stdout.take()` → `read_line()`).  
**Cumplen la regla "no fake providers"**: ejecutan procesos reales del SO.

### D2: Captura de logs con `tracing_subscriber`
CA3 y CA5 usan `tracing_subscriber::fmt().with_writer(CaptureWriter)` para verificar la salida de `tracing::info!()`.  
El `CaptureWriter` es un `Arc<Mutex<Vec<u8>>>` que acumula los bytes.  
**Alternativa considerada**: `tracing-test` crate. Descartado para evitar dependencia externa adicional.

### D3: Firmas forward-looking
Los tests llaman a `invoke_once()` e `invoke_with_retry()` con el parámetro `verbose` que aún no existe en las funciones reales.  
**Propósito**: definir el contrato/API que el Developer debe implementar.  
Los tests **no compilarán** hasta que el Developer añada el parámetro. Esto es intencional — sirve como guía de implementación.

### D4: Timeout con `SleepProvider`
Usa `sleep N` para forzar timeouts. El test verifica que:
1. El error contiene "timeout"
2. El tiempo transcurrido es razonable (< 5s para un timeout de 100ms)
Esto valida que no hay busy-polling ni bloqueos.

### D5: CA8 y CA9 no son unit tests
- CA8 (`cargo check --lib`) es una verificación de compilación.
- CA9 (`cargo test --lib infra::agent`) es una verificación de regresión.
Ambos son responsabilidad del Developer durante el ciclo de implementación.  
No tiene sentido escribirlos como tests unitarios dentro del crate.

---

## Notas para el Developer

1. **Añadir `verbose: bool` a `invoke_once()`**: 5º parámetro después de `timeout`.
2. **Añadir `verbose: bool` a `invoke_with_retry()`**: 6º parámetro después de `opts`.
3. **Añadir `verbose: bool` a `invoke_with_retry_blocking()`**: 6º parámetro, propagar a la versión async.
4. **Actualizar call sites**:
   - `src/app/pipeline.rs:774` — `process_story()` → pasar `false` inicialmente.
   - `src/app/plan.rs:152` — `handle_plan()` → pasar `false`.
   - `src/infra/agent.rs:191` — wrapper interno → propagar el parámetro.
5. **Modo verbose**: cuando `true`, usar `child.stdout.take()` + `BufReader::new()` + `read_line()` en bucle. Log con `tracing::info!("  │ {}", trimmed)`. Acumular en `Vec<u8>`.
6. **Modo no-verbose**: usar `child.wait_with_output()` (comportamiento actual, sin cambios).
7. **stderr**: leer en `tokio::spawn` separado, acumular en `Vec<u8>`, NO stremeae al log.
8. **Ejecutar `cargo check --lib` y `cargo test --lib infra::agent`** para validar CA8 y CA9.
