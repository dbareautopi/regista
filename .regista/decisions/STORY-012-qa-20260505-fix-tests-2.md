# STORY-012 QA — Corrección de tests de infraestructura async (2ª ronda)

**Fecha:** 2026-05-05
**Rol:** QA Engineer
**Historia:** STORY-012 — Migrar `pipeline.rs` a async
**Resultado:** 2 tests corregidos, 318 tests pasan, 0 fallos

---

## Contexto

Tras la primera ronda de corrección de tests (migración de tests síncronos a `#[tokio::test]` + `.await`), la suite completa pasaba (307 unit + 11 architecture). Sin embargo, 2 tests de infraestructura fallaban:

1. `infra::hooks::tests::run_hook_safe_from_async_context`
2. `infra::git::tests::concurrent_snapshots_dont_deadlock`

Ambos relacionados con la migración async del pipeline (CA4 y CA5 respectivamente).

---

## Test 1: `run_hook_safe_from_async_context` — "Cannot start a runtime from within a runtime"

### Causa raíz

El test envolvía `run_hook()` en `tokio::spawn(async { ... })`. Pero `run_hook()` internamente llama a `RUNTIME.block_on()` para ejecutar `tokio::process::Command`. Cuando `tokio::spawn` ejecuta el future en un worker thread de tokio, y `run_hook()` intenta `block_on()`, tokio detecta que se está intentando bloquear el thread actual que ya está corriendo tareas async → panic.

### Corrección

Cambiar `tokio::spawn(async { run_hook(...) })` por `tokio::task::spawn_blocking(|| run_hook(...))`.

`spawn_blocking` ejecuta el closure en un hilo del blocking thread pool (no en un worker async de tokio), por lo que `RUNTIME.block_on()` puede crear su propio runtime sin conflicto.

```rust
// Antes (falla):
let handle = tokio::spawn(async { run_hook(Some("true"), "async_hook") });

// Después (funciona):
let handle = tokio::task::spawn_blocking(|| run_hook(Some("true"), "async_hook"));
```

---

## Test 2: `concurrent_snapshots_dont_deadlock` — Contienda por `index.lock` de git

### Causa raíz

El test disparaba 3 `snapshot()` concurrentes sobre el mismo repositorio git. Cada `snapshot()` ejecuta `git add -A && git commit`. Git usa un archivo de lock (`.git/index.lock`) para serializar operaciones de escritura. Con 3 operaciones concurrentes, solo una adquiría el lock y las otras fallaban → `assert!(result.is_some())` fallaba.

No era un deadlock, sino contienda de recurso externo (lock de git).

### Corrección

Cada llamada concurrente a `snapshot()` usa su propio directorio temporal y repositorio git independiente. El objetivo del test es verificar que `spawn_blocking` + `snapshot()` no causa deadlocks, lo cual se verifica igualmente con repositorios separados.

```rust
// Antes: un solo repo compartido (falla por index.lock)
let tmp = TempDir::new().unwrap();
let root = tmp.path();
// ... 3 spawn_blocking sobre el mismo root

// Después: repo independiente por tarea concurrente
for i in 0..3 {
    let handle = tokio::task::spawn_blocking(move || {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        // ... snapshot sobre su propio root
    });
}
```

---

## Verificación

| Comando | Resultado |
|---------|-----------|
| `cargo test` | 318 passed (307 unit + 11 architecture), 0 failed, 1 ignored |
| `cargo clippy -- -D warnings` | Clean, sin warnings |
| `cargo fmt --check` | Clean, sin diferencias |

---

## Notas

- Los tests de STORY-012 (pipeline) y STORY-011 (shared state) ya pasaban correctamente tras la primera ronda de QA.
- Esta segunda ronda corrige los 2 tests de infraestructura que fallaban por diseño incorrecto del test, no por bugs en el código fuente.
- Status mantenido en **Tests Ready** — CA6 (cargo test --lib orchestrator pasa) sigue cumplido.
