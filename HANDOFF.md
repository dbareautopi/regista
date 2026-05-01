# 🧠 regista — Session Handoff

> **Fecha**: 2026-05-01
> **Sesión**: Definición de roadmap #09 (prompts agnósticos) y #10 (cross-story context) + análisis de colisiones con paralelismo
> **Versión**: v0.3.0
> **Estado**: 128 tests pasando, 0 fallos, 1 ignorado, 0 warnings.

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
│   ├── main.rs                 ← CLI, 6 subcomandos (run/validate/init/groom/help + flags), JSON output, --provider
│   ├── config.rs               ← Config, AgentsConfig + AgentRoleConfig, provider_for_role(), skill_for_role(), carga TOML
│   ├── state.rs                ← Status (9), Actor (5), Transition, can_transition_to()
│   ├── story.rs                ← Story, parseo .md, set_status(), advance_status_in_memory()
│   ├── dependency_graph.rs     ← Grafo, ciclo DFS, has_any_cycle(), blocks_count()
│   ├── deadlock.rs             ← analyze(), DeadlockResolution, priorización
│   ├── providers.rs            ← trait AgentProvider + PiProvider/ClaudeCodeProvider/CodexProvider/OpenCodeProvider + factory from_name()
│   ├── agent.rs                ← invoke_with_retry(provider: &dyn AgentProvider, …), AgentOptions, feedback rico, guardado decisiones
│   ├── prompts.rs              ← PromptContext, 7 funciones de prompt (po_groom, qa_tests, etc.)
│   ├── orchestrator.rs         ← run(), run_real(), run_dry(), process_story() con resolución de provider por rol
│   ├── checkpoint.rs           ← OrchestratorState: save/load/remove (.regista/state.toml)
│   ├── validator.rs            ← validate(): chequeo pre-vuelo multi-provider (config, skills, historias, dependencias, git)
│   ├── init.rs                 ← init(): scaffolding multi-provider (pi, claude, codex, opencode)
│   ├── groom.rs                ← run(): generación de backlog desde spec con bucle validate
│   ├── hooks.rs                ← run_hook(): comandos post-fase
│   ├── git.rs                  ← snapshot(), rollback()
│   └── daemon.rs               ← detach(), status(), kill(), follow()
├── roadmap/
│   ├── ROADMAP.md              ← Índice general con estado de cada feature y orden de implementación
│   ├── 01-paralelismo.md       ← Diseño detallado (Fase 2, pendiente)
│   ├── 02-salida-json-ci-cd.md        ← ✅ IMPLEMENTADO
│   ├── 03-dry-run.md                  ← ✅ IMPLEMENTADO
│   ├── 04-workflow-configurable.md    ← Diseño detallado (Fase 6, pendiente)
│   ├── 05-validate.md                 ← ✅ IMPLEMENTADO
│   ├── 06-init-scaffold.md            ← ✅ IMPLEMENTADO
│   ├── 07-checkpoint-resume.md        ← ✅ IMPLEMENTADO
│   ├── 08-feedback-agentes.md         ← ✅ IMPLEMENTADO
│   ├── 09-prompts-agnosticos.md       ← ✍️  Diseño definido (Fase 3)
│   ├── 10-cross-story-context.md      ← ✍️  Diseño definido (Fase 5)
│   ├── 13-groom-generacion-historias.md ← ✅ IMPLEMENTADO
│   ├── 14-groom-from-dir.md           ← Pendiente (variante)
│   ├── 15-groom-interactive.md        ← Pendiente (variante)
│   ├── 20-multi-provider.md           ← ✅ IMPLEMENTADO
│   └── 20-implementacion.md           ← Detalle técnico de la implementación
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
| `regista [dir]` | `orchestrator.rs` | Pipeline completo multi-provider |
| `regista validate [dir]` | `validator.rs` | Chequeo pre-vuelo (config, instrucciones de rol, historias, dependencias, git) |
| `regista init [dir]` | `init.rs` | Scaffolding multi-provider: .regista/config.toml + instrucciones de rol + estructura dirs |
| `regista groom <spec>` | `groom.rs` | Generar backlog desde spec con bucle de validación |
| `regista help` | `main.rs` | Mostrar todos los comandos y flags |

### Flags

| Flag | Feature |
|------|---------|
| `--provider <NAME>` | Seleccionar provider (pi, claude, codex, opencode) — aplica a pipeline y groom |
| `--json` | Salida JSON estructurada a stdout (CI/CD) |
| `--quiet` | Suprimir logs de progreso |
| `--dry-run` | Simular pipeline en memoria sin agentes |
| `--resume` | Reanudar desde último checkpoint |
| `--clean-state` | Borrar checkpoint |
| `--max-stories N` | (groom) Límite de historias, 0 = sin límite |
| `--replace` | (groom) Regenerar desde cero |
| `--light` | (init) Solo .regista/config.toml |
| `--with-example` | (init) Incluir historia de ejemplo |

### JSON / CI-CD (`02`)
- `RunReport` con `Serialize`, incluye `StoryRecord` por historia
- Exit codes: 0 = OK, 2 = tiene Failed, 3 = parada temprana (límite)
- `regista --json > report.json` para GitHub Actions/GitLab CI
- Nuevo en v0.2.0: campos `stopped_early` y `stop_reason` en JSON

### Dry-run (`03`)
- `run_dry()`: simula iteraciones en memoria, sin `pi` ni escritura a disco
- `Story::advance_status_in_memory()` para mutar sin tocar archivos
- Muestra desbloqueos, estima tiempo (~5 min/iteración)
- Compatible con `--json`, `--once`

### Validate (`05`)
- Valida: config parseable, stories_dir existe, skills existen, historias parseables, dependencias sin referencias rotas ni ciclos, git repo
- `--json` para CI, exit codes: 0=OK, 1=errores, 2=warnings

### Init (`06`) — multi-provider
- Genera `.regista/config.toml` con el provider especificado
- 4 instrucciones de rol en el directorio del provider:
  - `pi` → `.pi/skills/<rol>/SKILL.md`
  - `claude` → `.claude/agents/<rol>.md`
  - `codex` → `.agents/skills/<rol>/SKILL.md`
  - `opencode` → `.opencode/commands/<rol>.md`
- `.regista/stories/`, `.regista/epics/`, `.regista/decisions/`, `.regista/logs/`
- No pisa archivos existentes
- `--provider` flag (default: pi)
- `--light`: solo config, sin instrucciones de rol
- `--with-example`: incluye historia y épica de ejemplo
- `max_iterations = 0` por defecto (auto-escalado)

### Groom (`13`)
- `regista groom <spec.md>`: PO descompone spec en historias y épicas
- **Bucle de validación**: groom → validate dependencias → si errores → feedback al PO → corregir → repetir (máx `groom_max_iterations`=5)
- `--max-stories` (0 = sin límite), `--merge` (default) / `--replace`

### Checkpoint / Resume (`07`)
- `checkpoint.rs`: `OrchestratorState` guardado en `.regista/state.toml` tras cada iteración
- Auto-crea el directorio `.regista/` si no existe
- `--resume`: restaura `iteration`, `reject_cycles`, `story_iterations`, `story_errors`
- Se limpia automáticamente en `PipelineComplete`
- `--clean-state` para borrado manual

### Feedback rico (`08`)
- `AgentOptions` con `inject_feedback`, `decisions_dir`, `story_id`
- Fallos: guarda `.regista/decisions/<STORY>-<actor>-<ts>.md` con stdout/stderr
- Reintentos: prompt aumentado con «Tu intento anterior falló: [error]. Corrígelo.»
- `AgentResult` incluye `attempts: Vec<AttemptTrace>`
- Configurable: `inject_feedback_on_retry` (default true)

### v0.2.0 — Migración a `.regista/`, calidad de vida y multi-provider
- **Comando `help`**: `regista help` lista todos los comandos y flags
- **Migración a `.regista/`**: todos los paths viven bajo `.regista/`:
  `config.toml`, `stories/`, `epics/`, `decisions/`, `logs/`,
  `state.toml` (checkpoint), `daemon.pid`, `daemon.log`
- **Auto-escalado de `max_iterations`**: cuando es 0, calcula `max(10, stories × 6)`
- **Exit code 3**: parada temprana por límite de iteraciones o wall time
- **`stop_reason`**: `RunReport` incluye razón de parada (`None` = completado)
- **README refinado**: quick start, badges, ejemplos JSON, estructura de proyecto
- Sin retrocompatibilidad con rutas antiguas

#### Multi-provider (#20)
- **Nuevo módulo `providers.rs`**: trait `AgentProvider` + 4 implementaciones
- **PiProvider**: `pi -p "..." --skill <path> --no-session`
- **ClaudeCodeProvider**: `claude -p "..." --append-system-prompt-file <path> --permission-mode bypassPermissions`
- **CodexProvider**: `codex exec --sandbox workspace-write "..."` (auto-descubre `.agents/skills/`)
- **OpenCodeProvider**: `opencode -p "..." -q`
- **Config**: `AgentsConfig.provider` global + `AgentRoleConfig` por rol (provider + skill)
- **Resolución**: `provider_for_role()` y `skill_for_role()` en `AgentsConfig`
- **Factory**: `providers::from_name("claude")` con alias (claude-code, open-code, etc.)
- **Agent**: `invoke_with_retry` recibe `&dyn AgentProvider` en vez de hardcodear `pi`
- **Orquestador**: `process_story` resuelve provider e instrucciones por rol vía config
- **Init**: genera instrucciones en directorio del provider (`--provider`)
- **Validator**: chequea instrucciones de rol multi-provider
- **CLI**: flag `--provider` para sobreescribir provider global
- **Breaking changes: NINGUNO** — si no se especifica provider, default `"pi"`

---

## 🔨 Comandos esenciales

```bash
cargo build              # Debug
cargo build --release    # Release
cargo test               # 128 tests, 0 fallos, 1 ignorado
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
- **01 - Paralelismo**: ejecutar historias independientes simultáneamente (Fase 2)
- **09 - Prompts agnósticos al stack**: desacoplar referencias a cargo/npm (Fase 3, ✍️ diseño definido)
- **04 - Workflow configurable**: transiciones definibles en `.regista/config.toml` (Fase 6)

### Media prioridad
- **10 - Cross-story context**: agentes reciben contexto de historias relacionadas (Fase 5, ✍️ diseño definido)
- **11 - TUI / dashboard**: visualización en vivo del progreso (Fase 7)
- **12 - Cost tracking**: límite de gasto en llamadas LLM (Fase 7)

### Variantes de groom
- **14 - `groom --from-dir`**: múltiples documentos fuente (Fase 4)
- **15 - `groom --interactive`**: PO entrevista al usuario (Fase 7)

---

## 🔑 Decisiones de diseño

1. **Agnóstico al proyecto**: regista no sabe de Rust, cargo, ni nada.
   Solo invoca el provider configurado con prompts genéricos.

2. **Workflow fijo**: las 14 transiciones son canónicas e inmutables.

3. **Trait `AgentProvider` devuelve `Vec<String>`**: no `Command`, para ser
   compatible con ejecución síncrona y asíncrona (paralelismo #01).

4. **Shell `true` en hooks**: `hooks.rs` ejecuta con `sh -c`.

5. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos.

6. **`set_status()` con backup atómico**: escribe → re-parsea → si falla, restaura `.bak`.

7. **Subcomandos vía args manual**: en vez de refactorizar todo clap, se detectan
   `validate`, `init`, `groom` al inicio de `main()` inspeccionando `std::env::args()`.

8. **Dry-run en memoria**: `advance_status_in_memory()` muta `Story.status` y
   `raw_content` sin tocar el filesystem.

9. **Groom con bucle validate**: el PO recibe feedback concreto de errores de
   dependencias y corrige hasta que el grafo está limpio.

10. **Checkpoint TOML**: mismo formato que el resto del proyecto, legible y
    editable manualmente si es necesario.

11. **Feedback truncado a 2000 bytes**: para no desbordar la ventana de contexto
    del LLM en reintentos.

12. **`max_iterations = 0` por defecto**: el orquestador escala automáticamente
    el límite según `nº de historias × 6`, con un mínimo de 10.

13. **Provider por defecto `"pi"`**: retrocompatibilidad total. Si no se
    especifica provider en config ni CLI, se usa pi. Projects existentes
    siguen funcionando sin cambios.
