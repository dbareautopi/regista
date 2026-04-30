# 04 — Workflow configurable

## 🎯 Objetivo

Permitir que los equipos definan sus propios estados, transiciones y
asignaciones de agentes en `.regista.toml`, en lugar de usar el workflow
fijo de 9 estados y 14 transiciones.

## ❓ Problema actual

Los 14 transitions en `state.rs` son canónicas e inmutables *por diseño*.
El argumento es noble (consistencia, previsibilidad), pero limita la adopción:

- Equipos que no usan QA automatizado (sin `TestsReady`).
- Equipos con fase de UAT adicional (nuevo estado `UatReview`).
- Equipos que fusionan roles (PO + Reviewer misma persona).
- Flujos más simples: `Draft → In Progress → In Review → Done`.

Cada equipo tiene su proceso. Imponer uno fijo garantiza que muchos no lo usen.

## ✅ Solución propuesta

### Workflow por defecto (compatible hacia atrás)

Si no se especifica `[workflow]` en el TOML, se usa el workflow canónico
actual. Cero breaking change.

### Workflow personalizado en `.regista.toml`

```toml
[workflow]
states = ["Draft", "Ready", "In Progress", "In Review", "Done", "Blocked", "Failed"]

[workflow.agents]
product_owner = ".pi/skills/po/SKILL.md"
developer     = ".pi/skills/dev/SKILL.md"
reviewer      = ".pi/skills/rev/SKILL.md"

# Transiciones: de → a ejecutada por
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

[[workflow.transitions]]
from = "In Review"
to   = "Done"
by   = "reviewer"

[[workflow.transitions]]
from = "In Review"
to   = "In Progress"
by   = "reviewer"

# Transiciones automáticas las gestiona el orquestador
[[workflow.transitions]]
from = "*"
to   = "Blocked"
by   = "orchestrator"
condition = "has_unresolved_dependencies"

[[workflow.transitions]]
from = "*"
to   = "Failed"
by   = "orchestrator"
condition = "max_reject_cycles_exceeded"
```

### Impacto en el código

- `Status` deja de ser un enum y pasa a ser `String` o un `SmolStr` con un
  conjunto validado en runtime.
- `Transition` se carga desde TOML en vez de ser `const`.
- `prompts.rs` necesita volverse genérico: prompts basados en el rol del
  agente, no en transiciones específicas.
- Validación de workflow en `config.rs`: sin ciclos de transiciones inválidas,
  estados huérfanos, etc.

## 📝 Notas de implementación

- El mayor riesgo es el impacto en `prompts.rs`. Si las transiciones son
  arbitrarias, los prompts deben ser genéricos ("haz tu trabajo de [rol] para
  mover [historia] de [A] a [B]").
- Los roles (`product_owner`, `developer`, etc.) serían los conceptos
  estables, no los estados.
- Esto es **el cambio más grande** de todo el roadmap. Toca `state.rs`,
  `config.rs`, `prompts.rs`, `orchestrator.rs`.
- Estrategia: implementar con feature flag `custom-workflow` para no romper
  el código existente durante el desarrollo.

## 🔗 Relacionado con

- [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) — necesario como
  prerequisito para workflows custom.
- [`05-validate.md`](./05-validate.md) — validación del workflow definido por
  el usuario.
