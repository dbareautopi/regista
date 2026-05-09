# STORY-V10-006: Task genérico con parseo configurable

## Status
**Draft**

## Epic
EPIC-V10-02

## Descripción
Crear `domain/task.rs` con una struct `Task` genérica que reemplace al `Story` hardcodeado de v0.x. El formato de archivo `.md` de cada task debe ser configurable: el usuario define en `[workflow.task_format]` el patrón de ID (`id_pattern`), qué secciones buscar (`section_markers`), y el marcador de dependencias (`dependency_marker`).

El parser debe ser capaz de extraer cualquier campo definido en `section_markers` y almacenarlo en `fields: HashMap<String, String>`. El contenido que no coincide con ninguna sección conocida se conserva en `raw_content` para inyectarlo completo en los prompts.

**Valor de negocio**: Permite que regista orqueste cualquier dominio (research, single-agent, etc.), no solo desarrollo de software. El usuario define qué campos necesita en sus tareas.

## Criterios de aceptación
- [ ] CA1: `Task::load(path, task_format) -> Result<Task>` lee un archivo `.md`, extrae el ID usando `id_pattern` (regex configurable), y parsea secciones según `section_markers` (mapa de `nombre_de_campo → marcador_de_sección`, ej. `"status" → "## Status"`), poblando `fields: HashMap<String, String>`
- [ ] CA2: Las dependencias se extraen del campo marcado por `dependency_marker` (ej. `"Bloqueado por:"`), resultando en `blockers: Vec<String>`. El `activity_log` se parsea desde la sección `"## Activity Log"` con entradas `fecha | actor | descripción`
- [ ] CA3: `Task::set_status(path, new_status, section_markers)` escribe el nuevo estado en disco usando el marcador de sección configurado, preservando el resto del contenido intacto. Si el parseo post-escritura falla, se restaura desde `.bak`

## Dependencias
- Ninguna (módulo de dominio puro)

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 2
