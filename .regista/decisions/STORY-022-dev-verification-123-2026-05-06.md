# STORY-022 — 123ª verificación del Dev — 2026-05-06

## Resumen

Re-verificación del estado de STORY-022 (streaming de stdout en `invoke_once()` + parámetro `verbose`).

## Resultado

### ✅ Código de producción: COMPLETO Y CORRECTO

| Verificación | Resultado |
|---|---|
| `cargo check --bin regista` | ✅ OK, 0 errores |
| `cargo clippy --no-deps --bin regista` | ✅ OK, 0 warnings |
| `cargo fmt -- --check` | ✅ OK, código formateado |
| `cargo test --test architecture` | ✅ OK, 11/11 pasan |

### ❌ Tests del QA (mod story022): NO COMPILAN

`cargo test -- story022` produce 3 errores E0716 idénticos:

| # | Test | Línea | Error |
|---|---|---|---|
| 1 | `ca3_verbose_logs_lines_with_pipe_prefix` | 1763 | `String::from_utf8_lossy(&buffer.lock().unwrap())` — MutexGuard temporal destruido |
| 2 | `ca3_empty_lines_not_logged` | 1809 | Mismo error E0716 |
| 3 | `ca5_stderr_not_streamed_to_log` | 2006 | Mismo error E0716 |

### Causa

`String::from_utf8_lossy(&buffer.lock().unwrap())` crea un `MutexGuard` temporal que se destruye al final del statement, antes de que el `Cow<str>` resultante sea usado. Rust no permite referencias a temporales que ya no existen.

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

### CA9 bloqueado

`cargo test -- story022` no puede ejecutarse hasta que el QA corrija los 3 errores de compilación.

## Estado

**NO se avanza a In Review.** El bloqueo está en los tests del QA, no en la implementación.
El orquestador debe pasar el turno al QA.
