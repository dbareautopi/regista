# STORY-010 — Dev — Verificación post-corrección QA (2026-05-05)

## Resultado
✅ Éxito — historia avanzada de Tests Ready → In Review

## Contexto

Tras la iteración anterior (donde el Dev implementó la migración a tokio pero encontró un bug en el test `save_agent_decision_creates_file_with_content`), el QA corrigió los tests y volvió a poner la historia en Tests Ready.

Esta iteración verifica que todo funciona correctamente sobre los tests corregidos y avanza la historia.

## Verificaciones realizadas

### Build
```
cargo build → OK (0 errores, 0 warnings)
```

### Clippy
```
cargo clippy -- -D warnings → OK (0 warnings)
```

### Formato
```
cargo fmt --check → OK (sin cambios pendientes)
```

### Tests
```
cargo test → 290 passed, 0 failed, 1 ignored
```

Tests específicos de agent.rs (14 tests):
- 3 tests sync (funciones puras): `build_feedback_prompt_includes_error`, `build_feedback_prompt_truncates_long_output`, `agent_options_default`
- 10 tests async (#[tokio::test]): `invoke_once_is_async`, `invoke_once_respects_timeout`, `invoke_with_retry_is_async`, `invoke_with_retry_preserves_retry_count`, `invoke_with_retry_backoff_doubles_delay`, `save_agent_decision_creates_file_with_content`, `save_agent_decision_noops_when_story_id_is_none`, `save_agent_decision_noops_when_decisions_dir_is_none`
- 1 test compile-time: `tokio_is_a_dependency`
- 1 test ignorado (`invoke_with_retry_fails_when_agent_not_installed` — requiere pi instalado)

### Arquitectura
```
cargo test --test architecture → 11 passed, 0 failed
```

## Estado de los CAs

| CA | Descripción | Estado |
|----|-------------|--------|
| CA1 | `invoke_once()` async con `tokio::process::Command` | ✅ |
| CA2 | `tokio::time::timeout` en lugar de `thread::sleep` + `try_wait` | ✅ |
| CA3 | `invoke_with_retry()` async | ✅ |
| CA4 | Backoff exponencial con `tokio::time::sleep` | ✅ |
| CA5 | `save_agent_decision()` con `tokio::fs::write` | ✅ |
| CA6 | `tokio` en `Cargo.toml` | ✅ |
| CA7 | Tests adaptados a async (`#[tokio::test]`) | ✅ |
| CA8 | Tests pasan | ✅ (14/14) |

## Cambios en el código

La implementación ya estaba presente de la iteración anterior:

1. **`Cargo.toml`**: `tokio = { version = "1", features = ["rt-multi-thread", "macros", "process", "time", "fs"] }`
2. **`invoke_once()`** (línea ~197): `async fn` usando `tokio::process::Command::new()` + `.spawn()` + `tokio::time::timeout(duration, child.wait_with_output()).await`. En timeout, mata el proceso por PID vía `kill -9` (Unix) o `taskkill` (Windows).
3. **`invoke_with_retry()`** (línea ~56): `async fn` con `tokio::time::sleep(delay).await` y backoff exponencial `delay *= 2`.
4. **`save_agent_decision()`** (línea ~163): `async fn` usando `tokio::fs::create_dir_all()` + `tokio::fs::write()`.
5. **`invoke_with_retry_blocking()`** (línea ~144): wrapper síncrono que usa `LazyLock<tokio::runtime::Runtime>` + `.block_on()`.
6. **Callers** (`plan.rs`, `pipeline.rs`): usan `invoke_with_retry_blocking()`.

## Decisión

Historia lista para revisión. Todos los CAs satisfechos. Sin deuda técnica. Sin cambios pendientes.

Avanzada a **In Review**.
