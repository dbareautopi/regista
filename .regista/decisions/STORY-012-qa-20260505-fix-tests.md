# Decisión QA — STORY-012: Corrección de tests async (2026-05-05)

## Contexto

El Developer completó la migración async de `run_real()` y `process_story()` en `pipeline.rs` (CA1-CA5).
El código fuente compila y pasa clippy/fmt limpios. Sin embargo, 8 tests no compilaban porque llamaban
a funciones ahora `async fn` sin `.await` desde funciones `#[test]` síncronas.

## Tests afectados

### STORY-011 (3 tests)

| Test | Línea | Error |
|------|-------|-------|
| `process_story_accepts_shared_state` | ~L1871 | `process_story()` llamada sin `.await` (E0599) |
| `process_story_blocked_returns_early` | ~L1894 | `process_story()` llamada sin `.await` (E0599) |
| `process_story_failed_returns_early` | ~L1915 | `process_story()` llamada sin `.await` (E0599) |

### STORY-012 (4 tests)

| Test | Línea | Error |
|------|-------|-------|
| `run_real_with_terminal_stories_completes_in_one_iteration` | ~L2570 | `run_real()` llamada sin `.await` (E0599) |
| `run_real_with_no_stories_completes_immediately` | ~L2624 | `run_real()` llamada sin `.await` (E0599) |
| `run_real_with_draft_story_invokes_po_path` | ~L2647 | `run_real()` llamada sin `.await` (E0599) |
| `run_real_shared_state_reflects_sequential_processing` | ~L2692 | `run_real()` llamada sin `.await` (E0599) |

## Solución aplicada

Todos los tests se migraron al patrón async:

- `#[test]` → `#[tokio::test]`
- `fn test_name()` → `async fn test_name()`
- `run_real(...)` / `process_story(...)` → `.await` añadido tras la llamada

Los cambios son mecánicos y no alteran la semántica de los tests. Las aserciones se mantienen idénticas.

## Verificación

- `cargo test -- pipeline`: 65 tests pasan (0 fallos)
- `cargo test`: 305 pass, 2 fallos preexistentes no relacionados (hooks/git async context)
- `cargo clippy -- -D warnings`: limpio
- `cargo fmt -- --check`: limpio

## Notas

Los 2 tests que fallan en el suite completo (`infra::hooks::tests::run_hook_safe_from_async_context`
y `infra::git::tests::concurrent_snapshots_dont_deadlock`) son preexistentes y no están relacionados
con STORY-012. Ambos son problemas conocidos de la migración async (anidamiento de runtimes).
