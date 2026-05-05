# Decisión de implementación — STORY-009

**Fecha:** 2026-05-05
**Autor:** Dev
**Transición:** Tests Ready → In Review

---

## Resumen

Reemplazado el placeholder `todo!()` en `render_board()` con la lógica de renderizado
dinámico basada en `&dyn Workflow`. Eliminado el array hardcodeado `canonical_order`
de `print_human()`, que ahora delega completamente en `render_board()`.

---

## Cambios realizados

### `src/app/board.rs`

1. **`render_board(data, workflow)`** — implementado el cuerpo completo:
   - Cabecera fija: `📊 Story Board — regista`
   - Columnas en el orden devuelto por `workflow.canonical_column_order()`
   - Omite columnas con `count = 0` (CA3)
   - Línea separadora `─` solo si hay columnas visibles
   - Total (siempre visible)
   - Sección `🔴 Blocked` (si hay bloqueadas)
   - Sección `❌ Failed` (si hay fallidas)

2. **`print_human(data, workflow)`** — firma actualizada para aceptar `&dyn Workflow`.
   Cuerpo simplificado a delegar en `render_board()` e imprimir el resultado.

3. **`run()`** — pasa `&CanonicalWorkflow` a `print_human()`.

---

## Verificación

| Comando | Resultado |
|---------|-----------|
| `cargo test board` | 16/16 pasan |
| `cargo test` | 223/223 pasan (1 ignorado) |
| `cargo fmt` | limpio |
| `cargo clippy -- -D warnings` | limpio |

### Tests STORY-009 (8 tests)

| Test | CA | Verifica |
|------|----|---------|
| `render_board_accepts_dyn_workflow` | CA1 | Firma acepta `&dyn Workflow` (compila) |
| `column_order_comes_from_workflow` | CA2 | Orden de `canonical_column_order()` |
| `empty_columns_are_skipped` | CA3 | Columnas count=0 omitidas |
| `all_empty_columns_shows_none` | CA3 | Sin columnas cuando todas count=0 |
| `canonical_workflow_output_matches_current_behavior` | CA4 | Output canónico idéntico al actual |
| `custom_5_column_workflow_shows_exactly_those_columns` | CA6 | Workflow 5-columnas muestra solo esas 5 |
| `custom_workflow_skips_empty_columns` | CA3+CA6 | Workflow custom omite count=0 |
| (pre-existentes) | CA5 | `cargo test` pasa |

---

## Decisiones de diseño

- **`render_board()` como función de renderizado canónica**: centraliza toda la lógica
  de formato en un solo lugar. `print_human()` y futuros renderizadores (TUI, #11)
  pueden reutilizarla.

- **`CanonicalWorkflow` como default en `run()`**: mantiene retrocompatibilidad total.
  Cuando #04 permita workflows configurables, `run()` obtendrá el workflow de la config.

- **Separador `─` condicional**: solo se muestra si hay al menos una columna visible.
  Evita una línea huérfana cuando todas las columnas están vacías.

- **Sin cambios en `build_board_data()`**: ya operaba con claves string, por lo que
  es naturalmente compatible con workflows que tengan nombres de columna arbitrarios.
