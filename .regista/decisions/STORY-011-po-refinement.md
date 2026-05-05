# STORY-011 — Refinamiento PO (Draft → Ready)

**Fecha**: 2026-05-05
**Rol**: Product Owner
**Transición**: Draft → Ready

---

## Verificación DoR

### 1. Descripción clara y no ambigua
La historia describe con precisión el problema actual (variables locales `reject_cycles`, `story_iterations`, `story_errors` pasadas como `&mut`) y la solución requerida (wrapping en `Arc<RwLock<HashMap<...>>>`). Coincide exactamente con el hallazgo #10.3 de `roadmap/AUDITORIA-ESCALABILIDAD.md`.

### 2. Criterios de aceptación específicos y testeables
Los 7 CAs son específicos y verificables:
- CA1: Define `SharedState` con tipos concretos (`Arc<RwLock<HashMap<String, u32>>>`, `Arc<RwLock<HashMap<String, String>>>`).
- CA2: Cambio de firma de `process_story()`.
- CA3: Patrones de lock (`read()`, `write()`) con unwrap para locks de corta duración.
- CA4: `apply_automatic_transitions()` accede vía `SharedState`.
- CA5: `save_checkpoint()` clona bajo `read()` lock.
- CA6-CA7: Gates de compilación y tests.

### 3. Dependencias
Ninguna. STORY-010 (migración a tokio) es ortogonal: `Arc<RwLock<>>` funciona en código síncrono. Ambas historias pueden implementarse en paralelo.

---

## Notas para el Developer

1. **Comando de test (CA6)**: `cargo test --lib orchestrator` es una referencia heredada. El módulo actual es `app::pipeline`. El comando efectivo será `cargo test --lib` (todos los tests unitarios). Adaptar según la estructura real del proyecto.

2. **Firma de `save_checkpoint()`**: Actualmente recibe 3 `&HashMap` separados. Debe cambiarse a recibir `&SharedState` como parte de CA5. La serialización de `OrchestratorState` no requiere cambios (sus campos siguen siendo `HashMap<...>` sin locks — es la snapshot serializable).

3. **Funciones afectadas** (confirmado contra código actual en `src/app/pipeline.rs`):
   - `process_story(story, project_root, cfg, reject_cycles: &mut HashMap<...>, agent_opts, workflow)` → `process_story(story, project_root, cfg, state: &SharedState, agent_opts, workflow)`
   - `apply_automatic_transitions(stories, graph, reject_cycles: &mut HashMap<...>, cfg, simulate, workflow)` → `apply_automatic_transitions(stories, graph, state: &SharedState, cfg, simulate, workflow)`
   - `save_checkpoint(project_root, iteration, reject_cycles, story_iterations, story_errors)` → `save_checkpoint(project_root, iteration, state: &SharedState)`
   - `run_real()`: crear `SharedState` al inicio en lugar de 3 `HashMap` separados.

4. **Inmutabilidad**: `run_dry()` usa sus propios `HashMap::new()` locales. Puede crear un `SharedState` dummy o mantener variables separadas — no requiere locks en modo simulación.

5. **Épica**: EPIC-04 (Preparación Async para Paralelismo). Este es el segundo de tres prerequisitos.

---

## Referencias
- Hallazgo #10.3 en `roadmap/AUDITORIA-ESCALABILIDAD.md`
- Recomendación #7 en `roadmap/AUDITORIA-ESCALABILIDAD.md`
- EPIC-04: `.regista/epics/EPIC-04.md`
