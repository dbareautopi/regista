# EPIC-V10-05: Limpieza y Migración

## Objetivo
Eliminar todo el código obsoleto de v0.x: el sistema de providers CLI (`infra/providers.rs`), la invocación de agentes vía procesos (`infra/agent.rs`), el dominio hardcodeado (`domain/story.rs`, tipos `Status`/`Actor`/`Transition` fijos), y los 7 prompts hardcodeados de `domain/prompts.rs`. Actualizar dependencias en `Cargo.toml` y verificar que la arquitectura de capas sigue respetándose.

## Alcance
- Eliminar `infra/providers.rs` y `infra/agent.rs`
- Eliminar `domain/story.rs` y los tipos fijos `Status`/`Actor`/`Transition`
- Quitar `ureq` y `spartito` de `Cargo.toml`, añadir `reqwest` con `rustls-tls`
- Actualizar `tests/architecture.rs` para las nuevas capas
- `cargo build`, `cargo test`, `cargo clippy` pasan limpios

## Historias
- STORY-V10-019: Eliminar providers CLI y agent.rs
- STORY-V10-020: Eliminar dominio hardcodeado y dependencia spartito

## Dependencias
- EPIC-V10-04 (todo el código nuevo debe estar funcionando antes de eliminar el viejo)

## Rama
`rework`

## Estimación
1 semana
