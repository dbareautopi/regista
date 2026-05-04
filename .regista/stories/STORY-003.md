# STORY-003: Validar `epics_dir` y separar side-effects de `Config::validate()`

## Status
**Draft**

## Epic
EPIC-01

## Descripción
Actualmente `Config::validate()` solo verifica que `stories_dir` existe, pero no valida `epics_dir`. Si el usuario configura un `epics_dir` que no existe, el pipeline falla en runtime cuando `plan.rs` intenta escribir épicas. Además, la función `validate()` tiene efectos secundarios (crea `decisions_dir` y `log_dir` con `create_dir_all`), lo cual viola el principio de que una validación debe ser solo-lectura. La creación de directorios debe moverse al orchestrator o a `init`.

## Criterios de aceptación
- [ ] CA1: `Config::validate()` verifica que `epics_dir` existe (si está configurado) y reporta error si no
- [ ] CA2: `Config::validate()` NO crea directorios (`create_dir_all` se elimina de `validate`)
- [ ] CA3: La creación de `decisions_dir` y `log_dir` se mueve a `orchestrator.rs` (al inicio de `run_real()`) o a `init.rs`
- [ ] CA4: `Config::validate()` sigue verificando que `stories_dir` existe (comportamiento existente)
- [ ] CA5: `cargo test --lib config` pasa
- [ ] CA6: `cargo test` pasa todos los tests existentes (los tests que asumían creación de directorios en validate deben adaptarse)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #8.1, #8.2).
