# Plan Decision: Descomposición de Auditoría de Escalabilidad

**Fecha**: 2026-05-04
**Fuente**: `roadmap/AUDITORIA-ESCALABILIDAD.md`
**Versión analizada**: v0.8.0

---

## Decisiones de descomposición

### 1. Agrupación en 6 épicas

La auditoría cubre 14 áreas de mejora. Se agruparon en 6 épicas siguiendo el principio de **alta cohesión interna y bajo acoplamiento entre épicas**:

| Épica | Áreas cubiertas | Motivación |
|-------|----------------|------------|
| EPIC-01: Providers Robustos | #7 (from_name panic, binarios), #8 (epics_dir, side-effects), #11.2 (reubicar funciones) | Todas tocan el sistema de providers/config; comparten el mismo contexto de prueba |
| EPIC-02: Cacheo y Optimización | #2.1 (load_all_stories), #2.2 (DependencyGraph), #2.3 (git add -A) | Las 3 optimizan el hot path de I/O del pipeline |
| EPIC-03: Workflow Abstraído | #1 (enum vs string, next_status, map_status_to_role, apply_automatic_transitions, board) | Prepara el terreno para #04 sin romper compatibilidad |
| EPIC-04: Async Migration | #2.4 (busy-polling), #10.2 (agent sync), #10.3 (HashMaps no Send+Sync) | Prerrequisitos directos para #01 (paralelismo) |
| EPIC-05: Estado y Daemon | #3 (checkpoint), #6.1-6.5 (daemon polling, kill, log), #9 (formato historia) | Agrupa la robustez del estado persistente y operación continua |
| EPIC-06: Métricas y Limpieza | #5 (PromptContext clones), #11.1 (extract_numeric), #11.3 (métricas), #11.4 (benchmarks) | Mejoras de calidad que no bloquean features mayores |

### 2. Orden de implementación recomendado

1. **EPIC-02** (Cacheo) — mejora inmediata de rendimiento, sin cambiar APIs públicas
2. **EPIC-01** (Providers) — elimina panics y mejora feedback al usuario
3. **EPIC-05** (Estado) — checkpoint y logs robustos, prerequisito para pipelines largos
4. **EPIC-03** (Workflow) — desacopla el workflow, habilita #04
5. **EPIC-04** (Async) — último prerequisito para #01, requiere EPIC-02 y EPIC-03 estables
6. **EPIC-06** (Métricas) — nice-to-have, no bloquea nada

### 3. Dependencias entre historias

- STORY-008 (adaptar pipeline a Workflow) → STORY-007 (trait Workflow)
- STORY-009 (board dinámico) → STORY-007 (trait Workflow)
- STORY-012 (pipeline async) → STORY-010 (agent async) + STORY-011 (Arc<RwLock<>>)
- STORY-017 (health.rs) → STORY-011 (Arc<RwLock<>>, para entender el estado compartido)

El resto de historias son independientes y se pueden implementar en paralelo dentro de su épica.

### 4. Criterios de atomicidad

Cada historia:
- Modifica como máximo 2-3 módulos
- Se puede implementar en una sesión de agente (< 200 líneas de cambio)
- Tiene criterios de aceptación verificables con `cargo test` o `cargo build`
- Entrega valor por sí misma (ej: STORY-001 elimina panics incluso sin el resto de EPIC-01)

### 5. Lo que NO se incluye (out of scope)

- La implementación completa de #01 (paralelismo) — este backlog solo cubre los prerequisitos
- La implementación completa de #04 (workflow configurable) — solo la abstracción del trait
- Cambios en el contrato de formato de historia que requieran migración de historias existentes
- Integración con sistemas externos de monitoreo (Prometheus, etc.)
