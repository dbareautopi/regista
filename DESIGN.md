# 🏗️ regista — Diseño

🎬 **Orquestador del ecosistema mezzala-regista.** Regista es el director que lee
la partitura (`spartito`) y coordina a los músicos (agentes de codificación).
Independiente del proyecto: no sabe nada de Rust, ni de qué construyen los agentes.
Solo sabe tres cosas:

1. **Dónde están las historias** y cómo leer su estado (formato definido en `spartito::story_format`)
2. **Qué provider y qué instrucciones de rol** usar para cada rol del workflow
3. **La partitura** — `spartito` — que define estados, transiciones, formato de historia,
   y checklists DoD/DoR. Es el source of truth compartido con `mezzala`.

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
plan_max_iterations      = 5       # bucle plan→validate→corregir
inject_feedback_on_retry  = true    # inyectar stderr en reintentos

[hooks]
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings"
post_reviewer = "cargo test && cargo clippy -- -D warnings"

[git]
enabled = true

[stack]
# Comandos del stack. Opcionales: si no se definen, el prompt usa
# instrucciones genéricas y el skill del agente interpreta el stack.
build_command = "npm run build"
test_command  = "npm test"
lint_command  = "eslint ."
fmt_command   = "prettier --check ."
src_dir       = "src/"

# ── Workflow configurable (spartito) ──────────────────────
# Opcional. Si no se define, se usa el CanonicalWorkflow
# (14 transiciones fijas, compatible hacia atrás).
#
# [workflow]
# states = ["Draft", "Ready", "In Progress", "In Review", "Done", "Blocked", "Failed"]
#
# [[workflow.transitions]]
# from = "Draft"
# to   = "Ready"
# by   = "product_owner"
```

### 2.1 StackConfig

```rust
pub struct StackConfig {
    pub build_command: Option<String>,
    pub test_command: Option<String>,
    pub lint_command: Option<String>,
    pub fmt_command: Option<String>,
    pub src_dir: Option<String>,
}
```

`StackConfig::render()` genera el bloque de comandos para el prompt.
Si no hay comandos definidos, devuelve instrucción genérica.

---

## 3. Máquina de Estados

> ⚠️ La máquina de estados está **externalizada** en el crate `spartito`,
> compartido con `mezzala`. Regista importa `spartito::workflow`.
> El diseño aquí es referencia; el source of truth es [`spartito`](../../mezzala/docs/spec-spartito.md).

### 3.1 Workflow canónico (14 transiciones, por defecto)

```
                    ┌──────────┐
                    │  Draft   │  ← Historia creada, pendiente de refinamiento
                    └────┬─────┘
                         │ PO (plan)
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
| 1 | `Draft` | `Ready` | **PO** (plan) | Historia cumple DoR |
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

### 3.3 Tipos en Rust (viven en `spartito`)

```rust
// spartito::types — newtypes sobre String para extender en runtime
pub struct Status(String);   // constantes: Status::DRAFT, Status::READY, ...
pub struct Actor(String);    // constantes: Actor::PRODUCT_OWNER, Actor::DEVELOPER, ...

pub struct Transition {
    pub from: Status,
    pub to: Status,
    pub by: Actor,
    pub guard: Option<Guard>,  // Some → transición automática (orchestrator)
}

pub enum Guard {
    HasUnresolvedDependencies,
    AllDependenciesDone,
    MaxRejectCyclesExceeded,
    Custom(String),
}
```

### 3.4 Trait `Workflow` (vive en `spartito::workflow`)

```rust
pub trait Workflow: Sync {
    fn transitions_from(&self, state: &Status) -> Vec<&Transition>;
    fn role_for_state(&self, state: &Status) -> Actor;
    fn state_order(&self) -> &[Status];
    fn terminal_states(&self) -> &[Status];
    fn stuck_states(&self) -> &[Status];
    fn automatic_transitions(&self) -> Vec<&Transition>;
    fn evaluate_guard(&self, guard: &Guard, deps_all_done: bool, ...) -> bool;
}
```

- `CanonicalWorkflow`: 14 transiciones fijas (compatible hacia atrás).
- `ConfigurableWorkflow`: workflow cargado desde `[workflow]` en `.regista/config.toml`.
- Soporta **bifurcaciones**: `transitions_from()` devuelve N destinos; el agente elige.
- `domain/workflow.rs` se **elimina** de regista.

---

## 4. Detección de bloqueos (deadlock)

Si no hay historias accionables, el orquestador analiza el grafo de dependencias
y dispara al PO para desatascar la historia que más bloqueos resuelve.

### 4.1 Algoritmo

```
Para cada historia no terminal:
  1. Si status == Draft         → "stuck": necesita PO (plan)
  2. Si status == Blocked:
     a. Si algún bloqueador está en Draft → "stuck": PO debe planificar el Draft
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

## 5. Arquitectura del ecosistema

```
┌──────────────────────────────────────────────────────┐
│                    spartito                           │
│  crate independiente (crates.io)                     │
│  zero-deps (solo serde opcional)                     │
│                                                      │
│  types.rs          Status, Actor, Transition, Guard  │
│  workflow.rs       Workflow trait, Canonical, Config │
│  story_format.rs   Parseo/validación de .md          │
│  dod.rs / dor.rs   Checklists canónicos              │
│  config.rs         WorkflowConfig (serde)            │
└──────────┬───────────────────────────────┬───────────┘
           │                               │
    ┌──────▼──────┐                 ┌──────▼──────┐
    │   regista    │                 │   mezzala    │
    │ (director)   │                 │  (músico)    │
    │              │                 │              │
    │ orquestador  │                 │ agent harness│
    │ de pipelines │                 │ TUI + WASM   │
    └──────────────┘                 └──────────────┘
```

### 5.1 Arquitectura interna de regista

```
regista/
├── Cargo.toml                 ← depende de spartito
├── README.md
├── DESIGN.md                  ← este documento
├── AGENTS.md                  ← guía para agentes de codificación
├── src/
│   ├── main.rs                ← CLI (clap), subcomandos, JSON output, exit codes
│   ├── config.rs              ← Config, AgentsConfig + AgentRoleConfig, carga TOML
│   │                            (añade campo workflow: Option<WorkflowConfig>)
│   ├── cli/
│   │   ├── args.rs            ← Cli, Commands
│   │   └── handlers.rs        ← dispatch(), handlers, daemon, exit codes
│   ├── app/
│   │   ├── pipeline.rs        ← run_real(), run_dry(), process_story() con Box<dyn Workflow>
│   │   ├── plan.rs            ← Generación de backlog + bucle plan→validate
│   │   ├── board.rs           ← Dashboard Kanban (usa workflow.state_order())
│   │   ├── health.rs          ← HealthReport: métricas
│   │   ├── validate.rs        ← Chequeo pre-vuelo + validación de workflow
│   │   ├── init.rs            ← Scaffolding multi-provider
│   │   └── update.rs          ← Auto-update desde crates.io
│   ├── domain/
│   │   ├── state.rs           ← Wrapper: re-exports de spartito + SharedState
│   │   ├── story.rs           ← Story: delega parseo a spartito::story_format
│   │   ├── graph.rs           ← DependencyGraph, DFS ciclos
│   │   ├── deadlock.rs        ← analyze() usando &dyn Workflow
│   │   └── prompts.rs         ← PromptContext, prompts con bifurcaciones
│   └── infra/
│       ├── providers.rs       ← trait AgentProvider + 4 implementaciones
│       ├── agent.rs           ← invoke_with_retry() async
│       ├── checkpoint.rs      ← OrchestratorState: save/load/remove
│       ├── daemon.rs          ← detach(), status(), kill(), follow()
│       ├── git.rs             ← snapshot(), rollback()
│       └── hooks.rs           ← run_hook(): comandos shell
├── tests/
│   ├── architecture.rs        ← verifica reglas de capas R1-R5
│   └── fixtures/
└── roadmap/
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
| `regista plan <SPEC>` | Generar backlog desde spec |
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

`regista plan <spec.md>` invoca al PO para descomponer una spec en historias
y épicas. Tras generar, ejecuta un **bucle de validación**:

```
plan → generate → validate dependencias
  ├── OK → terminar
  └── errores → feedback al PO → corregir → validate → ...
```

Máximo de iteraciones configurable: `plan_max_iterations` (default 5).

---

## 12. Plan de implementación (histórico)

| Fase | Qué | Resultado |
|------|-----|-----------|
| F1–F12 | Crate base, CLI, máquina de estados, pipeline, daemon, tests | 82 tests ✅ |
| F13 | Salida JSON + CI/CD, dry-run | `--json`, `--dry-run` ✅ |
| F14 | `regista validate`, `regista init` | Subcomandos ✅ |
| F15 | `regista plan` | Generación de backlog ✅ |
| F16 | Checkpoint/resume + feedback rico | `--resume`, feedback en retry ✅ |
| F17 | **Multi-provider (#20)** | pi, Claude Code, Codex, OpenCode ✅ |
| F18 | Reestructuración en capas (v0.9.0) | `cli/`, `app/`, `domain/`, `infra/` ✅ |
| F19 | Workflow trait + CanonicalWorkflow | `domain/workflow.rs` ✅ (migrará a spartito) |
| S1 | **Spartito** — crate compartido (v0.10.0) | 🔜 Pendiente: ~115 tests nuevos |
| S2 | Migrar regista a spartito (v0.10.0) | 🔜 Pendiente: adaptar ~357 tests |

---

## 13. Spartito — El contrato externalizado

### 13.1 ¿Por qué un crate separado?

| Problema | Solución con spartito |
|----------|----------------------|
| `Status` es un enum fijo en 9 variantes | `Status` es un newtype sobre `String`; constantes `&'static str` para el workflow canónico; se extiende en TOML |
| Las 14 transiciones están hardcodeadas | `ConfigurableWorkflow` carga transiciones desde `[workflow]` en TOML |
| El formato de historia está duplicado (regista lo parsea, mezzala lo documenta en skills) | `spartito::story_format` es el parser oficial; ambos proyectos lo importan |
| Si cambia el contrato, hay que actualizar dos proyectos manualmente | `cargo update -p spartito` actualiza ambos simultáneamente |
| No hay bifurcaciones: cada estado tiene un solo destino | `transitions_from()` devuelve `Vec<&Transition>`; el agente elige |

### 13.2 Metáfora del ecosistema

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   SPARTITO   │     │   REGISTA    │     │   MEZZALA    │
│  (partitura) │     │  (director)  │     │  (músico)    │
│              │     │              │     │              │
│ Define QUÉ   │────▶│ Lee y decide │────▶│ Ejecuta      │
│ y CÓMO       │     │ QUIÉN y      │     │ siguiendo la │
│              │     │ CUÁNDO       │     │ partitura    │
└──────────────┘     └──────────────┘     └──────────────┘
     crate               binario              binario
  zero-deps           orquestador         agent harness
```

### 13.3 Decisiones de diseño de spartito

| # | Decisión | Justificación |
|---|----------|---------------|
| D1 | `Status` newtype sobre `String` | Extensible en runtime sin recompilar |
| D2 | `Actor` newtype sobre `String` | Roles arbitrarios definibles en TOML |
| D3 | `Guard` enum + variante `Custom` | 3 guards estándar cubren 95%; `Custom` para extensión |
| D4 | `Workflow: Sync` | Permite `&dyn Workflow` a través de `.await` |
| D5 | `serde` feature-gated (default on) | WASM no necesita TOML |
| D6 | Zero regex crate | Búsqueda manual de IDs; compatible con WASM |
| D7 | `domain/state.rs` como wrapper | Re-exports para migración progresiva sin romper imports |
| D8 | `domain/workflow.rs` eliminado | El trait vive en spartito |

### 13.4 Plan de migración

```
Fase S1: Crear spartito (en mezzala workspace)
  ├── S1.1 Esqueleto del crate
  ├── S1.2 types.rs (~25 tests)
  ├── S1.3 story_format.rs (~25 tests)
  ├── S1.4 dod.rs + dor.rs (~10 tests)
  ├── S1.5 workflow.rs — CanonicalWorkflow + ConfigurableWorkflow (~40 tests)
  ├── S1.6 config.rs — WorkflowConfig + deserialización (~15 tests)
  └── S1.7 lib.rs, docs, cargo test (~115 tests total)

Fase S2: Migrar regista
  ├── S2.1 Añadir spartito como dependencia
  ├── S2.2 domain/state.rs → wrapper con re-exports
  ├── S2.3 domain/workflow.rs → ELIMINAR
  ├── S2.4 domain/story.rs → delegar a spartito::story_format
  ├── S2.5 domain/deadlock.rs → usar &dyn Workflow
  ├── S2.6 domain/prompts.rs → prompts con bifurcaciones
  ├── S2.7 config.rs → añadir campo workflow
  ├── S2.8 app/pipeline.rs → Box<dyn Workflow> (cambio crítico)
  ├── S2.9 app/board.rs → workflow.state_order()
  ├── S2.10 app/validate.rs → validar workflow custom
  ├── S2.11 tests/architecture.rs → actualizar reglas
  └── S2.12 cargo test → 357 tests pasando

Fase S3: Verificar mezzala
  └── cargo test en workspace completo

Fase S4: Publicar
  ├── cargo publish -p spartito (v0.1.0)
  └── Actualizar regista a spartito = "0.1" desde crates.io
```
