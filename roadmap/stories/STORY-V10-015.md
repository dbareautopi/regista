# STORY-V10-015: Adaptar init.rs para scaffolding con presets y LLM nativo

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Adaptar `app/init.rs` para que el scaffolding refleje el nuevo modelo: en lugar de generar skills CLI para providers externos, genera `.regista/config.toml` con la sección `[models]` (placeholders para API keys) y el workflow completo del preset elegido. Las instrucciones de rol ahora son system prompts dentro del TOML, no archivos de skill separados.

El comando `regista init --preset <name>` debe ofrecer 3 presets (`software-dev`, `research`, `single-agent`) más la opción de generar un `--preset custom` con un template de workflow vacío para que el usuario lo rellene.

**Valor de negocio**: Onboarding rápido. Un usuario nuevo pasa de cero a pipeline funcional con un solo comando (más configurar API keys).

## Criterios de aceptación
- [ ] CA1: `regista init --preset software-dev` genera `.regista/config.toml` con `[models]` (placeholders para `gpt4o` y `claude`), `[workflow]` con el preset completo, `[limits]`, `[hooks]` y `[git]`. No genera directorios `.pi/skills/` ni `.claude/agents/`
- [ ] CA2: `regista init --preset custom` genera un `.regista/config.toml` con `[models]` vacío y `[workflow]` con una fase de ejemplo comentada, más `[workflow.states]` y `[workflow.task_format]` con valores placeholder
- [ ] CA3: Si `.regista/config.toml` ya existe, `init` aborta con error descriptivo a menos que se pase `--force` (nuevo flag que sobreescribe). `--with-example` genera una task de ejemplo en `.regista/tasks/` con el formato del preset

## Dependencias
- Bloqueado por: STORY-V10-013, STORY-V10-014

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
