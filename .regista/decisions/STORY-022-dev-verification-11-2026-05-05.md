# STORY-022-dev-verification-11-2026-05-05

## Resultado
❌ Tests del QA no compilan — NO se avanza a In Review

## Verificación del código de producción

### Compilación y calidad
| Comando | Resultado |
|---------|-----------|
| `cargo check` | ✅ OK (0.15s, sin errores) |
| `cargo build` | ✅ OK (0.14s, binario generado) |
| `cargo clippy --no-deps` | ✅ OK (0.22s, 0 warnings) |
| `cargo fmt -- --check` | ✅ OK (código formateado) |

### Cobertura de criterios de aceptación (código de producción)

| CA | Descripción | Estado | Evidencia |
|----|-------------|--------|-----------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ | Línea 78: `verbose: bool` como último parámetro |
| CA2 | `verbose=true` → `BufReader::new()` + `read_line()` async | ✅ | `invoke_once_verbose()` en L358: `BufReader::new(stdout)` + loop `read_line()` |
| CA3 | `tracing::info!("  │ {}", trimmed)` por línea | ✅ | L395: `tracing::info!("  │ {}", trimmed)` para líneas no vacías |
| CA4 | stdout acumulado en `Vec<u8>`, devuelto en `Output` | ✅ | L390: `accumulated.extend_from_slice(line.as_bytes())` + `Ok(Output { stdout, ... })` |
| CA5 | stderr en `tokio::spawn` sin streaming | ✅ | L406-418: `tokio::spawn` + `read_to_end()` silencioso |
| CA6 | `verbose=false` → `wait_with_output()` | ✅ | L325-345: rama else usa `child.wait_with_output()` |
| CA7 | timeout funciona en ambos modos | ✅ | `kill_process_by_pid()` L440-460, usado en ambas ramas |
| CA8 | `cargo check` compila | ✅ | Verificado (ver tabla arriba) |
| CA9 | `cargo test --lib infra::agent` pasa | ❌ | Bloqueado por 3 errores E0716 en `mod story022` |
| CA10 | Call sites actualizados | ✅ | `app/plan.rs:152` → `false`, `app/pipeline.rs:774` → `false` |
| CA11 | `AgentResult` mantiene `stdout`, `stderr`, `exit_code` | ✅ | Struct AgentResult L37-47 sin cambios |

### Dependencias
- `Cargo.toml`: tokio incluye feature `io-util` ✅

## Errores en tests del QA (NO corregidos)

Los 3 errores `E0716` (temporary value dropped while borrowed) están en el módulo `mod story022`:

### Error 1 — Línea 1763
- **Test**: `ca3_verbose_logs_lines_with_pipe_prefix`
- **Código**: `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());`
- **Problema**: El `MutexGuard` temporal de `buffer.lock().unwrap()` se destruye al final del statement, mientras el `Cow<str>` devuelto por `from_utf8_lossy` aún lo referencia (usado en asertos posteriores).

### Error 2 — Línea 1809
- **Test**: `ca3_empty_lines_not_logged`
- **Código**: `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());`
- **Problema**: Idéntico al Error 1.

### Error 3 — Línea 2006
- **Test**: `ca5_stderr_not_streamed_to_log`
- **Código**: `let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());`
- **Problema**: Idéntico al Error 1.

### Solución requerida (responsabilidad del QA)
Reemplazar en las 3 líneas:

```rust
// ANTES (rompe):
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());

// DESPUÉS (corrige):
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Decisión
- NO se avanza el estado a `In Review`.
- El código de producción está completo y correcto.
- Los 3 errores `E0716` son responsabilidad del QA (errores en el código de test, no en producción).
- El orquestador debe pasar el turno al QA para que corrija los tests.
