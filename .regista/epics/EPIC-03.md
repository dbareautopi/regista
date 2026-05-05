# EPIC-03: Abstracción de Workflow Configurable

## Descripción
Extraer un trait `Workflow` que encapsule las decisiones hardcodeadas del workflow actual (`next_status`, `map_status_to_role`, transiciones automáticas, orden canónico de columnas). Implementar `CanonicalWorkflow` como default para mantener retrocompatibilidad. Esto desacopla la preparación para #04 sin romper el pipeline existente.

Cubre el hallazgo #1 completo de la auditoría.

## Historias
- STORY-007: Definir trait `Workflow` + implementar `CanonicalWorkflow`
- STORY-008: Migrar `pipeline.rs` a usar `&dyn Workflow`
- STORY-009: Adaptar `board.rs` para columnas dinámicas según workflow
