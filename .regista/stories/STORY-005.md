# STORY-005: Memoización de `DependencyGraph` en el orchestrator

## Status
**Draft**

## Epic
EPIC-02

## Descripción
Actualmente el `DependencyGraph` se reconstruye 2-3 veces por iteración del pipeline (`from_stories()` itera sobre todas las historias y sus dependencias). Para 100 historias con 3 dependencias de media y 300 iteraciones, esto genera ~270.000 inserciones en HashMap. El grafo debe calcularse una vez y reconstruirse solo cuando `apply_automatic_transitions()` modifica estados que afectan las dependencias (cuando una historia bloqueada se desbloquea).

## Criterios de aceptación
- [ ] CA1: El orchestrator (`pipeline.rs`) mantiene una instancia de `DependencyGraph` como variable local del loop en lugar de reconstruirlo desde cero en cada uso
- [ ] CA2: El grafo se reconstruye solo cuando `apply_automatic_transitions()` modifica al menos un estado (Blocked → Ready o * → Failed)
- [ ] CA3: El resto de usos del grafo (deadlock, pick_next_actionable) usan la instancia memoizada
- [ ] CA4: El comportamiento funcional del pipeline es idéntico (mismas transiciones automáticas, misma detección de deadlocks)
- [ ] CA5: `cargo test --lib dependency_graph` pasa
- [ ] CA6: `cargo test --lib orchestrator` pasa (tests existentes de pipeline)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #2.2, recomendación #3).
