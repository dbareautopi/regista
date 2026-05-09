# STORY-V10-019: Eliminar providers CLI y agent.rs

## Status
**Draft**

## Epic
EPIC-V10-05

## Descripción
Eliminar `infra/providers.rs` y `infra/agent.rs`, los dos módulos que implementaban la invocación de agentes a través de binarios CLI externos (`pi`, `claude`, `codex`, `opencode`). Toda esa funcionalidad ha sido reemplazada por `infra/llm/` (cliente HTTP nativo) y el nuevo `app/pipeline.rs` (que invoca `llm.chat()` directamente).

También se elimina la dependencia `ureq` de `Cargo.toml` (ya no se usa para consultar crates.io en `update.rs`, que se migra a `reqwest`) y se añade `reqwest` con `rustls-tls`.

**Valor de negocio**: Reduce la superficie de código, elimina la dependencia de binarios externos, y simplifica el mantenimiento.

## Criterios de aceptación
- [ ] CA1: Los archivos `infra/providers.rs` y `infra/agent.rs` son eliminados. Todos los imports que los referenciaban (`use crate::infra::providers`, `use crate::infra::agent`) se eliminan de `app/pipeline.rs`, `app/plan.rs`, `cli/handlers.rs`, y tests
- [ ] CA2: `Cargo.toml` elimina la dependencia `ureq` y añade `reqwest = { version = "0.12", features = ["json", "rustls-tls"] }`. `app/update.rs` se adapta para usar `reqwest` en lugar de `ureq`
- [ ] CA3: `cargo build`, `cargo test` (unitarios), y `cargo clippy -- -D warnings` pasan sin errores ni warnings

## Dependencias
- Bloqueado por: STORY-V10-010, STORY-V10-015, STORY-V10-018

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 5
