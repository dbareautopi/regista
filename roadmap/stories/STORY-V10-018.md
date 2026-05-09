# STORY-V10-018: Fase de descomposición — generar tareas desde input

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Implementar la fase de descomposición del workflow como parte de `regista run --plan-only`.
Cuando `run` detecta que no existen tareas en `tasks_dir`, ejecuta la fase de descomposición
definida en el workflow (marcada con `decomposition = true` o con `from = "_init_"`).

Esta fase invoca al LLM nativo con el input del usuario (archivo de especificación, topic,
o lo que corresponda al preset) y genera los archivos `.md` iniciales en `tasks_dir` siguiendo
el `task_format` configurado. Tras generar, aplica un bucle de validación que verifica
dependencias correctas (sin referencias rotas, sin ciclos) y da feedback al agente si es necesario.

El flag `--plan-only` detiene el pipeline tras esta fase sin ejecutar el resto.

**Valor de negocio**: `run` funciona desde cero con cualquier preset. Sin tareas previas, las genera.
Con `--plan-only`, el usuario puede revisar el backlog antes de ejecutar el pipeline completo.

## Criterios de aceptación
- [ ] CA1: `regista run --plan-only spec.md` invoca al LLM con el modelo del primer rol del workflow, renderiza el prompt de la fase de descomposición con el input, y genera archivos `.md` en `tasks_dir` con el `task_format` definido (id_pattern, section_markers, dependency_marker)
- [ ] CA2: El bucle de validación post-generación (máx `plan_max_iterations`) parsea las tareas, verifica dependencias sin ciclos ni referencias rotas, y reinyecta feedback al agente si hay problemas
- [ ] CA3: `--plan-only` detiene el pipeline tras la descomposición. `run` sin `--plan-only` encadena descomposición + pipeline completo en una sola ejecución

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-010

## Activity Log
- 2026-05-09 | PO | reescrita: plan/auto unificados en run --plan-only
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
