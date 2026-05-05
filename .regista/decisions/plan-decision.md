# Plan Decision: Logs transparentes — streaming, trazabilidad y tracking de tokens

**Fecha**: 2026-05-05
**Fuente**: `specs/spec-logs-transparentes.md`
**Versión analizada**: v0.9.0

---

## Decisiones de descomposición

### 1. Agrupación en 4 épicas

La especificación cubre 7 áreas de cambio (config, domain, infra/agent, infra/daemon, cli, app/pipeline, app/health). Se agruparon en 4 épicas siguiendo el principio de **capas**: infraestructura base → infraestructura avanzada → CLI/UX → integración final.

| Épica | Módulos | Motivación |
|-------|---------|------------|
| EPIC-07: Infraestructura Base | config.rs, domain/state.rs, infra/agent.rs (solo parseo) | Estructuras de datos y funciones puras que todo lo demás necesita. Sin efectos secundarios complejos. |
| EPIC-08: Streaming y Daemon | infra/agent.rs (invoke_once), infra/daemon.rs (follow) | Cambios de infraestructura con I/O real (pipes, archivos). Más riesgo, más valor visible. |
| EPIC-09: CLI y Visibilidad | cli/args.rs, cli/handlers.rs | Capa de usuario: flags y header. Depende de EPIC-07 y EPIC-08 para ser funcional, pero se puede maquetar antes. |
| EPIC-10: Pipeline y Reportes | app/pipeline.rs, app/health.rs | Integración final. Orquesta todas las piezas. Solo se implementa cuando EPIC-07, EPIC-08, y EPIC-09 están listos. |

### 2. Orden de implementación recomendado

1. **EPIC-07** (Infraestructura Base) — STORY-019, STORY-020, STORY-021 son independientes y se pueden paralelizar. Sientan las bases.
2. **EPIC-08** (Streaming y Daemon) — STORY-022 y STORY-023, también independientes entre sí.
3. **EPIC-09** (CLI y Visibilidad) — STORY-024 depende de STORY-022, STORY-025 depende de STORY-023, STORY-026 depende de STORY-019. Se puede empezar tras EPIC-07 + EPIC-08.
4. **EPIC-10** (Pipeline y Reportes) — STORY-027 integra todo (depende de EPIC-07 completo + STORY-022 + STORY-026). STORY-028 cierra con health report (depende de STORY-027).

### 3. Grafo de dependencias entre historias

```
STORY-019 (model config) ─────────────────────────────┐
STORY-020 (TokenCount) ─────────────────────────────┐ │
STORY-021 (parse_token_count) ─────────────────────┐ │ │
                                                     │ │ │
STORY-022 (streaming) ──┐                           │ │ │
STORY-023 (follow) ──┐  │                           │ │ │
                      │  │                           │ │ │
STORY-024 (--compact)◄┘  │                           │ │ │
STORY-025 (--tail) ◄─────┘                           │ │ │
STORY-026 (header) ◄─────────────────────────────────┘ │ │
                                                        │ │
STORY-027 (pipeline integración) ◄──────────────────────┴─┴── STORY-022, STORY-026
                                                          │
STORY-028 (health tokens) ◄───────────────────────────────┘
```

### 4. Criterios de atomicidad

Cada historia:
- Modifica como máximo 2-3 archivos
- Se puede implementar en una sesión de agente
- Tiene criterios de aceptación verificables con `cargo test`, `cargo check`, o `cargo build`
- Entrega valor por sí misma: STORY-019 expone modelo aunque nada más lo use; STORY-021 se puede probar con strings sintéticos; STORY-023 mejora `regista logs` incluso sin el resto

### 5. Lo que NO se incluye (out of scope)

- TUI interactiva para visualización de logs (eso es #11 del roadmap)
- Persistencia de tokens entre sesiones (solo en memoria durante la sesión actual)
- Integración con APIs de pricing de proveedores (cost tracking real es #12 del roadmap)
- Migración de logs existentes al nuevo formato
- Rotación automática de archivos de log
