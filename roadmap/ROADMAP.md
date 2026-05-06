# 🗺️ regista — Roadmap

Ideas, mejoras y funcionalidades pendientes para convertir `regista` en una
herramienta de desarrollo real, adoptable por equipos.

Cada entrada tiene su propio documento con descripción detallada, motivación,
y notas de implementación.

---

## 🔴 Crítica — Bloquea adopción real

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 2 | **Salida JSON + CI/CD**: reportes estructurados, exit codes, integración con pipelines | [`02-salida-json-ci-cd.md`](./02-salida-json-ci-cd.md) | ✅ Implementado |
| 3 | **Dry-run**: simular qué haría el orquestador sin ejecutar agentes | [`03-dry-run.md`](./03-dry-run.md) | ✅ Implementado |
| 4 | **Workflow configurable**: estados y transiciones definibles en `.regista/config.toml` | [`04-workflow-configurable.md`](./04-workflow-configurable.md) | ✍️ Diseñado — implementado vía `spartito` |
| 20 | **🆕 Multi-provider**: pi, Claude Code, Codex, OpenCode | [`20-multi-provider.md`](./20-multi-provider.md) | ✅ Implementado |

---

## 🟠 Alta — Duele en el día a día

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 5 | **`regista validate`**: chequeo pre-vuelo de historias, skills, dependencias | [`05-validate.md`](./05-validate.md) | ✅ Implementado |
| 6 | **`regista init`**: scaffolding de proyecto nuevo (config + skills + historia ejemplo) | [`06-init-scaffold.md`](./06-init-scaffold.md) | ✅ Implementado |
| 7 | **Checkpoint / resume**: reanudar pipeline interrumpido sin reprocesar todo | [`07-checkpoint-resume.md`](./07-checkpoint-resume.md) ✅ | Medio |
| 8 | **Feedback rico de agentes**: capturar y usar stdout/stderr de agentes fallidos | [`08-feedback-agentes.md`](./08-feedback-agentes.md) ✅ | Bajo |

---

## 🟡 Media — Mejora la experiencia

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 9 | **Prompts agnósticos al stack**: desacoplar referencias a herramientas (cargo, npm) | [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) ✅ | Bajo |
| 10 | **Conciencia cross-story**: agentes reciben contexto de historias relacionadas | [`10-cross-story-context.md`](./10-cross-story-context.md) ✍️ | Medio |
| 11 | **TUI / dashboard**: visualización en vivo del progreso del pipeline | [`11-tui-dashboard.md`](./11-tui-dashboard.md) | Medio |
| 12 | **Cost tracking**: estimación y límite de gasto en llamadas LLM | [`12-cost-tracking.md`](./12-cost-tracking.md) | Medio |
| 1 | **🕐 Paralelismo**: ejecutar múltiples historias independientes simultáneamente | [`01-paralelismo.md`](./01-paralelismo.md) | Alto |
| 21 | **🐳 Dockerización**: `init --docker` genera Dockerfile + docker-compose con el provider elegido | [`21-docker.md`](./21-docker.md) 💡 | Medio-bajo |

## 🟢 Generación de backlog — Automatizar la creación de historias

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 13 | **`regista plan`**: generar historias desde un documento de requisitos, con bucle de validación de dependencias | [`13-groom-generacion-historias.md`](./13-groom-generacion-historias.md) | ✅ Implementado |
| 14 | **`plan --from-dir`**: generar desde un directorio de specs por feature | [`14-groom-from-dir.md`](./14-groom-from-dir.md) | Bajo |
| 15 | **`plan --interactive`**: el PO entrevista al usuario para extraer requisitos | [`15-groom-interactive.md`](./15-groom-interactive.md) | Medio |
| 22 | **`plan --append`**: generación incremental consciente de historias existentes, sin pisar el backlog previo | [`22-plan-append.md`](./22-plan-append.md) ✍️ | Medio |

## 🔵 v0.2.0 — Calidad de vida (implementado)

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 16 | **Migración a `.regista/`**: todos los paths bajo `.regista/` en vez de dispersos | — | ✅ Implementado |
| 17 | **Comando `help`**: `regista help` lista todos los comandos y flags | — | ✅ Implementado |
| 18 | **Auto-escalado `max_iterations`**: `max(10, stories × 6)` cuando se deja en 0 | — | ✅ Implementado |
| 19 | **Exit code 3 + `stop_reason`**: diferenciar pipeline completo de parada temprana | — | ✅ Implementado |

---

## 📐 Criterios de priorización

1. **Impacto en adopción**: ¿sin esto un equipo rechazaría la herramienta?
2. **Esfuerzo estimado**: ¿es un cambio localizado o toca toda la arquitectura?
3. **Valor incremental**: ¿se puede entregar parcialmente y ya aporta valor?

Las entradas marcadas como críticas son las que *impiden* que un equipo use
`regista` en su día a día. El resto son mejoras que aumentan la calidad de vida.

---

## 🗓️ Orden de implementación (mayo 2026)

```
Fase 0 (FUNDACIONAL — AHORA) ──── 🆕 Spartito: crate de contrato compartido
                                     ├── Crear spartito en workspace mezzala (~115 tests)
                                     ├── Migrar regista a usar spartito (adaptar ~357 tests)
                                     └── Esfuerzo: medio-alto (~500 líneas nuevas + migración)
                                     │
                                     │  ⚠️ Spartito REEMPLAZA a #04 (workflow configurable).
                                     │  Cuando spartito esté en crates.io, #04 estará completado.
                                     │
Fase 1 (abstracción fundacional) ── 🆕 #20 multi-provider (Claude Code, Codex, OpenCode…)
                                     ├── Trait AgentProvider (devuelve Vec<String>, agnóstico a sync/async)
                                     ├── PiProvider, ClaudeCodeProvider, CodexProvider, OpenCodeProvider
                                     └── Esfuerzo: medio (~215 líneas) ✅ IMPLEMENTADO

Fase 2 (prerrequisito natural) ──── #09 prompts agnósticos al stack
                                     ├── Templates de prompt con vars de stack
                                     └── Esfuerzo: bajo (~80 líneas) ✅ IMPLEMENTADO

Fase 3 (quick win) ──────────────── #14 plan --from-dir
                                     ├── Iterar specs en directorio
                                     └── Esfuerzo: bajo (~50 líneas)

Fase 4 (calidad de agentes) ─────── #10 cross-story context
                                     ├── Inyectar resúmenes de dependencias Done
                                     ├── Esfuerzo: medio (~120 líneas)
                                     │
                                     ├── #22 plan --append (complementario)
                                     ├── Generación incremental sin pisar backlog
                                     └── Esfuerzo: medio (~100 líneas)

Fase 4 (calidad de agentes) ─────── #10 cross-story context
                                     ├── Inyectar resúmenes de dependencias Done
                                     ├── Esfuerzo: medio (~120 líneas)
                                     │
                                     ├── #22 plan --append (complementario)
                                     ├── Generación incremental sin pisar backlog
                                     └── Esfuerzo: medio (~100 líneas)

Fase 5 (ELIMINADA) ──────────────── #04 workflow configurable
                                     └── REEMPLAZADA por Fase 0 (spartito)

Fase 6 (experiencia) ────────────── #11 TUI, #12 cost tracking, #15 interactive
                                     └── Nice to have, no bloquean adopción

Fase 7 (escalabilidad — ÚLTIMO) ─── #01 paralelismo con tokio async
                                     ├── Tokio runtime, oleadas independientes, Arc<Mutex<>>
                                     ├── Se construye LIMPIAMENTE sobre el trait AgentProvider
                                     └── Esfuerzo: alto (~430 líneas)
```

### 📊 Diagrama de dependencias entre features

```
┌──────────────────────┐
│ 🆕 Spartito (Fase 0) │────── Fundación: define Status, Actor, Workflow trait,
└────────┬─────────────┘        story_format, DoD/DoR. Shared contract.
         │                      Reemplaza a #04 (workflow configurable).
         ▼
┌──────────────────────┐
│ 🆕 #20 Multi-provider│────── Define el trait AgentProvider
└────────┬─────────────┘        (devuelve Vec<String>, agnóstico a sync/async)
         │
         ▼
┌──────────────────────────┐
│ #09 Prompts agnósticos   │
└────────┬─────────────────┘
         │
         ▼
┌────────────────────┐      ┌──────────────────────────┐
│ #14 plan --from-dir │      │ #10 Cross-story context   │
│ #22 plan --append   │      │ (complementa a #10)        │
└────────────────────┘      └───────────────────────────┘

> ⚠️ Las features #11 (TUI), #12 (cost tracking), y #15 (plan interactive) son
> ortogonales al resto y se pueden implementar en cualquier orden.

---

## 📝 Notas sobre el orden

1. **Spartito (Fase 0) primero** porque:
   - Define el **contrato fundacional** del ecosistema: `Status`, `Actor`, `Workflow` trait, `story_format`
   - Es el source of truth compartido entre regista y mezzala
   - Reemplaza y extiende #04 (workflow configurable) con bifurcaciones
   - Sin spartito, regista y mezzala dependen de un contrato implícito frágil
   - `Status` como newtype sobre `String` permite workflows arbitrarios sin recompilar
   - La migración de regista es el mayor riesgo; conviene hacerla cuanto antes

2. **#20 Multi-provider (Fase 1) después** porque:
   - Define la **interfaz fundacional** del sistema: el trait `AgentProvider`
   - El trait devuelve `Vec<String>` (args), no `Command` → compatible con sync y async
   - ✅ Ya implementado

3. **#09 (Fase 2) después** porque:
   - ✅ Ya implementado. Templates de prompt con vars de stack.
   - Define el placeholder `{cross_story_context}` que #10 usará.

4. **#14 y #10 (Fases 3-4)** porque:
   - #14 es quick win (~50 líneas).
   - #10 usa el placeholder de #09 para inyectar contexto.

5. **#01 al final (Fase 7)** porque:
   - El paralelismo añade complejidad de concurrencia.
   - Conviene tener todo el resto del sistema maduro.
