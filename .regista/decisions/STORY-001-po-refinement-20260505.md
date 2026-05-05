# STORY-001 — PO Refinement — 2026-05-05

## Decisión
**STORY-001 pasa de Draft → Ready.** La historia cumple el Definition of Ready.

---

## Validación DoR

### 1. Descripción clara y no ambigua ✅

La historia describe dos cambios concretos y bien delimitados:

| # | Cambio | Archivo afectado |
|---|--------|-----------------|
| 1 | `from_name()` devuelve `Result<Box<dyn AgentProvider>>` en vez de hacer `panic!` | `src/infra/providers.rs` |
| 2 | `validate` verifica que los binarios de providers existen en `PATH` | `src/app/validate.rs` |

No hay ambigüedad: se sabe exactamente qué archivos tocar, qué comportamiento actual hay que cambiar, y cuál es el comportamiento esperado.

### 2. Criterios de aceptación testeables ✅

Los 9 CAs cubren exhaustivamente los dos cambios:

**Cambio 1 — `from_name()` → `Result` (CA1-CA5, CA8):**
- CA1: Caso feliz (`"pi"` → `Ok`)
- CA2: Caso error (`"inventado"` → `Err`, sin panic)
- CA3-CA4: Aliases de Claude Code y OpenCode (regresión)
- CA5: Adaptación de callers (compilación)
- CA8: Tests existentes de providers no se rompen

**Cambio 2 — `validate` binarios (CA6-CA7, CA9):**
- CA6: Binario ausente → `Finding::Error`
- CA7: Codex no verificable → `Finding::Warning` (instalación npm no estándar)
- CA9: Tests existentes de validator no se rompen

### 3. Dependencias identificadas ✅

La historia declara explícitamente "Ninguna". Verificado: no hay dependencias entre STORY-001 y otras historias.

---

## Callers de `from_name()` — impacto del cambio

Se identificaron todos los puntos de llamada que necesitarán adaptación (CA5):

| Caller | Archivo | Uso actual |
|--------|---------|-----------|
| `skill_for_role()` | `src/infra/providers.rs:271` | `let provider = from_name(&provider_name);` → necesita `?` |
| Pipeline orchestrator | `src/app/pipeline.rs` | `let provider = providers::from_name(&provider_name);` → necesita `?` |
| Init scaffolding | `src/app/init.rs` | `let provider = providers::from_name(provider_name);` → necesita `?` |
| Plan generation | `src/app/plan.rs` | `let provider = providers::from_name(&provider_name);` → necesita `?` |

---

## Notas adicionales

- La historia referencia correctamente `src/infra/providers.rs` (la estructura actual del proyecto tras la reorganización en `domain/`, `app/`, `infra/`, `cli/`).
- El provider por defecto en `.regista/config.toml` es `"pi"`.
- CA7 (Codex warning) es pragmático: `codex` puede instalarse vía `npm i -g @openai/codex` con nombre de binario no predecible.
- Las referencias a `cargo test --lib providers` y `cargo test --lib validator` en CA8/CA9 son ilustrativas; con la estructura modular actual, los paths exactos serían `cargo test infra::providers` y `cargo test app::validate`, pero la intención (que los tests existentes sigan pasando) es clara.

---

## Conclusión

Historia lista para la fase de QA (Ready → Tests Ready). No se requiere intervención adicional del PO en este momento.
