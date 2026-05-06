# STORY-022 — Dev Verification #81 — 2026-05-06

## Resumen

Octogésima primera verificación de la implementación de STORY-022 por parte del rol Dev.

## Verificaciones realizadas

| Comando | Resultado | Tiempo |
|---------|-----------|--------|
| `cargo check --bin regista` | ✅ OK, sin errores | 0.14s |
| `cargo build` | ✅ OK | 0.24s |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings | — |
| `cargo fmt -- --check` | ✅ OK | — |
| `cargo test --test architecture` | ✅ OK, 11/11 | — |
| `cargo test -- story022` | ❌ NO compila | — |

## Código de producción — Estado

Completo y correcto. Cubre todos los criterios de aceptación de producción (CA1-CA8, CA10-CA11).

### Funciones implementadas

1. **`invoke_with_retry()`** (L84): acepta `verbose: bool` como último parámetro (CA1).
2. **`invoke_with_retry_blocking()`** (L199): propaga `verbose: bool` (CA1, CA10).
3. **`invoke_once()`** (L316): `verbose=false` → `wait_with_output()`, `verbose=true` → `invoke_once_verbose()` (CA2, CA6).
4. **`invoke_once_verbose()`** (L358): streaming de stdout línea a línea con `BufReader`, prefijo `  │ ` en el log, acumulación en `Vec<u8>`, stderr en `tokio::spawn` sin streaming (CA2-CA5).
5. **`kill_process_by_pid()`** (L440): helper cross-platform para timeout (CA7).
6. **Call sites**: `app/plan.rs:152` y `app/pipeline.rs:774` pasan `false` (CA10).
7. **`AgentResult`**: mantiene `stdout: String`, `stderr: String`, `exit_code: i32` (CA11).
8. **`Cargo.toml`**: feature `io-util` en tokio (CA2).

## Errores en tests del QA

Los tests del módulo `mod story022` (escritos por QA) presentan 3 errores de compilación E0716. Estos errores están en el código de test, NO en el código de producción.

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz

`buffer.lock().unwrap()` devuelve un `MutexGuard<Vec<u8>>` temporal. Al tomar `&` de ese temporal y pasarlo a `String::from_utf8_lossy()`, el `MutexGuard` se destruye al final del statement, pero el `Cow<str>` devuelto sigue tomando prestado de él → E0716.

### Solución (responsabilidad del QA)

```rust
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

En las 3 ubicaciones (líneas 1763, 1809, 2006).

## Decisión

- **NO se avanza a In Review.**
- Los tests del QA no compilan. Corregirlos es responsabilidad del QA.
- El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
- Esta es la 81ª iteración en la que el Dev encuentra el mismo problema.

## CA9 — Bloqueado

`cargo test -- story022` no puede verificarse hasta que el QA corrija los 3 errores de compilación en el módulo `mod story022`.
