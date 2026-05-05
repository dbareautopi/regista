# STORY-002 — QA — Verificación de tests — 2026-05-05

## Resultado
✅ Tests verificados — cobertura completa de CAs 1-5.

---

## Análisis de cobertura

| CA | Descripción | # Tests | Estado |
|----|-------------|---------|--------|
| CA1 | `provider_for_role()` como método de `AgentsConfig` | 6 | ✅ cubierto |
| CA2 | `skill_for_role()` como método de `AgentsConfig` | 8 | ✅ cubierto |
| CA3 | `all_roles()` existe | 2 | ✅ cubierto |
| CA4 | Funciones libres eliminadas de `providers.rs` | 3 | ✅ cubierto |
| CA5 | Callers usan `cfg.agents.provider_for_role(...)` | 3 | ✅ cubierto |
| CA6 | `cargo build` sin warnings | — | ⚙️ operacional (post-implementación) |
| CA7 | `cargo test` pasa | — | ⚙️ operacional (post-implementación) |

**Total: 22 tests unitarios (19 en `config.rs` + 3 en `providers.rs`)**

---

## Detalle por CA

### CA1 — `provider_for_role(&self, role) -> String` (`config.rs`)
- `story002_ca1_method_exists_provider_for_role` — método existe y retorna "pi" por defecto
- `story002_ca1_inherits_global_provider` — hereda del global cuando no hay override
- `story002_ca1_role_specific_provider_overrides_global` — override por rol funciona
- `story002_ca1_unknown_role_returns_global` — rol desconocido → global
- `story002_ca1_all_canonical_roles_return_pi_by_default` — 4 roles canónicos → "pi"
- `story002_ca1_each_canonical_provider_per_role` — cada provider canónico asignable por rol

### CA2 — `skill_for_role(&self, role) -> String` (`config.rs`)
- `story002_ca2_method_exists_skill_for_role` — método existe
- `story002_ca2_pi_convention_skill_paths` — convención `.pi/skills/<rol>/SKILL.md`
- `story002_ca2_claude_convention_skill_paths` — convención `.claude/agents/<rol>.md`
- `story002_ca2_codex_convention_skill_paths` — convención `.agents/skills/<rol>/SKILL.md`
- `story002_ca2_opencode_convention_skill_paths` — convención `.opencode/agents/<rol>.md`
- `story002_ca2_explicit_skill_path_overrides_convention` — path explícito sobreescribe
- `story002_ca2_unknown_role_returns_empty_string` — rol desconocido → ""
- `story002_ca2_mixed_provider_and_explicit_skill` — mezcla provider+skill explícito

### CA3 — `all_roles()` (`config.rs`)
- `story002_ca3_all_roles_returns_four_canonical_roles` — 4 roles exactos
- `story002_ca3_all_roles_is_iterable` — elementos iterables

### CA4 — Funciones libres eliminadas (`providers.rs`)
- `story002_ca4_provider_for_role_not_a_free_function` — usa método, no free function
- `story002_ca4_skill_for_role_not_a_free_function` — usa método, no free function
- `story002_ca4_no_free_function_conflict_with_agents_config_methods` — sin conflictos

### CA5 — Patrones de caller (`config.rs`)
- `story002_ca5_caller_pattern_provider_and_skill_for_role` — patrón básico
- `story002_ca5_caller_pattern_mixed_providers` — providers mixtos global+per-rol
- `story002_ca5_caller_pattern_plan_po_role` — patrón de plan.rs con opencode

---

## Lo que el Developer debe implementar

1. Reemplazar stubs `unimplemented!()` en `AgentsConfig::provider_for_role()` y `AgentsConfig::skill_for_role()` con la lógica real (actualmente en `providers.rs` líneas 308–345)
2. Eliminar las funciones libres `provider_for_role` y `skill_for_role` de `providers.rs`
3. Actualizar callers en `pipeline.rs`, `plan.rs`, `validate.rs`: cambiar `providers::provider_for_role(&cfg.agents, ...)` → `cfg.agents.provider_for_role(...)`
4. Actualizar tests existentes pre-STORY-002 en `config.rs` que usan `providers::provider_for_role(...)` / `providers::skill_for_role(...)` (líneas ~371-506)
5. Verificar CA6: `cargo build` sin warnings
6. Verificar CA7: `cargo test` todos los tests en verde (22 STORY-002 + existentes)

---

## Notas
- Los tests NO pueden ejecutarse actualmente porque los stubs usan `unimplemented!()` — esto es esperado en TDD.
- No se ejecutó `cargo build` ni `cargo test` porque el orquestador indicó que el Developer lo verificará.
- Los 3 tests existentes en `providers.rs` que fueron actualizados para STORY-002 también dependen de los stubs.
