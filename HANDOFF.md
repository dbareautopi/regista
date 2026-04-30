# 🧠 regista — Session Handoff

> **Fecha**: 2026-04-30  
> **Sesión**: Implementación de features del roadmap: JSON/CI-CD, dry-run, validate, init, groom, checkpoint/resume, feedback agentes  
> **Estado**: 104 tests pasando, 0 warnings, 0 clippy issues. Release build limpio.

---

## 📍 Estructura actual

```
/root/repos/regista/
├── Cargo.toml
├── Cargo.lock
├── README.md                   ← Actualizado
├── DESIGN.md                   ← Actualizado
├── AGENTS.md                   ← Guía para agentes
├── HANDOFF.md                  ← Este documento
├── .gitignore
├── src/
│   ├── main.rs                 ← CLI, 5 subcomandos (run/validate/init/groom + flags), JSON output
│   ├── config.rs               ← Config, carga TOML, defaults, nuevos campos
│   ├── state.rs                ← Status (9), Actor (5), Transition, can_transition_to()
│   ├── story.rs                ← Story, parseo .md, set_status(), advance_status_in_memory()
│   ├── dependency_graph.rs     ← Grafo, ciclo DFS, has_any_cycle(), blocks_count()
│   ├── deadlock.rs             ← analyze(), DeadlockResolution, priorización
│   ├── agent.rs                ← invoke_with_retry(), AgentOptions, feedback rico, guardado decisiones
│   ├── prompts.rs              ← PromptContext, 7 funciones de prompt (po_groom, qa_tests, etc.)
│   ├── orchestrator.rs         ← run(), run_real(), run_dry(), process_story(), checkpoint save
│   ├── checkpoint.rs           ← OrchestratorState: save/load/remove (.regista.state.toml)
│   ├── validator.rs            ← validate(): chequeo pre-vuelo de proyecto
│   ├── init.rs                 ← init(): scaffolding de proyecto nuevo
│   ├── groom.rs                ← run(): generación de backlog desde spec con bucle validate
│   ├── hooks.rs                ← run_hook(): comandos post-fase
│   ├── git.rs                  ← snapshot(), rollback()
│   └── daemon.rs               ← detach(), status(), kill(), follow()
├── roadmap/
│   ├── ROADMAP.md              ← Índice general con estado de cada feature
│   ├── 01-paralelismo.md
│   ├── 02-salida-json-ci-cd.md        ← ✅ IMPLEMENTADO
│   ├── 03-dry-run.md                  ← ✅ IMPLEMENTADO
│   ├── 04-workflow-configurable.md
│   ├── 05-validate.md                 ← ✅ IMPLEMENTADO
│   ├── 06-init-scaffold.md            ← ✅ IMPLEMENTADO
│   ├── 07-checkpoint-resume.md        ← ✅ IMPLEMENTADO
│   ├── 08-feedback-agentes.md         ← ✅ IMPLEMENTADO
│   ├── 13-groom-generacion-historias.md ← ✅ IMPLEMENTADO
│   ├── 14-groom-from-dir.md           ← Pendiente (variante)
│   └── 15-groom-interactive.md        ← Pendiente (variante)
└── tests/fixtures/
    ├── story_draft.md
    ├── story_blocked.md
    └── story_business_review.md
```

---

## ⚙️ Funcionalidades implementadas

### Pipeline base
- Loop principal con máquina de estados (Draft → Ready → Tests Ready → In Review → Business Review → Done)
- 14 transiciones canónicas (12 por agentes + 3 automáticas)
- Detección de deadlocks con priorización (mayor desbloqueo → menor ID)
- QA fix automático: si Dev reporta tests rotos, se dispara QA en vez de Dev

### Subcomandos

| Comando | Módulo | Función |
|---------|--------|---------|
| `regista [dir]` | `orchestrator.rs` | Pipeline completo |
| `regista validate [dir]` | `validator.rs` | Chequeo pre-vuelo (config, historias, skills, dependencias, git) |
| `regista init [dir]` | `init.rs` | Scaffolding: .regista.toml + 4 skills + estructura dirs |
| `regista groom <spec>` | `groom.rs` | Generar backlog desde spec con bucle de validación |

### Flags nuevos

| Flag | Feature |
|------|---------|
| `--json` | Salida JSON estructurada a stdout (CI/CD) |
| `--quiet` | Suprimir logs de progreso |
| `--dry-run` | Simular pipeline en memoria sin agentes |
| `--resume` | Reanudar desde último checkpoint |
| `--clean-state` | Borrar checkpoint |
| `--max-stories N` | (groom) Límite de historias, 0 = sin límite |
| `--replace` | (groom) Regenerar desde cero |
| `--light` | (init) Solo .regista.toml |
| `--with-example` | (init) Incluir historia de ejemplo |

### JSON / CI-CD (`02`)
- `RunReport` con `Serialize`, incluye `StoryRecord` por historia
- Exit codes: 0 = OK, 1 = error config, 2 = tiene Failed
- `regista --json > report.json` para GitHub Actions/GitLab CI

### Dry-run (`03`)
- `run_dry()`: simula iteraciones en memoria, sin `pi` ni escritura a disco
- `Story::advance_status_in_memory()` para mutar sin tocar archivos
- Muestra desbloqueos, estima tiempo (~5 min/iteración)
- Compatible con `--json`, `--once`

### Validate (`05`)
- Valida: config parseable, stories_dir existe, skills existen, historias parseables, dependencias sin referencias rotas ni ciclos, git repo
- `--json` para CI, exit codes: 0=OK, 1=errores, 2=warnings

### Init (`06`)
- Genera `.regista.toml` con todos los defaults documentados
- 4 `SKILL.md` (PO, QA, Dev, Reviewer) con responsabilidades y formato Activity Log
- `product/stories/`, `product/epics/`, `product/decisions/`, `product/logs/`
- No pisa archivos existentes

### Groom (`13`)
- `regista groom <spec.md>`: PO descompone spec en historias y épicas
- **Bucle de validación**: groom → validate dependencias → si errores → feedback al PO → corregir → repetir (máx `groom_max_iterations`=5)
- `--max-stories` (0 = sin límite), `--merge` (default) / `--replace`

### Checkpoint / Resume (`07`)
- `checkpoint.rs`: `OrchestratorState` guardado en `.regista.state.toml` tras cada iteración
- `--resume`: restaura `iteration`, `reject_cycles`, `story_iterations`, `story_errors`
- Se limpia automáticamente en `PipelineComplete`
- `--clean-state` para borrado manual

### Feedback rico (`08`)
- `AgentOptions` con `inject_feedback`, `decisions_dir`, `story_id`
- Fallos: guarda `product/decisions/<STORY>-<actor>-<ts>.md` con stdout/stderr
- Reintentos: prompt aumentado con «Tu intento anterior falló: [error]. Corrígelo.»
- `AgentResult` incluye `attempts: Vec<AttemptTrace>`
- Configurable: `inject_feedback_on_retry` (default true)

---

## 🔨 Comandos esenciales

```bash
cargo build              # Debug
cargo build --release    # Release
cargo test               # 104 tests, 0 fallos, 1 ignorado
cargo check              # Verificar warnings
cargo fmt                # Formatear
cargo clippy -- -D warnings  # 0 issues
```

---

## 🧩 Contrato de historia (.md)

```markdown
# STORY-NNN: Título

## Status
**Draft**

## Epic
EPIC-XXX

## Descripción
...

## Criterios de aceptación
- [ ] CA1: específico y verificable
- [ ] CA2: ...

## Dependencias
- Bloqueado por: STORY-XXX

## Activity Log
- YYYY-MM-DD | Actor | descripción
```

---

## 🚧 Pendiente (roadmap)

### Alta/media prioridad
- **01 — Paralelismo**: ejecutar historias independientes simultáneamente
- **04 — Workflow configurable**: transiciones definibles en `.regista.toml`

### Variantes de groom
- **14 — `groom --from-dir`**: múltiples documentos fuente
- **15 — `groom --interactive`**: PO entrevista al usuario

### No implementados (sin doc aún)
- **09 — Prompts agnósticos al stack**: desacoplar referencias a cargo/npm
- **10 — Cross-story context**: agentes reciben contexto de historias relacionadas
- **11 — TUI / dashboard**: visualización en vivo del progreso
- **12 — Cost tracking**: límite de gasto en llamadas LLM

---

## 🔑 Decisiones de diseño

1. **Agnóstico al proyecto**: regista no sabe de Rust, cargo, ni nada.
   Solo invoca `pi --skill <path>` con prompts genéricos.

2. **Workflow fijo**: las 14 transiciones son canónicas e inmutables.

3. **Shell `true` en hooks**: `hooks.rs` ejecuta con `sh -c`.

4. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos.

5. **`set_status()` con backup atómico**: escribe → re-parsea → si falla, restaura `.bak`.

6. **Subcomandos vía args manual**: en vez de refactorizar todo clap, se detectan
   `validate`, `init`, `groom` al inicio de `main()` inspeccionando `std::env::args()`.

7. **Dry-run en memoria**: `advance_status_in_memory()` muta `Story.status` y
   `raw_content` sin tocar el filesystem.

8. **Groom con bucle validate**: el PO recibe feedback concreto de errores de
   dependencias y corrige hasta que el grafo está limpio.

9. **Checkpoint TOML**: mismo formato que el resto del proyecto, legible y
   editable manualmente si es necesario.

10. **Feedback truncado a 2000 bytes**: para no desbordar la ventana de contexto
    del LLM en reintentos.
