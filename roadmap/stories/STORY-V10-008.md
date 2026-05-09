# STORY-V10-008: Sistema de templates para prompts

## Status
**Draft**

## Epic
EPIC-V10-02

## Descripción
Crear `domain/prompts.rs` con un sistema de templates que reemplaza los 7 prompts hardcodeados de v0.x. El sistema usa `{{variables}}` que se sustituyen en tiempo de ejecución con los campos de la `Task`, el contexto del pipeline, y los valores inyectados por el orquestador.

Esto permite que los prompts definidos en `PhaseConfig` (TOML) o en presets referencien dinámicamente cualquier campo de la task sin conocer su estructura de antemano.

**Valor de negocio**: Los usuarios pueden escribir sus propios prompts en TOML sin tocar código Rust. Las variables `{{task_fields.*}}` permiten referenciar cualquier campo definido en `task_format` sin conocerlo en tiempo de compilación.

## Criterios de aceptación
- [ ] CA1: `render_template(template, task, context) -> String` sustituye `{{task_id}}`, `{{task_status}}`, `{{task_fields.<campo>}}` (valor de un campo concreto), `{{task_fields.*}}` (bullet list de todos los campos), `{{last_rejection}}`, `{{blockers}}` y `{{context.<clave>}}` por sus valores reales
- [ ] CA2: El system prompt de cada rol se carga desde `RoleConfig.system_prompt` (TOML o preset) y se renderiza con las mismas variables que el prompt de fase, más `{{role_name}}`
- [ ] CA3: Si una variable referenciada no existe (ej. `{{task_fields.priority}}` pero la task no tiene campo `priority`), se sustituye por `"(no definido)"` en lugar de paniquear, con un warning en el log

## Dependencias
- Bloqueado por: STORY-V10-006

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 2
