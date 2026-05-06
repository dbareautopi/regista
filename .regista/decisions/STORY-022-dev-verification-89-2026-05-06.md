# STORY-022 — Dev — 2026-05-06 (verificación #89)

## Resultado
❌ No se avanza a In Review — tests del QA no compilan

## Verificación del código de producción

| Check | Resultado |
|-------|-----------|
| `cargo check --bin regista` | ✅ OK (0.20s) |
| `cargo build` | ✅ OK (0.36s) |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings (0.40s) |
| `cargo fmt -- --check` | ✅ OK |
| `cargo test --test architecture` | ✅ 11/11 pasan (0.38s) |
| `cargo test -- story022` | ❌ NO compila |

## Código de producción — estado CA1-CA8, CA10-CA11

Todos los criterios de aceptación de implementación están cubiertos:

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_with_retry()` acepta `verbose: bool` | ✅ L84 |
| CA2 | `invoke_once()` verbose → BufReader + read_line | ✅ L316-358 |
| CA3 | Líneas stdout logueadas con `  │ ` | ✅ |
| CA4 | stdout acumulado en Vec<u8> | ✅ |
| CA5 | stderr en tokio::spawn, sin streaming | ✅ |
| CA6 | verbose=false → wait_with_output() | ✅ |
| CA7 | Timeout funciona en ambos modos | ✅ |
| CA8 | cargo check compila | ✅ |
| CA10 | Call sites actualizados | ✅ plan.rs + pipeline.rs |
| CA11 | AgentResult: stdout, stderr, exit_code | ✅ |

## Errores en tests del QA (NO corregidos)

Los 3 errores E0716 ocurren por la misma causa: `MutexGuard` temporal se destruye
antes de que `Cow<str>` deje de tomar prestado su contenido.

| # | Test | Línea | Código problemático |
|---|------|-------|---------------------|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 2 | `ca3_empty_lines_not_logged` | 1809 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | `String::from_utf8_lossy(&buffer.lock().unwrap())` |

### Causa raíz

```rust
// INCORRECTO: buffer.lock().unwrap() crea un MutexGuard temporal.
// Cuando la sentencia termina, el MutexGuard se destruye, pero
// String::from_utf8_lossy devuelve Cow<str> que toma prestado del
// Vec<u8> dentro del MutexGuard → use-after-free → E0716.
let log_output = String::from_utf8_lossy(&buffer.lock().unwrap());
```

### Solución (responsabilidad del QA)

```rust
// CORRECTO: el MutexGuard vive en la variable `binding`,
// por lo que el préstamo es válido mientras se usa `log_output`.
let binding = buffer.lock().unwrap();
let log_output = String::from_utf8_lossy(&binding);
```

## Acción siguiente

El orquestador debe pasar el turno al QA para que corrija los 3 errores E0716.
Una vez corregidos, se verificará CA9 (`cargo test -- story022`).
