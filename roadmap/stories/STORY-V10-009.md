# STORY-V10-009: Adaptar deadlock y graph a Task genérico

## Status
**Draft**

## Epic
EPIC-V10-02

## Descripción
Adaptar `domain/deadlock.rs` y `domain/graph.rs` para que trabajen con `Task` genérico en lugar del `Story` hardcodeado de v0.x. La lógica de detección de ciclos y dependencias bloqueantes es la misma, pero las referencias a IDs (STORY-NNN) deben generalizarse para aceptar cualquier patrón de ID definido en `task_format.id_pattern`.

El algoritmo de priorización de deadlock (qué task desbloquear primero) debe seguir funcionando: priorizar la task que desbloquea más tareas, y en caso de empate, la de menor ID numérico.

**Valor de negocio**: El orquestador sigue detectando y resolviendo bloqueos automáticamente, pero ahora con cualquier formato de task, no solo STORY-NNN.

## Criterios de aceptación
- [ ] CA1: `DependencyGraph::from_tasks(tasks)` construye el grafo usando `task.blockers` y `task.id` genéricos (sin asumir formato STORY-NNN), con detección de ciclos vía DFS
- [ ] CA2: `analyze_deadlock(tasks, graph, workflow)` usa `Task` en lugar de `Story`, y sus mensajes de resolución referencian los IDs de task con el formato que tengan (TASK-001, STORY-001, etc.)
- [ ] CA3: Las transiciones automáticas (blocked/unblocked/failed) se implementan como métodos de `ConfigurableWorkflow` que reciben `&Task` y `&DependencyGraph`, sin dependencia de tipos fijos

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-007

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 2
