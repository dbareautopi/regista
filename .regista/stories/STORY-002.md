# STORY-002: Reubicar `provider_for_role` / `skill_for_role` a `AgentsConfig`

## Status
**Done**

## Epic
EPIC-01

## Descripción
Las funciones `provider_for_role()` y `skill_for_role()` residen actualmente en `src/infra/providers.rs`, creando un acoplamiento incorrecto desde la capa `infra` hacia `config`. Estas funciones dependen de `crate::config::AgentsConfig` y deben ser métodos de `AgentsConfig` en `src/config.rs`. La capa `infra/providers.rs` debe contener solo el trait `AgentProvider` y sus implementaciones.

## Criterios de aceptación
- [x] CA1: `AgentsConfig` tiene el método `pub fn provider_for_role(&self, role: &str) -> String`
- [x] CA2: `AgentsConfig` tiene el método `pub fn skill_for_role(&self, role: &str) -> String`
- [x] CA3: `AgentsConfig` tiene el método `pub fn all_roles() -> impl Iterator<Item = &'static str>` (ya existe, verificar)
- [x] CA4: Las funciones `provider_for_role` y `skill_for_role` se eliminan de `src/infra/providers.rs`
- [x] CA5: Todos los callers (orchestrator, pipeline, validator) usan `cfg.agents.provider_for_role(...)` en lugar de `providers::provider_for_role(&cfg.agents, ...)`
- [x] CA6: `cargo build` compila sin warnings
- [x] CA7: `cargo test` pasa todos los tests existentes

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #11.2).
- 2026-05-05 | PO | Refinamiento: historia validada contra DoR. Descripción clara, 7 CAs testeables, sin dependencias. Decisión documentada en .regista/decisions/STORY-002-po-refinement-2026-05-05.md. Draft → Ready.
- 2026-05-05 | QA | Tests unitarios escritos y verificados para CAs 1-5. 22 tests nuevos en config.rs y providers.rs. CAs 6-7 son checks operacionales (build/test). Placeholder stubs en AgentsConfig serán reemplazados por el Developer. Decisión documentada en .regista/decisions/STORY-002-qa-tests-2026-05-05.md. Ready → Tests Ready.
- 2026-05-05 | Dev | Migración completada: implementados provider_for_role() y skill_for_role() como métodos de AgentsConfig en config.rs con la lógica original. Eliminadas las funciones libres de src/infra/providers.rs. Actualizados todos los callers (pipeline.rs, plan.rs, validate.rs) y tests antiguos para usar cfg.agents.provider_for_role() y cfg.agents.skill_for_role(). cargo build, cargo clippy, cargo fmt y cargo test (346 tests) pasan sin errores. Decisión documentada en .regista/decisions/STORY-002-dev-implement-2026-05-05.md. Tests Ready → In Review.
- 2026-05-05 | Reviewer | DoD técnico verificado: cargo build compila sin warnings, cargo test pasa 357 tests (346 unitarios + 11 arquitectura), 0 fallos, cargo clippy sin warnings, cargo fmt correcto. CAs 1-7 satisfechos. Sin regresiones. In Review → Business Review. Decisión documentada en .regista/decisions/STORY-002-reviewer-2026-05-05.md.
- 2026-05-05 | PO | Validación de negocio OK. Todos los CAs satisfechos: provider_for_role() y skill_for_role() son métodos de AgentsConfig en config.rs, las funciones libres fueron eliminadas de infra/providers.rs, todos los callers (plan, pipeline, validate) usan cfg.agents.*. 357 tests pasan (0 fallos), cargo build sin warnings. Valor de negocio cumplido: acoplamiento infra→config eliminado, arquitectura de capas restaurada. Sin regresiones. Business Review → Done. Decisión documentada en .regista/decisions/STORY-002-po-validate-2026-05-05.md.
