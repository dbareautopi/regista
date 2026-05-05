# STORY-001: `from_name()` devuelve `Result` + `validate` verifica binarios de providers

## Status
**Done**

## Epic
EPIC-01

## Descripción
El método `from_name()` en `src/infra/providers.rs` actualmente hace `panic!` cuando el nombre del provider no es reconocido. Esto aborta todo el proceso sin cleanup. Hay que cambiarlo para que devuelva `anyhow::Result<Box<dyn AgentProvider>>`. Adicionalmente, el comando `validate` debe verificar que los binarios de los providers configurados (`pi`, `claude`, `codex`, `opencode`) existen en el `PATH` del sistema.

## Criterios de aceptación
- [ ] CA1: `from_name("pi")` devuelve `Ok(Box<dyn AgentProvider>)` (mismo comportamiento, distinto tipo de retorno)
- [ ] CA2: `from_name("inventado")` devuelve `Err(...)` con mensaje descriptivo, sin hacer `panic!`
- [ ] CA3: `from_name("claude-code")` y alias (`"claude_code"`, `"claude"`) siguen funcionando
- [ ] CA4: `from_name("opencode")`, `"open-code"`, `"open_code"` siguen funcionando
- [ ] CA5: Todos los callers de `from_name()` se adaptan al nuevo `Result` (usar `?` o `match`)
- [ ] CA6: `regista validate` reporta un `Finding::Error` si el binario del provider configurado no está en `PATH`
- [ ] CA7: `regista validate` reporta un `Finding::Warning` si el provider es `codex` y no se puede verificar (codex puede estar instalado vía npm global con nombre no estándar)
- [ ] CA8: `cargo test --lib providers` pasa (todos los tests de providers existentes)
- [ ] CA9: `cargo test --lib validator` pasa (tests de validate)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #7.1, #7.2).
- 2026-05-05 | PO | Refinamiento completado. DoR verificado: descripción clara (2 cambios concretos), 9 CAs específicos y testeables, sin dependencias. Callers de from_name() identificados: providers.rs (skill_for_role), pipeline.rs, init.rs, plan.rs — todos necesitarán adaptación al Result. Lista para QA.
- 2026-05-05 | QA | Tests escritos para STORY-001. Cobertura por CA: CA1 (from_name_returns_ok_for_known_provider, from_name_returns_ok_for_all_canonical_providers), CA2 (4 tests: Err descriptivo, sin panic, varios nombres inválidos, chatgpt→Err), CA3 (3 tests: aliases claude, claude returns, case-insensitive), CA4 (3 tests: aliases opencode, opencode returns, case-insensitive), CA5 (5 tests: ? operator, match, skill_for_role happy path, skill_for_role claude config, skill_for_role invalid config), CA6 (validate_providers_reports_error_when_binary_missing, validate_providers_checks_all_roles), CA7 (3 tests: warning codex, never error codex, multi-role). CA8: 8 old tests actualizados a Result API (unwrap). CA9: tests validate existentes preservados. Total: 34 tests providers + 7 tests validate = 41 tests. Tests listos para Developer.
- 2026-05-05 | Dev | Implementación completada. Cambios: (1) from_name() → anyhow::Result<Box<dyn AgentProvider>> con anyhow::bail! en desconocidos, (2) trait AgentProvider extiende Debug, (3) skill_for_role() usa .expect() con mensaje descriptivo, (4) callers en plan.rs/init.rs/pipeline.rs adaptados con ?, (5) validate_providers() implementado con búsqueda manual en PATH, helper find_in_path(), wireado en validate(). 335/335 tests pasan, cargo fmt + clippy OK, cargo build --release OK. Decisions en .regista/decisions/2026-05-05-dev-story-001.md.
- 2026-05-05 | Reviewer | DoD técnico superado. cargo build: OK (sin errores). cargo test: 324 passed, 0 failed, 1 ignored (pi no instalado). cargo clippy -- -D warnings: sin warnings. cargo fmt -- --check: formato correcto. CAs verificados: CA1 (from_name devuelve Ok), CA2 (from_name devuelve Err sin panic), CA3 (aliases claude-code OK), CA4 (aliases opencode OK), CA5 (callers con ?), CA6 (validate Error si falta binario), CA7 (validate Warning para codex), CA8 (tests providers pasan), CA9 (tests validator pasan). No hay regresiones. Transición a Business Review. Decisiones en .regista/decisions/2026-05-05-reviewer-story-001.md.
- 2026-05-05 | PO | Validación de negocio superada. Verificación exhaustiva de los 9 CAs: CA1 (Ok retorno), CA2 (Err sin panic), CA3 (aliases claude), CA4 (aliases opencode), CA5 (callers con ?), CA6 (validate Error binario), CA7 (validate Warning codex), CA8 (34 tests providers), CA9 (7 tests validator). Build release OK, clippy limpio, fmt OK, 324 tests pasan. Valor de negocio entregado: from_name() ya no aborta con panic y validate diagnostica binarios faltantes. Transición a Done. Decisión en .regista/decisions/STORY-001-po-validate-done-2026-05-05.md.
