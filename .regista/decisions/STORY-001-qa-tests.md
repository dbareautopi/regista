# STORY-001 — QA Decision: Estrategia de tests

**Fecha**: 2026-05-05  
**Actor**: QA  
**Historia**: STORY-001: `from_name()` devuelve `Result` + `validate` verifica binarios de providers

## Decisiones de testing

### 1. Actualización de tests antiguos (CA8)

Los tests pre-existentes en `src/infra/providers.rs` llamaban a `from_name()` asumiendo que retorna `Box<dyn AgentProvider>`. Tras la migración a `Result<Box<dyn AgentProvider>>`, estos tests no compilarían.

**Decisión**: Actualizar los 8 tests antiguos añadiendo `.unwrap()` en cada llamada a `from_name()`. El test `from_name_panics_on_unknown` se renombra a `from_name_returns_err_on_unknown` y elimina `#[should_panic]`, verificando en su lugar que retorna `Err`.

Archivos modificados:
- `src/infra/providers.rs`: 8 tests antiguos actualizados

### 2. Tests STORY-001 existentes (CA1-CA5, CA6-CA7)

Los tests STORY-001 ya estaban pre-escritos tanto en `providers.rs` como en `validate.rs`. Se verificó que cubren todos los criterios de aceptación:

| CA | Tests | Archivo |
|----|-------|---------|
| CA1 | `from_name_returns_ok_for_known_provider`, `from_name_returns_ok_for_all_canonical_providers` | providers.rs |
| CA2 | `from_name_returns_err_for_unknown_provider`, `from_name_err_message_is_descriptive`, `from_name_returns_err_for_various_unknown_names`, `from_name_returns_err_on_unknown` | providers.rs |
| CA3 | `from_name_claude_aliases_return_ok`, `from_name_returns_claude`, `from_name_aliases_claude` | providers.rs |
| CA4 | `from_name_opencode_aliases_return_ok`, `from_name_returns_opencode`, `from_name_aliases_opencode` | providers.rs |
| CA5 | `from_name_result_works_with_question_mark_operator`, `from_name_result_handled_with_match`, `skill_for_role_uses_result_from_from_name`, `skill_for_role_works_with_claude_provider`, `skill_for_role_handles_invalid_provider_in_config` | providers.rs |
| CA6 | `validate_providers_reports_error_when_binary_missing`, `validate_providers_checks_all_roles` | validate.rs |
| CA7 | `validate_providers_reports_warning_for_codex`, `validate_providers_no_warning_when_codex_is_installed`, `validate_providers_checks_all_roles` | validate.rs |

### 3. Tests adicionales de borde

Se añadieron 2 tests para cubrir casos límite no contemplados:

- `from_name_result_is_case_insensitive`: Verifica que la insensibilidad a mayúsculas/minúsculas funciona con la API Result (CLAUDE, Codex, OPENCODE, Pi, clAuDe-CoDe).
- `skill_for_role_handles_invalid_provider_in_config`: Verifica que `skill_for_role` no causa undefined behavior cuando la config referencia un provider desconocido. Usa `catch_unwind` para ser tolerante tanto si el Developer decide hacer `.expect()` (fail-fast) como si propaga el error con `Result`.

### 4. Nota para el Developer

- Los tests NO compilan actualmente porque `from_name()` todavía retorna `Box<dyn AgentProvider>` en lugar de `Result<Box<dyn AgentProvider>>`.
- `validate_providers()` es un placeholder (TODO). El Developer debe implementar la lógica de chequeo de binarios en PATH.
- `validate()` no llama a `validate_providers()` — el Developer debe añadir la integración.
- Los tests de `skill_for_role` asumen que la firma podría cambiar a `Result<String>`. El Developer debe ajustar las aserciones según la implementación final.

### 5. Lo que NO se hizo

- No se crearon módulos nuevos.
- No se generaron fake providers ni infraestructura de testing.
- No se implementó lógica de negocio (cambios en `from_name`, `validate_providers`).
- No se ejecutó build ni tests (responsabilidad del Developer).
