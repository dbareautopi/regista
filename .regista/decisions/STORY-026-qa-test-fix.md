# Decisión QA: Corrección de header_uses_model_for_role_resolution (STORY-026)

**Fecha:** 2026-05-05  
**Rol:** QA Engineer  
**Historia:** STORY-026

## Problema

El test `header_uses_model_for_role_resolution` fallaba (30/31 pasando)
porque usaba una resolución de paths inconsistente con `format_session_header`:

- **`format_session_header`**: une el path relativo del skill con `project_root`
  (`project_root.join(&skill_rel)`) → path absoluto → lo pasa a `model_for_role`.
- **El test (antes)**: pasaba `Path::new(&skill_path)` directamente a
  `model_for_role`, sin unirlo con `project_root`. Esto resolvía el path
  relativo contra el CWD (`/root/repos/regista`), donde existen skills reales
  con `model: opencode/minimax-m2.5-free` en YAML.

Con `project_root=/tmp`, el header mostraba `desconocido` (skills no existen
en `/tmp`), pero el test esperaba `opencode/minimax-m2.5-free`. Esta
contradicción era también evidente por el test
`models_show_desconocido_by_default`, que correctamente verifica que con
`project_root=/tmp` y sin skills, todos los modelos son `desconocido`.

## Acción tomada

Se reescribió `header_uses_model_for_role_resolution` para:

1. Crear un directorio temporal (`tempdir`) como `project_root`.
2. Crear skills para los 4 roles con modelos YAML distintos
   (`po-model-v1`, `qa-model-v1`, `dev-model-v1`, `reviewer-model-v1`).
3. Usar `Config::default()` (sin modelos explícitos en config).
4. Llamar a `format_session_header` con el `project_root` temporal.
5. Verificar que `model_for_role(role, &skill_abs)` — donde `skill_abs =
   project_root.join(&skill_rel)` — produce el mismo resultado que aparece
   en el header.

Esto replica exactamente la lógica de resolución de paths de
`format_session_header`, verificando la integración real entre ambas
funciones.

## Resultado

- 31/31 tests del módulo `story026` pasan.
- 400/400 tests totales pasan (1 ignorado).
- `cargo build` compila sin errores.
- `cargo clippy -- -D warnings` limpio.
- `cargo fmt` limpio.
- Arquitectura de capas respetada (11 tests de arquitectura pasan).

## Lección aprendida

Al escribir tests que verifican integración entre funciones, es esencial
replicar exactamente la misma lógica de resolución de paths que usa el
código de producción. Usar paths relativos en tests puede causar
comportamiento dependiente del CWD, haciendo que los tests fallen o pasen
de forma no determinista según el entorno de ejecución.
