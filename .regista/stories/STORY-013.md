# STORY-013: Compactación de checkpoint — filtrar historias terminales (Done/Failed)

## Status
**Draft**

## Epic
EPIC-05

## Descripción
El checkpoint actual (`OrchestratorState`) mantiene entradas en `story_iterations`, `reject_cycles` y `story_errors` para todas las historias, incluso las que ya llegaron a estado terminal (`Done` o `Failed`). Esto hace que el checkpoint crezca sin límite. Al guardar, se deben filtrar las entradas correspondientes a historias en estado terminal, manteniendo solo las de historias activas. También se debe evitar clonar todos los HashMaps en cada iteración (usar referencias o construir el estado de checkpoint incrementalmente).

## Criterios de aceptación
- [ ] CA1: `save_checkpoint()` (o `OrchestratorState::save()`) recibe la lista de historias y filtra las entradas de historias en estado `Done` o `Failed`
- [ ] CA2: `story_iterations` solo contiene entradas para historias activas (no terminales)
- [ ] CA3: `reject_cycles` solo contiene entradas para historias activas
- [ ] CA4: `story_errors` solo contiene entradas para historias activas
- [ ] CA5: Los clones de HashMap en `save_checkpoint()` se reemplazan por referencias (tomar `&HashMap` en lugar de clonar) o se construye el estado incrementalmente
- [ ] CA6: `cargo test --lib checkpoint` pasa (tests existentes + test que verifica compactación)
- [ ] CA7: El checkpoint de un pipeline con 100 historias (50 Done, 50 activas) tiene ~50 entradas, no ~100

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #3.1, #3.2, #3.4, recomendación #9).
