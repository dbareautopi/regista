# STORY-V10-017: Adaptar validate.rs a dominio genérico

## Status
**Draft**

## Epic
EPIC-V10-04

## Descripción
Adaptar `app/validate.rs` para que valide la configuración del nuevo dominio: modelos LLM, workflow, task_format, y tasks. El validador debe detectar problemas como modelos referenciados pero no definidos, regex de ID inválido, fases que referencian estados inexistentes, y tasks que no cumplen el `id_pattern`.

El chequeo pre-vuelo es crítico porque un error de configuración en TOML solo se detecta en runtime. Con `validate`, el usuario puede verificar su setup antes de lanzar un pipeline que consuma créditos de LLM.

**Valor de negocio**: Previene pipelines fallidos por errores de configuración. Ahorra créditos de LLM y tiempo del usuario.

## Criterios de aceptación
- [ ] CA1: Valida `[models]`: detecta modelos referenciados en `workflow.phases[].model` que no existen en `[models]`, y API keys con variables de entorno no definidas (warning, no error)
- [ ] CA2: Valida `[workflow]`: detecta fases cuyo `from` o `to` no están en `states`; `id_pattern` que no es un regex válido; `section_markers` que referencian el mismo marcador para dos campos distintos
- [ ] CA3: Valida tasks: escanea el directorio de tasks, verifica que cada archivo cumple el `id_pattern`, que los estados son válidos según el workflow, y que las dependencias (blockers) referencian tasks existentes (sin ciclos, usando `DependencyGraph`)

## Dependencias
- Bloqueado por: STORY-V10-005, STORY-V10-006, STORY-V10-007

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 4
