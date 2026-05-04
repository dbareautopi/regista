# EPIC-02: Cacheo y Optimización de I/O del Pipeline

## Descripción
Reducir drásticamente la carga de I/O del pipeline principal cacheando historias (con invalidación por `mtime`), memoizando el `DependencyGraph`, y limitando `git add` a los paths relevantes en lugar de stajear todo el repositorio.

Cubre los hallazgos #2.1, #2.2 y #2.3 de la auditoría.

## Historias
- STORY-004: `StoryCache` con invalidación por `mtime`
- STORY-005: Memoización de `DependencyGraph` en el orchestrator
- STORY-006: `git add` selectivo para snapshots
