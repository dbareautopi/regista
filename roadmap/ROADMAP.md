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
| 4 | **Workflow configurable**: estados y transiciones definibles en `.regista/config.toml` | [`04-workflow-configurable.md`](./04-workflow-configurable.md) | Medio |
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
| 9 | **Prompts agnósticos al stack**: desacoplar referencias a herramientas (cargo, npm) | [`09-prompts-agnosticos.md`](./09-prompts-agnosticos.md) ✍️ | Bajo |
| 10 | **Conciencia cross-story**: agentes reciben contexto de historias relacionadas | [`10-cross-story-context.md`](./10-cross-story-context.md) ✍️ | Medio |
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

## 🗓️ Orden de implementación (mayo 2026)

```
Fase 1 (abstracción fundacional) ── 🆕 #20 multi-provider (Claude Code, Aider…)
                                     ├── Trait AgentProvider (devuelve Vec<String>, agnóstico a sync/async)
                                     ├── PiProvider, ClaudeCodeProvider, AiderProvider
                                     └── Esfuerzo: medio (~215 líneas)

Fase 2 (escalabilidad) ──────────── #01 paralelismo con tokio async
                                     ├── Tokio runtime, oleadas independientes, Arc<Mutex<>>
                                     ├── Se construye LIMPIAMENTE sobre el trait AgentProvider
                                     └── Esfuerzo: alto (~430 líneas)

Fase 3 (prerrequisito natural) ──── #09 prompts agnósticos al stack
                                     ├── Templates de prompt con vars de stack
                                     └── Esfuerzo: bajo (~80 líneas)

Fase 4 (quick win) ──────────────── #14 groom --from-dir
                                     ├── Iterar specs en directorio
                                     └── Esfuerzo: bajo (~50 líneas)

Fase 5 (calidad de agentes) ─────── #10 cross-story context
                                     ├── Inyectar resúmenes de dependencias Done
                                     └── Esfuerzo: medio (~120 líneas)

Fase 6 (diferenciación) ─────────── #04 workflow configurable
                                     ├── Status dinámico, transiciones desde TOML
                                     └── Esfuerzo: medio-alto (~300 líneas)

Fase 7 (experiencia) ────────────── #11 TUI, #12 cost tracking, #15 interactive
                                     └── Nice to have, no bloquean adopción
```

### 📊 Diagrama de dependencias entre features

```
┌──────────────────────┐
│ 🆕 #20 Multi-provider│────── Fundación: define el trait AgentProvider
└────────┬─────────────┘        (devuelve Vec<String>, agnóstico a sync/async)
         │
         │  #01 se construye sobre el trait.
         │  Sin el trait, el código concurrente tendría "pi" hardcodeado.
         ▼
┌──────────────────────┐
│  #01 Paralelismo     │────── Tokio async + oleadas independientes
└────────┬─────────────┘        Arc<Mutex<>> para shared state
         │
         │  #09 necesita providers + async ya funcionando
         │  para saber qué variables de stack usar
         ▼
┌──────────────────────────┐
│ #09 Prompts agnósticos   │
└────────┬─────────────────┘
         │
         ▼
┌────────────────────┐      ┌──────────────────────────┐
│ #14 groom --from-dir│      │ #10 Cross-story context   │
└────────────────────┘      └────────┬─────────────────┘
         │                           │
         │  #04 necesita prompts     │  #04 necesita contexto
         │  genéricos + cross-story  │  de dependencias
         │  ya funcionando           │
         └───────────┬───────────────┘
                     ▼
         ┌──────────────────────────┐
         │ #04 Workflow configurable│
         └──────────────────────────┘
```

> ⚠️ Las features #11 (TUI), #12 (cost tracking), y #15 (groom interactive) son
> ortogonales al resto y se pueden implementar en cualquier orden.

---

## 📝 Notas sobre el orden

1. **#20 Multi-provider primero** porque:
   - Define la **interfaz fundacional** del sistema: el trait `AgentProvider`
   - El trait devuelve `Vec<String>` (args), no `Command` → compatible con sync y async
   - Es la feature con mayor impacto en adopción que NO estaba en el roadmap original
   - Elimina la dependencia dura de `pi` (vendor lock-in)
   - Una vez que `agent.rs` usa providers, añadir Claude Code, Aider o cualquier otro es trivial

2. **#01 Paralelismo justo después** porque:
   - Se construye LIMPIAMENTE sobre el trait `AgentProvider` (sin hardcodeos a `pi`)
   - El trait ya está diseñado para ser async-compatible (devuelve args, no `Command`)
   - Si se hiciera antes, el código concurrente tendría `Command::new("pi")` hardcodeado
   - Usa `tokio` async (no threads crudos) para timeouts, cancelación y rate limiting
   - Establece el modelo de shared state (`Arc<Mutex<>>`) que todo lo demás usará

3. **#09 después** porque:
   - Con providers + async ya funcionando, los templates de prompt pueden adaptarse a cada stack/provider.
   - Define el placeholder `{cross_story_context}` que #10 usará para inyectar contexto.
   - Prepara los prompts genéricos por rol que #04 (workflow configurable) necesita.

4. **#14 antes que #10** porque:
   - Es un quick win (~50 líneas) que no depende de nada más.
   - #10 se beneficia de tener el ecosistema de groom completo antes de añadir contexto.

5. **#10 después de #09 y #14** porque:
   - Usa el placeholder `{cross_story_context}` definido en #09 para inyectar contexto.
   - Con paralelismo ya estable, sabe exactamente cuándo construir contexto (entre oleadas).
   - Los resúmenes de historias Done se cachean en `decisions/`, sin llamadas额外 al LLM.

6. **#04 al final** porque:
   - Es el cambio más disruptivo (Status de enum a string, prompts genéricos).
   - Conviene tener providers, paralelismo, prompts agnósticos y cross-story context estables antes de meterle mano a la máquina de estados.
   - Para entonces, los prompts ya son genéricos por rol y aceptan contexto dinámico.
