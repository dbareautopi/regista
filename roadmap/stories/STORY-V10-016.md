# STORY-V10-016: Adaptar board.rs a columnas dinámicas desde workflow

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Adaptar `app/board.rs` para que las columnas del dashboard Kanban se deriven dinámicamente del workflow configurado en lugar de usar los 9 estados fijos de v0.x. El orden de columnas debe seguir el flujo natural del DAG de fases (desde el estado inicial, recorriendo las fases en orden de definición, hasta los estados terminales).

El dashboard debe seguir funcionando con `--json` para CI/CD y `--epic` para filtrar (usando el campo de épica definido en `task_format`).

**Valor de negocio**: El dashboard refleja fielmente el pipeline del usuario, con solo las columnas que su workflow define. Sin columnas vacías ni estados irrelevantes.

## Criterios de aceptación
- [ ] CA1: `BoardData::from_tasks(tasks, workflow)` agrupa las tasks por su `status` y ordena las columnas siguiendo el DAG de fases: estados en orden topológico desde el inicial hasta los terminales. Estados sin tasks se omiten
- [ ] CA2: `--json` emite `{ "columns": [...], "tasks": {...}, "summary": {...} }` donde `columns` contiene solo los estados con tasks y en el orden del workflow
- [ ] CA3: `--epic <ID>` filtra tasks cuyo campo `epic` (definido en `section_markers`) coincide con el ID dado. Si el preset no define campo `epic`, el filtro no se aplica y se advierte al usuario

## Dependencias
- Bloqueado por: STORY-V10-006, STORY-V10-007

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
