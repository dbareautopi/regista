# 🗺️ regista — Roadmap

Ideas, mejoras y funcionalidades pendientes para convertir `regista` en una
herramienta de desarrollo real, adoptable por equipos.

Cada entrada tiene su propio documento con descripción detallada, motivación,
y notas de implementación.

---

## 🔴 Crítica — Bloquea adopción real

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 1 | **Paralelismo**: ejecutar múltiples historias independientes simultáneamente | [`01-paralelismo.md`](./01-paralelismo.md) | Alto |
| 2 | **Salida JSON + CI/CD**: reportes estructurados, exit codes, integración con pipelines | [`02-salida-json-ci-cd.md`](./02-salida-json-ci-cd.md) | ✅ Implementado |
| 3 | **Dry-run**: simular qué haría el orquestador sin ejecutar agentes | [`03-dry-run.md`](./03-dry-run.md) | ✅ Implementado |
| 4 | **Workflow configurable**: estados y transiciones definibles en `.regista.toml` | [`04-workflow-configurable.md`](./04-workflow-configurable.md) | Medio |

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
| 9 | **Prompts agnósticos al stack**: desacoplar referencias a herramientas (cargo, npm) | [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) | Bajo |
| 10 | **Conciencia cross-story**: agentes reciben contexto de historias relacionadas | [`10-cross-story-context.md`](./10-cross-story-context.md) | Medio |
| 11 | **TUI / dashboard**: visualización en vivo del progreso del pipeline | [`11-tui-dashboard.md`](./11-tui-dashboard.md) | Medio |
| 12 | **Cost tracking**: estimación y límite de gasto en llamadas LLM | [`12-cost-tracking.md`](./12-cost-tracking.md) | Medio |

## 🟢 Generación de backlog — Automatizar la creación de historias

| # | Funcionalidad | Doc | Esfuerzo |
|---|---|---|---|
| 13 | **`regista groom`**: generar historias desde un documento de requisitos, con bucle de validación de dependencias | [`13-groom-generacion-historias.md`](./13-groom-generacion-historias.md) | ✅ Implementado |
| 14 | **`groom --from-dir`**: generar desde un directorio de specs por feature | [`14-groom-from-dir.md`](./14-groom-from-dir.md) | Bajo |
| 15 | **`groom --interactive`**: el PO entrevista al usuario para extraer requisitos | [`15-groom-interactive.md`](./15-groom-interactive.md) | Medio |

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

## 🗓️ Orden sugerido de implementación

```
Fase 1 (bajo esfuerzo, alto impacto) ─── dry-run + JSON/CI-CD + validate
Fase 2 (calidad de vida) ─────────────── init + feedback agentes + prompts agnósticos
Fase 3 (diferenciación) ──────────────── workflow configurable + checkpoint + cross-story
Fase 4 (experiencia) ─────────────────── paralelismo + TUI + cost tracking
```
