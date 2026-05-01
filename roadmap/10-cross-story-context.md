# 10 — Cross-story context (conciencia cross-story)

## 🎯 Objetivo

Proveer a cada agente de **contexto sobre las historias relacionadas** con la
que está procesando: dependencias ya completadas, historias bloqueadas por esta,
y la épica que las agrupa. Esto permite que los agentes tomen decisiones más
informadas y reduzcan la tasa de rechazo.

## 📍 Posición en el roadmap

**Fase 5** — después del paralelismo (#01), prompts agnósticos (#09), y
groom --from-dir (#14). En este punto:

- Las oleadas paralelas ya definen *cuándo* está disponible el contexto (al
  terminar una oleada, todas sus historias están Done).
- Los prompts agnósticos ya tienen un placeholder `{cross_story_context}` listo
  para recibir este contenido.
- El sistema de providers ya es estable y el contexto se inyecta como texto
  plano en el prompt (agnóstico al provider).

## ❓ Problema actual

Los agentes trabajan **a ciegas** respecto al resto del proyecto:

| Situación | Qué pasa hoy | Qué debería pasar |
|-----------|-------------|-------------------|
| Dev implementa STORY-005 que depende de STORY-003 (Done) | No sabe qué hizo STORY-003. Puede romper la API o duplicar código. | Recibe un resumen: "STORY-003 implementó el endpoint GET /users con paginación. Úsalo." |
| QA escribe tests para STORY-007 | No sabe qué tests ya existen en el proyecto. Puede duplicar o contradecir. | Recibe contexto de la épica y convenciones de test existentes. |
| Reviewer evalúa STORY-010 | No sabe que 3 historias dependen de esta. Puede aprobar una API que rompe a las demás. | Recibe: "STORY-012, 013, 014 dependen de esta historia. Necesitan el endpoint POST /orders." |
| PO valida STORY-004 | No ve la épica completa. Puede aprobar algo inconsistente con el resto. | Recibe el resumen de la épica y las historias hermanas. |

El resultado: **rechazos evitables** que alargan el pipeline y consumen créditos
de LLM. Con paralelismo (#01), el problema se agrava porque varias historias
avanzan simultáneamente y la falta de contexto puede generar conflictos de
integración al final de la oleada.

## ✅ Solución propuesta

### Tres fuentes de contexto

| Fuente | Qué inyecta | Cuándo |
|--------|------------|--------|
| **Dependencias Done** | Resumen de cada historia de la que depende y que ya está en `Done` | Siempre que la historia tenga dependencias Done |
| **Bloqueos salientes** | Lista de historias que dependen de esta, con su descripción resumida | Opcional (configurable, puede saturar prompts) |
| **Épica** | Descripción de la épica a la que pertenece la historia | Si la historia tiene `## Epic` y el archivo de épica existe |

### Formato del contexto inyectado

El contexto se añade al prompt como una sección delimitada entre líneas `---`:

```markdown
---
## 📋 Contexto del proyecto

### Épica: EPIC-003 — Sistema de usuarios
Las historias de esta épica implementan el CRUD de usuarios con autenticación
JWT y roles (admin, user). Sigue las convenciones de la API ya establecidas.

### Dependencias completadas
- **STORY-003** (Done): Endpoint GET /api/users con paginación. Usa UserRepository
  y devuelve `PaginatedResponse<T>`. Ver `src/handlers/users.rs:42`.
- **STORY-004** (Done): Middleware de autenticación JWT. Inyecta `AuthUser` en
  `RequestContext`. Ver `src/middleware/auth.rs:18`.

### Historias que dependen de esta (si falla esto, fallan ellas)
- **STORY-008**: Endpoint PUT /api/users/{id} — necesita el schema de User
  definido aquí.
- **STORY-009**: Panel de admin — consume el endpoint que estás implementando.

---
```

### Configuración en `.regista/config.toml`

```toml
[limits]
# Cuántas historias de contexto inyectar como máximo (por categoría).
cross_story_max_deps_context = 3      # default: 3. 0 = desactivado.
cross_story_max_blocked_by_context = 2 # default: 0 (desactivado por defecto).

# Incluir resumen de épica si está disponible.
cross_story_include_epic = true       # default: true
```

### Interacción con paralelismo (#01)

Con oleadas paralelas, el contexto se construye **al inicio de cada oleada**,
una vez que la oleada anterior ha terminado:

```
Oleada 1: [STORY-001, STORY-002]  ← sin dependencias Done → sin contexto cross-story
Oleada 2: [STORY-003, STORY-004]  ← dependen de 001 y 002 (ya Done)
           ↑ Cada una recibe resumen de su dependencia Done.
Oleada 3: [STORY-005]             ← depende de 003 y 004 (ya Done)
           ↑ Recibe resúmenes de ambas.
```

La función `build_cross_story_context(story, all_stories, graph, config)`:

1. Busca las dependencias de `story` que están `Done` en `all_stories`.
2. Para cada una, extrae un resumen (primeras 3 líneas de `## Descripción`).
3. Busca qué historias en `all_stories` dependen de `story` (bloqueos salientes).
4. Busca el archivo de épica si existe (`.regista/epics/EPIC-NNN.md`).
5. Ensambla la sección de contexto.

### Resúmenes de historias Done

Para no saturar la ventana de contexto del LLM, cada historia Done se resume a:

```
- **{id}** ({status}): {primeras 1-2 frases de la descripción}.
  Decisiones clave: {última entrada del Activity Log}.
```

El resumen se genera una vez cuando la historia llega a `Done` y se cachea
en `.regista/decisions/{story_id}-summary.md` para no re-leer y re-resumir
en cada iteración.

### Estrategia de resumen (sin LLM adicional)

Para no gastar créditos generando resúmenes, se usa extracción determinista:

1. `## Descripción` → primeras 200 caracteres.
2. `## Activity Log` → última entrada (línea final).
3. `## Decisiones` o `## Notas técnicas` → si existe, primeras 100 caracteres.

Sin llamadas额外 al LLM. Es barato, determinista, y suficiente para dar contexto.

## 📝 Notas de implementación

### Archivos modificados

| Archivo | Cambio | Líneas |
|---------|--------|--------|
| `src/config.rs` | Nuevos campos en `LimitsConfig`: `cross_story_max_deps_context`, `cross_story_max_blocked_by_context`, `cross_story_include_epic` | +15 |
| `src/prompts.rs` | Nuevo struct `CrossStoryContext`; placeholder `{cross_story_context}` en `build_prompt()` | +5 |
| `src/orchestrator.rs` | Función `build_cross_story_context()`; integración en `process_story()`; generación de resumen al llegar a Done | +120 |
| `src/story.rs` | Método `summary() -> String` que extrae primeras 200 chars de descripción | +15 |
| Tests | Tests de `build_cross_story_context` con/sin dependencias; tests de truncado | +60 |
| **Total** | | **~215 líneas** |

### Riesgos

- **Saturación de ventana de contexto**: si una historia tiene 10 dependencias
  Done, inyectar 10 resúmenes puede consumir miles de tokens. El límite
  `cross_story_max_deps_context = 3` lo previene. Si hay más de N dependencias,
  se listan solo los IDs y se indica "y X más".
- **Contexto desactualizado**: si una historia Done se modifica después de
  generar su resumen, el resumen cacheado queda obsoleto. Solución: invalidar
  caché si el `mtime` del archivo de historia cambió.
- **Información sensible**: los resúmenes pueden contener nombres de archivos
  o detalles internos. No es un riesgo nuevo (el agente ya lee la historia
  completa), pero con contexto cross-story se amplifica. Los providers ya
  manejan esto (el código nunca sale del proyecto).
- **Costo de generar resúmenes**: la estrategia de extracción determinista
  (sin LLM) asegura que esto no consume créditos. Si en v2 se quiere un
  resumen más inteligente, se puede añadir una llamada opcional al LLM.

## 🔗 Relacionado con

- [`01-paralelismo.md`](./01-paralelismo.md) — **prerrequisito**. El contexto
  cross-story depende del modelo de oleadas: el contexto se construye al
  inicio de cada oleada con las historias Done de oleadas anteriores.
- [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) — **prerrequisito**.
  El contexto cross-story se inyecta en el placeholder `{cross_story_context}`
  del prompt agnóstico.
- [`04-workflow-configurable.md`](./04-workflow-configurable.md) — **cliente**.
  Con workflows custom, los agentes necesitan aún MÁS contexto (reglas de
  transición, condiciones). El cross-story context sienta las bases.
- [`20-multi-provider.md`](./20-multi-provider.md) — el contexto se inyecta
  como texto en el prompt, independiente del provider. Funciona igual con pi,
  Claude Code, Codex y OpenCode.
- [`05-validate.md`](./05-validate.md) — la validación pre-vuelo ya comprueba
  que las referencias a dependencias y épicas son correctas. El cross-story
  context se beneficia de esa validación previa.
