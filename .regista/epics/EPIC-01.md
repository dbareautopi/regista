# EPIC-01: Robustez de Providers y Configuración

## Descripción
Eliminar los `panic!` del sistema de providers, añadir validación temprana de binarios, validar `epics_dir` en la configuración, separar efectos secundarios de la validación, y reubicar funciones huérfanas para eliminar acoplamiento `infra → config`.

Cubre los hallazgos #7, #8 y #11.2 de la auditoría.

## Historias
- STORY-001: `from_name()` devuelve `Result` + `validate` verifica binarios
- STORY-002: Reubicar `provider_for_role` / `skill_for_role` a `AgentsConfig`
- STORY-003: Validar `epics_dir` en `validate()` + separar side-effects de creación de directorios
