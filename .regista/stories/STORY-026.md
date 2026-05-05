# STORY-026: Header de sesión con metadatos (versión, modelos, límites, git, hooks)

## Status
**Tests Ready**

## Epic
EPIC-09

## Descripción
Emitir un header de sesión al iniciar el daemon. En modo detallado (default), se emite un bloque multilínea con: versión de regista, timestamp UTC, directorio del proyecto, provider, modelos por rol, límites (max_iter, max_reject, timeout), estado de git, y hooks configurados. En modo compacto (`--compact`), el header se reduce a una línea. El header se emite en `setup_daemon_tracing()` o inmediatamente después, usando `tracing::info!`. La resolución de modelos usa `AgentsConfig::model_for_role()`.

## Criterios de aceptación
- [ ] CA1: En modo detallado (default), el header se emite con formato de bloque:
  ```
  ══════════════════════════════════════════════════════════════
  🛰️  regista vX.Y.Z — sesión iniciada YYYY-MM-DD HH:MM:SS UTC
     Proyecto   : /path/to/project
     Provider    : pi
     Modelos     : PO=desconocido, QA=desconocido, Dev=desconocido, Reviewer=desconocido
     Límites     : max_iter=N (M stories × 6), max_reject=8, timeout=1800s
     Git         : habilitado / deshabilitado
     Hooks       : (lista de hooks activos o "ninguno")
  ══════════════════════════════════════════════════════════════
  ```
- [ ] CA2: En modo compacto, el header es una línea: `🛰️  regista vX.Y.Z | <provider> | <fecha> UTC | max_iter=N`
- [ ] CA3: El campo "Modelos" muestra el modelo resuelto para cada rol usando `AgentsConfig::model_for_role()`
- [ ] CA4: El campo "Límites" muestra `max_iter` efectivo (calculado: `n_stories × 6` si `max_iterations=0`), `max_reject_cycles`, y `agent_timeout_seconds`
- [ ] CA5: El campo "Git" muestra "habilitado" si `git.enabled`, "deshabilitado" si no
- [ ] CA6: El campo "Hooks" lista los hooks con config no vacía: `post_qa, post_dev, post_reviewer` (o "ninguno" si no hay)
- [ ] CA7: El header se emite usando `tracing::info!` (para que aparezca tanto en consola como en daemon.log)
- [ ] CA8: `cargo build` compila sin errores
- [ ] CA9: `cargo test` pasa todos los tests existentes

## Dependencias
- Bloqueado por: STORY-019

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 2: Header de sesión).
- 2026-05-05 | QA | Tests unitarios verificados: 31 tests en src/cli/handlers.rs (mod story026) cubren los 7 CAs. Formato de bloque y compacto, modelos vía model_for_role() con YAML fallback, límites con max_iter efectivo y floor, git habilitado/deshabilitado, hooks parciales/totales/ninguno, emisión vía tracing::info!. Sin tests adicionales requeridos.