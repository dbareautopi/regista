# STORY-002: Reubicar `provider_for_role` / `skill_for_role` a `AgentsConfig`

## Status
**Tests Ready**

## Epic
EPIC-01

## Descripción
Las funciones `provider_for_role()` y `skill_for_role()` residen actualmente en `src/infra/providers.rs`, creando un acoplamiento incorrecto desde la capa `infra` hacia `config`. Estas funciones dependen de `crate::config::AgentsConfig` y deben ser métodos de `AgentsConfig` en `src/config.rs`. La capa `infra/providers.rs` debe contener solo el trait `AgentProvider` y sus implementaciones.

## Criterios de aceptación
- [ ] CA1: `AgentsConfig` tiene el método `pub fn provider_for_role(&self, role: &str) -> String`
- [ ] CA2: `AgentsConfig` tiene el método `pub fn skill_for_role(&self, role: &str) -> String`
- [ ] CA3: `AgentsConfig` tiene el método `pub fn all_roles() -> impl Iterator<Item = &'static str>` (ya existe, verificar)
- [ ] CA4: Las funciones `provider_for_role` y `skill_for_role` se eliminan de `src/infra/providers.rs`
- [ ] CA5: Todos los callers (orchestrator, pipeline, validator) usan `cfg.agents.provider_for_role(...)` en lugar de `providers::provider_for_role(&cfg.agents, ...)`
- [ ] CA6: `cargo build` compila sin warnings
- [ ] CA7: `cargo test` pasa todos los tests existentes

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #11.2).
- 2026-05-05 | PO | Refinamiento: historia validada contra DoR. Descripción clara, 7 CAs testeables, sin dependencias. Decisión documentada en .regista/decisions/STORY-002-po-refinement-2026-05-05.md. Draft → Ready.
- 2026-05-05 | QA | Tests unitarios escritos y verificados para CAs 1-5. 22 tests nuevos en config.rs y providers.rs. CAs 6-7 son checks operacionales (build/test). Placeholder stubs en AgentsConfig serán reemplazados por el Developer. Decisión documentada en .regista/decisions/STORY-002-qa-tests-2026-05-05.md. Ready → Tests Ready.
