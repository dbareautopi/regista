# 04 — Workflow configurable

> ⚠️ **Esta feature está implementada vía `spartito`.**  
> El diseño original de #04 evolucionó a un crate independiente compartido
> entre `regista` y `mezzala`. Este documento queda como referencia histórica
> y explicación conceptual. El diseño definitivo está en
> [`mezzala/docs/spec-spartito.md`](../../mezzala/docs/spec-spartito.md).

## 🎯 Objetivo

Permitir que los equipos definan sus propios estados, transiciones y
asignaciones de agentes en `.regista/config.toml`, en lugar de usar el workflow
fijo de 9 estados y 14 transiciones.

## 📍 Posición en el roadmap

**Fase 0 (fundacional)** — Spartito es el prerequisito arquitectónico para
todo el resto. Se implementa ANTES de cualquier otra feature porque define
el contrato que regista y mezzala comparten. Reemplaza a la antigua Fase 5.

## ❓ Problema que resuelve

Los 14 transitions en `state.rs` eran canónicas e inmutables *por diseño*.
El argumento era noble (consistencia, previsibilidad), pero limitaba la adopción:

- Equipos que no usan QA automatizado (sin `TestsReady`).
- Equipos con fase de UAT adicional (nuevo estado `UatReview`).
- Equipos que fusionan roles (PO + Reviewer misma persona).
- Flujos más simples: `Draft → In Progress → In Review → Done`.

## ✅ Solución: Spartito

En lugar de implementar workflow configurable como una feature dentro de
regista, se extrajo a un **crate independiente** (`spartito`) que ambos
proyectos (regista y mezzala) importan como dependencia.

Ver el diseño completo en: [`mezzala/docs/spec-spartito.md`](../../mezzala/docs/spec-spartito.md)

### Workflow por defecto (compatible hacia atrás)

Si no se especifica `[workflow]` en el TOML, se usa `CanonicalWorkflow`
(14 transiciones fijas). Cero breaking change.

### Workflow personalizado en `.regista/config.toml`

```toml
[workflow]
states = ["Draft", "Ready", "In Progress", "In Review", "Done", "Blocked", "Failed"]

[[workflow.transitions]]
from = "Draft"
to   = "Ready"
by   = "product_owner"

[[workflow.transitions]]
from = "Ready"
to   = "In Progress"
by   = "developer"

[[workflow.transitions]]
from = "In Progress"
to   = "In Review"
by   = "developer"

# Bifurcación: Reviewer elige entre 3 caminos
[[workflow.transitions]]
from = "In Review"
to   = "Business Review"
by   = "reviewer"
description = "Aprueba DoD técnico"

[[workflow.transitions]]
from = "In Review"
to   = "QA Verification"
by   = "reviewer"
description = "Necesita verificación manual de QA"

[[workflow.transitions]]
from = "In Review"
to   = "In Progress"
by   = "reviewer"
description = "Rechazo técnico"

# Transiciones automáticas (orchestrator)
[[workflow.transitions]]
from = "*"
to   = "Blocked"
by   = "orchestrator"
guard = "has_unresolved_dependencies"

[[workflow.transitions]]
from = "*"
to   = "Failed"
by   = "orchestrator"
guard = "max_reject_cycles_exceeded"
```

### Bifurcaciones

La gran novedad sobre el diseño original de #04 es el soporte de
**bifurcaciones**: un estado puede tener múltiples destinos, y es el
**agente** quien elige cuál tomar. El orquestador solo valida que la
transición elegida sea legal.

```rust
// spartito::workflow
pub trait Workflow: Sync {
    fn transitions_from(&self, state: &Status) -> Vec<&Transition>;
    // Si devuelve N > 1 entradas → bifurcación → el agente elige
}
```

### Tipos fundamentales

| Tipo | Antes (#04 original) | Ahora (spartito) |
|------|---------------------|-------------------|
| `Status` | `String` o `SmolStr` | Newtype `Status(String)` con constantes `&'static str` |
| `Actor` | `String` | Newtype `Actor(String)` con roles canónicos |
| `Transition` | Struct desde TOML | Struct con `Guard` opcional para automáticas |
| `Guard` | String suelto | Enum cerrado: `HasUnresolvedDependencies`, `AllDependenciesDone`, `MaxRejectCyclesExceeded`, `Custom(String)` |

## 📊 Impacto en regista

| Archivo | Acción |
|---------|--------|
| `domain/workflow.rs` | **Eliminado** (vive en spartito) |
| `domain/state.rs` | **Wrapper**: `pub use spartito::{Status, Actor, Transition};` + SharedState |
| `domain/story.rs` | Delega parseo a `spartito::story_format` |
| `domain/deadlock.rs` | Usa `&dyn Workflow` para consultas de estado |
| `domain/prompts.rs` | Soporta bifurcaciones: `transitions_from()` en la generación del prompt |
| `config.rs` | Añade `workflow: Option<spartito::config::WorkflowConfig>` |
| `app/pipeline.rs` | `Box<dyn Workflow>` + bifurcaciones en `process_story()` |
| `app/board.rs` | `workflow.state_order()` para columnas dinámicas |
| `app/validate.rs` | Validación de workflow custom (sin ciclos, sin huérfanos) |
| `tests/architecture.rs` | Actualizar reglas de capas |

## 🔗 Relacionado con

- **[`spartito spec`](../../mezzala/docs/spec-spartito.md)** — ⭐ Diseño definitivo. Source of truth.
- [`20-multi-provider.md`](./20-multi-provider.md) — ✅ Implementado. Cada rol en el workflow custom se asigna a un provider.
- [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) — ✅ Implementado. Los prompts genéricos permiten cualquier workflow.
- [`10-cross-story-context.md`](./10-cross-story-context.md) — Los agentes necesitan contexto de dependencias incluso con workflows custom.
- [`05-validate.md`](./05-validate.md) — ✅ Implementado. Validación del workflow definido por el usuario.
