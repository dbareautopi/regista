# 22 — `plan --append`: generación incremental consciente de historias existentes

> **Estado**: ✍️ Especificación
> **Esfuerzo**: Medio
> **Complementa**: [#10 Cross-story context](./10-cross-story-context.md)

---

## 🎯 Problema

Actualmente `regista plan <spec>` tiene dos modos:

| Modo | Comportamiento |
|------|---------------|
| **Default** (sin `--replace`) | El PO genera historias en el directorio sin saber qué IDs ya existen. Puede pisar archivos, duplicar IDs, o crear dependencias inconsistentes con el backlog previo. |
| **`--replace`** | Borra todo el backlog existente y empieza desde cero. Destructivo. |

No existe un modo **incremental** que permita añadir nuevas historias a un backlog existente de forma segura. Si un equipo tiene 30 historias y quiere añadir 5 más desde una spec nueva, no puede hacerlo sin riesgo de colisiones o sin borrar todo.

---

## ✅ Solución propuesta

### Flag `--append` (nuevo, mutuamente excluyente con `--replace`)

```
regista plan spec-nueva.md --append
regista auto spec-nueva.md --append
```

### Comportamiento

1. **Detección de IDs existentes**: antes de invocar al PO, `plan.rs` escanea `stories_dir/` y extrae todos los IDs numéricos existentes (`STORY-001` → `1`). Calcula `next_id = max_id + 1`.

2. **Prompt contextualizado**: el prompt del PO incluye:
   - Lista de IDs de historias ya existentes (solo IDs y títulos, no contenido completo)
   - El `next_id` desde el cual debe empezar a numerar
   - Instrucción explícita: "Puedes referenciar historias existentes en `Bloqueado por:`"
   - Las épicas existentes también se listan para que el PO decida si añade historias a épicas existentes o crea nuevas

3. **Validación de no-colisión**: tras la generación, `plan.rs` verifica que ningún archivo nuevo pisa uno existente. Si hay colisión, se reporta como error.

4. **Validación de dependencias cruzadas**: el bucle `plan → validate` comprueba que las dependencias a historias existentes son válidas (la historia referenciada existe y su ID está en el conjunto de IDs preexistentes).

### Prompt del PO en modo `--append`

```
Eres un Product Owner. Vas a añadir NUEVAS historias a un backlog EXISTENTE.

## Backlog existente ({existing_count} historias, {existing_epics} épicas)

Historias ya existentes (NO las modifiques):
{existing_stories_list}

Épicas ya existentes:
{existing_epics_list}

## Nueva especificación a descomponer
Archivo: {spec_path}
...

## Instrucciones

1. NO modifiques ni borres ninguna de las historias existentes.
2. Empieza a numerar desde STORY-{next_id}.
3. Si una historia nueva depende de una existente, indícalo en "Bloqueado por:".
4. Puedes añadir historias a épicas existentes O crear nuevas épicas.
5. Si creas una épica nueva, usa el siguiente ID disponible: EPIC-{next_epic_id}.
```

### Relación con #10 Cross-story context

| Feature | Qué aporta |
|---------|-----------|
| **#22 `--append`** | El PO sabe qué historias existen y genera nuevas SIN pisarlas. Conciencia *estructural* del backlog (IDs, títulos, épicas). |
| **#10 Cross-story context** | Los agentes (Dev, QA, Reviewer) reciben resúmenes de historias relacionadas durante la implementación. Conciencia *semántica* (decisiones, interfaces, contratos). |

Son complementarias: `#22` resuelve la **generación** del backlog, `#10` resuelve la **ejecución** del pipeline con contexto. Tienen sentido implementarlas juntas (Fase 4) porque comparten la lógica de "leer historias existentes y extraer información relevante".

---

## 📁 Módulos afectados

| Módulo | Cambios |
|--------|---------|
| `cli/args.rs` | Añadir `--append` a `PlanModeArgs`, mutually exclusive con `--replace` |
| `app/plan.rs` | Escanear historias/épicas existentes, calcular `next_id`, generar prompt contextualizado, validar no-colisión |
| `domain/story.rs` | Posible helper `extract_id_number(path)` para parsear IDs numéricos |
| `app/validate.rs` | Extender validación de dependencias: si una historia referencia un ID que existe, debe ser un ID real (no necesariamente generado en esta sesión) |

---

## 🧪 Tests

| Test | Qué verifica |
|------|-------------|
| `append_detects_existing_ids` | Escanea directorio con STORY-001..STORY-005, `next_id = 6` |
| `append_does_not_overwrite` | Si PO intenta escribir STORY-003.md, se detecta colisión |
| `append_prompt_includes_existing_stories` | El prompt contiene IDs y títulos del backlog existente |
| `append_prompt_includes_next_id` | El prompt contiene `next_id` correcto |
| `append_validate_allows_existing_deps` | Dependencia a STORY-001 (existente) es válida |
| `append_validate_rejects_nonexistent_deps` | Dependencia a STORY-999 (no existe) es error |
| `append_and_replace_are_mutually_exclusive` | `--append --replace` → error de CLI |
| `append_with_empty_backlog_behaves_like_default` | Sin historias previas, next_id=1, equivalente a default |

---

## 🔢 Orden de implementación

Encaja en **Fase 4** del roadmap, junto con #10 Cross-story context. Ambas features comparten la necesidad de escanear el backlog existente, por lo que implementarlas juntas evita duplicar lógica.

Si se implementa antes que #10, la base de "escanear historias existentes" queda lista para que #10 la reutilice.
