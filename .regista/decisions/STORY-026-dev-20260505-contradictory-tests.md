# STORY-026: Decisión del Developer — Tests contradictorios

**Fecha**: 2026-05-05  
**Rol**: Developer  
**Historia**: STORY-026 (Header de sesión con metadatos)

## Resumen

Se implementó `format_session_header()` en `src/cli/handlers.rs`. 30 de 31 tests pasan.

## Implementación

### Función principal
- **Modo detallado**: bloque multilínea con bordes `═`, emoji 🛰️, y secciones:
  Proyecto, Provider, Modelos, Límites, Git, Hooks.
- **Modo compacto**: una sola línea con versión, provider, fecha UTC, max_iter.
- **Resolución de modelos**: usa `AgentsConfig::skill_for_role()` para obtener la ruta,
  la resuelve contra `project_root` con `Path::join()`, y pasa la ruta absoluta a
  `AgentsConfig::model_for_role()`.
- **Límites**: `max_iter` efectivo calculado con `max(10, story_count × 6)` cuando
  `max_iterations=0`.
- **Hooks**: lista solo los hooks con `Some(...)`, o "ninguno" si no hay ninguno.

### Funciones auxiliares
- `effective_max_iter(story_count, cfg_max)`: réplica local de la lógica de `app::pipeline`.
- `role_abbreviation(role)`: mapea nombres canónicos a abreviaturas (PO, QA, Dev, Reviewer).

## Test fallido: `header_uses_model_for_role_resolution`

### Descripción del problema
El test pasa `Path::new("/tmp")` como `project_root`, pero espera que los modelos
en el header reflejen lo que devuelve `model_for_role(role, Path::new(&skill_path))`,
donde `skill_path` es la ruta relativa cruda (ej: `.pi/skills/product-owner/SKILL.md`).

`model_for_role` llamado con una ruta relativa resuelve contra el CWD, donde los
skills existen con `model: opencode/minimax-m2.5-free`. Por tanto, el test espera
`PO=opencode/minimax-m2.5-free` en el header.

### Contradicción con `models_show_desconocido_by_default`
Este test también pasa `/tmp` como project_root y espera `PO=desconocido` (sin skills
en `/tmp/.pi/skills/`). Esto solo funciona si la implementación resuelve skills contra
`project_root`.

### Conclusión
Ambos tests no pueden pasar simultáneamente con ninguna implementación:
- Si se resuelve contra `project_root` → `models_show_desconocido_by_default` pasa,
  `header_uses_model_for_role_resolution` falla.
- Si NO se resuelve contra `project_root` → `header_uses_model_for_role_resolution` pasa,
  `models_show_desconocido_by_default` falla.

La implementación elegida resuelve contra `project_root` porque:
1. Es el patrón correcto usado en el resto del código (`app::pipeline`).
2. El test `header_reflects_yaml_frontmatter_model` lo verifica creando skills
   en un directorio temporal y pasándolo como `project_root`.
3. Es la única forma de que los skills específicos del proyecto sean descubiertos.

### Acción requerida del QA
El test `header_uses_model_for_role_resolution` debe ser corregido para:
- Pasar el CWD real (o un directorio con skills) como `project_root`, o
- Unir `skill_path` con `project_root` antes de llamar a `model_for_role` en la
  construcción del valor esperado.

## Resultados

- 30/31 tests pasan ✅
- 1 test falla por inconsistencia en los tests del QA ❌
- `cargo build` compila sin errores ✅
- `cargo fmt` aplicado ✅
- `cargo clippy` sin warnings ✅
