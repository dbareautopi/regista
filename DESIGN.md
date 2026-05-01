# 🏗️ regista — Diseño

🎬 AI agent director multi-provider. Independiente del proyecto: no sabe nada
de Rust, ni de qué construyen los agentes. Solo sabe tres cosas:

1. **Dónde están las historias** y cómo leer su estado
2. **Qué provider y qué instrucciones de rol** usar para cada rol del workflow
3. **La máquina de estados fija** que gobierna las transiciones

El proyecto anfitrión se configura mediante un archivo `.regista/config.toml` en su raíz.
El provider se puede elegir vía config (`agents.provider`) o flag CLI (`--provider`).

---

## 1. Sistema de providers

### 1.1 Trait `AgentProvider`

```rust
pub trait AgentProvider {
    fn binary(&self) -> &str;                                      // "pi", "claude", "codex", "opencode"
    fn build_args(&self, instruction_path: &Path, prompt: &str) -> Vec<String>;
    fn display_name(&self) -> &str;                                // "pi", "Claude Code", "Codex", "OpenCode"
    fn instruction_name(&self) -> &str;                            // "skill", "agent", "command"
    fn instruction_dir(&self, role: &str) -> String;               // ".pi/skills/po/SKILL.md"
}
```

El trait devuelve `Vec<String>` (args de CLI), no un `Command`, para ser
compatible con ejecución síncrona y asíncrona (paralelismo #01).

### 1.2 Providers implementados

| Provider | Binario | Argumentos no-interactivo | Directorio de instrucciones |
|----------|---------|--------------------------|----------------------------|
| `PiProvider` | `pi` | `-p "..." --skill <path> --no-session` | `.pi/skills/<rol>/SKILL.md` |
| `ClaudeCodeProvider` | `claude` | `-p "..." --append-system-prompt-file <path> --permission-mode bypassPermissions` | `.claude/agents/<rol>.md` |
| `CodexProvider` | `codex` | `exec --sandbox workspace-write "..."` | `.agents/skills/<rol>/SKILL.md` |
| `OpenCodeProvider` | `opencode` | `-p "..." -q` | `.opencode/commands/<rol>.md` |

### 1.3 Resolución de provider

`AgentsConfig` tiene un `provider` global (default `"pi"`) y cada rol puede
sobreescribirlo vía `AgentRoleConfig`. La CLI puede sobreescribir el global
con `--provider`.

```rust
impl AgentsConfig {
    pub fn provider_for_role(&self, role: &str) -> String { ... }
    pub fn skill_for_role(&self, role: &str) -> String { ... }
}
```

---

## 2. Configuración (`.regista/config.toml`)

```toml
[project]
stories_dir    = ".regista/stories"
story_pattern  = "STORY-*.md"
epics_dir      = ".regista/epics"
decisions_dir  = ".regista/decisions"
log_dir        = ".regista/logs"

[agents]
provider = "pi"                        # provider global (pi, claude, codex, opencode)

[agents.product_owner]                 # opcional: sobreescribir por rol
# provider = "claude"
# skill = ".claude/agents/po-custom.md"

[limits]
max_iterations            = 0   # 0 = auto: nº historias × 6 (mín 10)
max_retries_per_step      = 5
max_reject_cycles         = 3
agent_timeout_seconds     = 1800
max_wall_time_seconds     = 28800
retry_delay_base_seconds  = 10
groom_max_iterations      = 5       # bucle groom→validate→corregir
inject_feedback_on_retry  = true    # inyectar stderr en reintentos

[hooks]
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings"
post_reviewer = "cargo test && cargo clippy -- -D warnings"

[git]
enabled = true
```

---

## 3. Máquina de Estados

### 3.1 Diagrama

```
                    ┌──────────┐
                    │  Draft   │  ← Historia creada, pendiente de refinamiento
                    └────┬─────┘
                         │ PO (groom)
                    ┌────▼─────┐
              ┌─────│  Ready   │  ← Refinada, lista para QA
              │     └────┬─────┘
              │ QA       │ QA (tests escritos)
              │ (rollback│
              │ si no es  ┌────▼──────┐
              │ testeable)│Tests Ready│  ← Tests existen, pendiente Dev
              │           └────┬──────┘
              │                │ Dev (implementa)
              │           ┌────▼──────┐
              │           │ In Review │  ← Implementación lista, pendiente Reviewer
              │           └────┬──────┘
              │                │ Reviewer
              │           ┌────▼──────────┐
              │           │Business Review│  ← DoD técnico OK, pendiente PO
              │           └────┬──────────┘
              │                │ PO (validate)
              │           ┌────▼──────┐
              │           │    Done   │  ← ¡Finalizada!
              │           └───────────┘
              │
              │   ┌──────────────────────────────────────────┐
              └───┤  Rechazos (retrocesos)                   │
                  │                                          │
                  │  In Review ──Reviewer──→ In Progress     │
                  │  Business Review ──PO──→ In Review       │
                  │  Business Review ──PO──→ In Progress     │
                  │  In Progress ──Dev(fix)──→ In Review     │
                  │                                          │
                  │  Estados terminales de fallo:            │
                  │  * ──max reject cycles──→ Failed         │
                  │                                          │
                  │  Bloqueo por dependencias:               │
                  │  * ──bloqueadores no Done──→ Blocked     │
                  │  Blocked ──bloqueadores Done──→ Ready    │
                  └──────────────────────────────────────────┘
```

### 3.2 Tabla canónica de transiciones

| # | De | A | Actor | Condición |
|---|---|---|---|---|
| 1 | `Draft` | `Ready` | **PO** (groom) | Historia cumple DoR |
| 2 | `Ready` | `Tests Ready` | **QA** | Tests escritos para todos los CAs |
| 3 | `Ready` | `Draft` | **QA** (rollback) | Historia no es testeable → PO debe re-refinar |
| 4 | `Tests Ready` | `In Review` | **Dev** | Implementación completa, tests pasan |
| 5 | `Tests Ready` | `Tests Ready` | **QA** (corregir) | Dev reportó que tests no compilan o son incorrectos |
| 6 | `In Progress` | `In Review` | **Dev** (fix) | Corrección aplicada tras rechazo |
| 7 | `In Review` | `Business Review` | **Reviewer** | DoD técnico OK, todos los CAs cubiertos |
| 8 | `In Review` | `In Progress` | **Reviewer** | Rechazo técnico con detalles concretos |
| 9 | `Business Review` | `Done` | **PO** (validate) | Validación de negocio OK |
| 10 | `Business Review` | `In Review` | **PO** (reject) | Rechazo leve: falta detalle, base técnica sólida |
| 11 | `Business Review` | `In Progress` | **PO** (reject) | Rechazo grave: no cumple valor de negocio |
| 12 | `*` | `Blocked` | **Orquestador** | Tiene dependencias en estado ≠ `Done` |
| 13 | `Blocked` | `Ready` | **Orquestador** | Todas las dependencias están en `Done` |
| 14 | `*` | `Failed` | **Orquestador** | Superado `max_reject_cycles` |

> Las transiciones 12, 13, 14 son **automáticas**: las ejecuta el propio orquestador
> sin invocar agentes.

### 3.3 Tipo en Rust

```rust
pub enum Status {
    Draft, Ready, TestsReady, InProgress, InReview,
    BusinessReview, Done, Blocked, Failed,
}

pub enum Actor {
    ProductOwner, QaEngineer, Developer, Reviewer, Orchestrator,
}
```

---

## 4. Detección de bloqueos (deadlock)

Si no hay historias accionables, el orquestador analiza el grafo de dependencias
y dispara al PO para desatascar la historia que más bloqueos resuelve.

### 4.1 Algoritmo

```
Para cada historia no terminal:
  1. Si status == Draft         → "stuck": necesita PO (groom)
  2. Si status == Blocked:
     a. Si algún bloqueador está en Draft → "stuck": PO debe groom el Draft
     b. Si hay ciclo de dependencias     → "stuck": PO debe romper el ciclo
     c. Si todos los bloqueadores Done   → automático → Ready
  3. Resto → el loop normal lo maneja

Si ninguna accionable Y hay stuck → disparar PO para la de mayor prioridad.
Si ninguna accionable Y sin stuck  → Pipeline Complete.
```

### 4.2 Prioridad de desbloqueo

La historia que **desbloquea más historias** (conteo de referencias inversas).
En empate, ID numérico más bajo.

---

## 5. Arquitectura del crate

```
regista/
├── Cargo.toml
├── README.md
├── DESIGN.md                  ← este documento
├── AGENTS.md                  ← guía para agentes de codificación
├── src/
│   ├── main.rs                ← CLI (clap), subcomandos, JSON output, exit codes
│   ├── config.rs              ← Config, AgentsConfig + AgentRoleConfig, carga TOML
│   ├── state.rs               ← Status, Actor, Transition, can_transition_to()
│   ├── story.rs               ← Story, parseo .md, set_status(), advance_status_in_memory()
│   ├── dependency_graph.rs    ← Grafo, ciclo DFS, has_any_cycle(), blocks_count()
│   ├── deadlock.rs            ← analyze(), DeadlockResolution, priorización
│   ├── providers.rs           ← trait AgentProvider + 4 implementaciones + factory
│   ├── agent.rs               ← invoke_with_retry(), AgentOptions, feedback rico
│   ├── prompts.rs             ← PromptContext, 7 funciones de prompt
│   ├── orchestrator.rs        ← run(), run_real(), run_dry(), process_story()
│   ├── checkpoint.rs          ← OrchestratorState: save/load/remove (.regista/state.toml)
│   ├── validator.rs           ← validate(): chequeo pre-vuelo de proyecto
│   ├── init.rs                ← init(): scaffolding multi-provider
│   ├── groom.rs               ← run(): generación de backlog desde spec
│   ├── hooks.rs               ← run_hook(): comandos post-fase
│   ├── git.rs                 ← snapshot(), rollback()
│   └── daemon.rs              ← detach(), status(), kill(), follow()
├── roadmap/                   ← Ideas y features futuras
└── tests/fixtures/            ← Archivos .md de ejemplo
```

---

## 6. Formato de historia esperado (contrato fijo)

```markdown
# STORY-NNN: Título

## Status
**Draft**   ← uno de los 9 estados

## Epic
EPIC-XXX

## Descripción
...

## Criterios de aceptación
- [ ] CA1
- [ ] CA2

## Dependencias       ← opcional
- Bloqueado por: STORY-XXX, STORY-YYY

## Activity Log       ← obligatorio
- YYYY-MM-DD | Actor | descripción
```

---

## 7. CLI

### Comandos

| Comando | Descripción |
|---------|-------------|
| `regista [DIR]` | Pipeline completo (default) |
| `regista validate [DIR]` | Chequeo pre-vuelo de integridad |
| `regista init [DIR]` | Scaffolding de proyecto nuevo |
| `regista groom <SPEC>` | Generar backlog desde spec |
| `regista help` | Mostrar todos los comandos y flags |

### Flags principales

| Flag | Descripción |
|------|-------------|
| `--provider <NAME>` | Provider a usar (pi, claude, codex, opencode) |
| `--once` | Una sola iteración |
| `--json` | Salida JSON a stdout |
| `--quiet` | Suprimir logs de progreso |
| `--dry-run` | Simular sin ejecutar agentes |
| `--resume` | Reanudar desde checkpoint |
| `--clean-state` | Borrar checkpoint |
| `--story <ID>` | Filtrar por historia |
| `--epic <ID>` | Filtrar por épica |
| `--epics <RANGE>` | Filtrar por rango de épicas |
| `--config <FILE>` | Configuración alternativa |
| `--log-file <FILE>` | Archivo de log |
| `--detach` | Modo daemon |
| `--follow` | Ver log del daemon |
| `--status` | Estado del daemon |
| `--kill` | Detener daemon |

### Exit codes

| Código | Significado |
|--------|-------------|
| 0 | Pipeline completo, 0 `Failed` |
| 2 | Pipeline completo, ≥1 `Failed` |
| 3 | Parada temprana por límite (`max_iterations` o `max_wall_time`) |

---

## 8. Checkpoint / Resume

Tras cada `process_story()` exitoso, el orquestador guarda su estado en
`<project_dir>/.regista/state.toml`:

```toml
iteration = 7

[reject_cycles]
"STORY-013" = 2

[story_iterations]
"STORY-001" = 4

[story_errors]
"STORY-015" = "max_reject_cycles alcanzado"
```

Al reanudar con `--resume`, se restauran los contadores y se continúa desde
la iteración guardada. El checkpoint se limpia automáticamente al llegar a
`PipelineComplete`.

---

## 9. Feedback rico de agentes

Cuando `inject_feedback_on_retry = true` (default):

1. En cada intento fallido, se guarda stdout/stderr en
   `.regista/decisions/<STORY>-<actor>-<timestamp>.md`.
2. En el reintento, el prompt se modifica:
   ```
   ⚠️ Tu intento anterior falló. Esto fue lo ocurrido:
     [stderr del intento anterior]
   Corrige el error e inténtalo de nuevo.
   ---
   [prompt original]
   ```
3. El `AgentResult` incluye `attempts: Vec<AttemptTrace>` con la traza completa.

---

## 10. Dry-run

`--dry-run` simula el pipeline en memoria sin invocar agentes ni modificar
archivos. Usa `Story::advance_status_in_memory()` para mutar estados sin
escribir a disco. Muestra qué transiciones se harían, qué historias se
desbloquearían, y estima el tiempo total. Compatible con `--json`.

---

## 11. Groom — Generación automática de backlog

`regista groom <spec.md>` invoca al PO para descomponer una spec en historias
y épicas. Tras generar, ejecuta un **bucle de validación**:

```
groom → generate → validate dependencias
  ├── OK → terminar
  └── errores → feedback al PO → corregir → validate → ...
```

Máximo de iteraciones configurable: `groom_max_iterations` (default 5).

---

## 12. Plan de implementación (histórico)

| Fase | Qué | Resultado |
|------|-----|-----------|
| F1–F12 | Crate base, CLI, máquina de estados, pipeline, daemon, tests | 82 tests ✅ |
| F13 | Salida JSON + CI/CD, dry-run | `--json`, `--dry-run` ✅ |
| F14 | `regista validate`, `regista init` | Subcomandos ✅ |
| F15 | `regista groom` | Generación de backlog ✅ |
| F16 | Checkpoint/resume + feedback rico | `--resume`, feedback en retry ✅ |
| F17 | **Multi-provider (#20)** | pi, Claude Code, Codex, OpenCode ✅ |
