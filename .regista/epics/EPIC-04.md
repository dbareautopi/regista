# EPIC-04: Preparación Async para Paralelismo (#01)

## Descripción
Migrar la capa de invocación de agentes de síncrono (con busy-polling `thread::sleep`) a `tokio`, wrappear el estado compartido del orchestrator (`reject_cycles`, `story_iterations`, `story_errors`) en `Arc<RwLock<>>`, y convertir el loop principal del pipeline a async. Estos son los tres prerequisitos técnicos indispensables para implementar paralelismo (#01).

Cubre los hallazgos #2.4, #10.2 y #10.3 de la auditoría.

## Historias
- STORY-010: Migrar `agent.rs` a `tokio` (eliminar busy-polling)
- STORY-011: Estado compartido con `Arc<RwLock<>>`
- STORY-012: Migrar `pipeline.rs` a async (process_story, loop principal)
