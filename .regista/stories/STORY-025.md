# STORY-025: Flag `--tail` en `RepoArgs` + integración en `handle_logs()`

## Status
**Blocked**

## Epic
EPIC-09

## Descripción
Añadir el flag `--tail` (bool, default false) a `RepoArgs` en `cli/args.rs`. Modificar `handle_logs()` en `cli/handlers.rs` para que propague este flag a `daemon::follow()`: cuando `--tail` está presente, `from_beginning = false` (comportamiento actual, solo nuevo); por defecto (sin `--tail`), `from_beginning = true` (vuelca todo + tail).

## Criterios de aceptación
- [ ] CA1: `RepoArgs` tiene campo `pub tail: bool` con `#[arg(long = "tail")]` y default `false`
- [ ] CA2: `handle_logs()` lee `args.repo.tail` y lo pasa como `from_beginning = !args.repo.tail` a `daemon::follow()`
- [ ] CA3: Por defecto: `regista logs` vuelca todo el historial y luego sigue en vivo (sin `seek(End)`)
- [ ] CA4: Con `--tail`: `regista logs --tail` solo muestra contenido nuevo (hace `seek(End)`, comportamiento actual)
- [ ] CA5: `cargo build` compila sin errores
- [ ] CA6: `cargo test` pasa todos los tests existentes

## Dependencias
- Bloqueado por: STORY-023

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 5: regista logs — comportamiento por defecto y --tail).