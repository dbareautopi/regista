# 🏗️ regista v1.0 — Diseño del rework

**Orquestador de agentes LLM agnóstico al propósito.** Regista coordina
modelos de lenguaje directamente (sin tools CLI externas) siguiendo un
workflow configurable con estados, roles y prompts definidos por el usuario.

Presets de fábrica (desarrollo de software, research, single-agent) ofrecen
experiencia out-of-the-box sin configuración.

---

## 1. Visión

Regista v0.x era un **orquestador especializado en desarrollo de software**
con 4 roles canónicos (PO, QA, Dev, Reviewer), 14 transiciones fijas, y
dependencia de tools CLI externas (`pi`, `claude`, `codex`, `opencode`).

Regista v1.0 rompe las tres ataduras:

| v0.x | v1.0 |
|---|---|
| Solo desarrollo de software | **Cualquier dominio** definible vía workflow |
| Invoca binarios CLI externos | **Llama APIs de LLM directamente** (OpenAI, Anthropic, Ollama) |
| 4 roles + 14 transiciones fijas | **Workflow configurable** en TOML con presets |

Las tools CLI ya no escalan con modelos cada vez más capaces que necesitan
menos supervisión y más contexto multi-turn. Regista v1.0 gestiona la
conversación directamente, con historial completo por task.

---

## 2. Arquitectura

Misma arquitectura en 4 capas verificada por `tests/architecture.rs`, pero
con módulos reescritos:

```
cli/            ← 🟢 CLI: args + handlers (adaptados al nuevo modelo)
app/            ← 🟡 Casos de uso (importa domain + infra + config)
  pipeline.rs     ← Loop genérico: lookup config → fase → LLM invoke
  board.rs        ← Columnas dinámicas desde workflow
  plan.rs         ← Spec → tasks con agente genérico
  init.rs         ← Scaffolding con presets
  validate.rs     ← Valida config + tasks
  health.rs       ← Métricas (se conserva)
  update.rs       ← Auto-update (se conserva)
  presets/        ← Presets de fábrica embebidos como constantes
domain/         ← 🔴 Lógica pura (NO importa otras capas)
  task.rs         ← Task genérico con campos configurables
  workflow.rs     ← ConfigurableWorkflow desde TOML
  graph.rs        ← Grafo de dependencias (se conserva)
  deadlock.rs     ← Detección de bloqueos (adaptado)
  prompts.rs      ← Sistema de templates
  state.rs        ← SharedState (se conserva)
infra/          ← 🔵 Infraestructura: I/O, HTTP
  llm/            ← ✨ NUEVO: cliente HTTP multi-provider
    mod.rs          ← Trait LlmProvider + factory
    openai.rs       ← OpenAI + Ollama (mismo formato API, distinto base_url)
    anthropic.rs    ← Anthropic Messages API
    types.rs        ← Message, ChatRequest, ChatResponse, TokenUsage
  checkpoint.rs   ← Persistencia de sesión (se conserva)
  daemon.rs       ← Modo daemon (se conserva)
  git.rs          ← Snapshots/rollback (se conserva)
  hooks.rs        ← Hooks post-fase (se conserva)
config.rs       ← ⚪ Expandido: modelos, roles, fases, presets, tasks
```

### Lo que desaparece

| Módulo | Razón |
|---|---|
| `infra/providers.rs` | Ya no se invocan CLI tools externas |
| `infra/agent.rs` | `invoke_with_retry` contra procesos → `infra/llm/` contra HTTP |
| `domain/story.rs` | Formato STORY-NNN hardcodeado → `domain/task.rs` genérico |
| `domain/state.rs` (Status/Actor/Transition) | Tipos fijos → tipos configurables desde TOML |
| `domain/workflow.rs` (CanonicalWorkflow) | 14 transiciones fijas → DAG configurable |
| `domain/prompts.rs` (7 prompts hardcodeados) | TDD/DoD fijos → templates con variables |

### Lo que se conserva tal cual

| Módulo | Líneas |
|---|---|
| `infra/git.rs` — snapshots/rollback | 390 |
| `infra/checkpoint.rs` — persistencia de sesión | 203 |
| `infra/daemon.rs` — modo background | 580 |
| `infra/hooks.rs` — comandos post-fase | 143 |
| `domain/graph.rs` — DAG + detección de ciclos | 218 |
| `app/health.rs` — métricas | 806 |
| `app/update.rs` — auto-update | 149 |
| `domain/state.rs` — SharedState | ~150 |

**~2,600 líneas intactas.** El resto se reescribe o se adapta.

---

## 3. Cliente LLM nativo (`infra/llm/`)

### 3.1 Trait `LlmProvider`

```rust
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    fn chat(
        &self,
        messages: Vec<Message>,
        model: &str,
        timeout: Duration,
    ) -> Result<ChatResponse>;

    fn provider_name(&self) -> &str;
}

pub struct Message {
    pub role: String,       // "system", "user", "assistant"
    pub content: String,
}

pub struct ChatResponse {
    pub content: String,
    pub finish_reason: String,  // "stop", "length", "tool_calls"
    pub token_usage: TokenUsage,
}
```

### 3.2 Providers implementados

| Provider | API | Base URL | Auth |
|---|---|---|---|
| `OpenAiProvider` | chat/completions | `https://api.openai.com/v1` | Bearer token |
| `OllamaProvider` | chat/completions (mismo formato OpenAI) | configurable | opcional |
| `AnthropicProvider` | messages | `https://api.anthropic.com/v1` | x-api-key |

### 3.3 Configuración de modelos

```toml
[models.gpt4o]
provider = "openai"
model_id = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"

[models.claude]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
api_key = "${ANTHROPIC_API_KEY}"

[models.ollama_local]
provider = "openai"          # Ollama utiliza el formato de API de OpenAI
model_id = "llama3:70b"
api_key = ""                  # Ollama no requiere auth por defecto
base_url = "http://localhost:11434/v1"
```

### 3.4 Funcionalidades

- **Multi-turn**: el pipeline pasa el historial completo de conversación por task
- **Retry con backoff exponencial**: adaptado de la lógica actual de `agent.rs`
- **Timeout configurable** por fase
- **Sin streaming en v1** (se añadirá después)
- **Rate limiting**: delay entre llamadas configurable por provider

---

## 4. Workflow configurable

### 4.1 Concepto

Un workflow es un **grafo dirigido de fases**. Cada fase define:
- Un estado de origen y destino
- Un rol (system prompt + modelo asignado)
- Un prompt template con variables de contexto
- Política de rechazo (a qué estado volver, máximo de ciclos)

```
┌──────────┐  plan   ┌───────┐  implement  ┌────────┐  review  ┌──────┐
│  draft   │────────▶│ ready │────────────▶│ review │────────▶│ done │
└──────────┘         └───────┘             └────────┘         └──────┘
                         ▲                      │
                         │      reject          │
                         └──────────────────────┘
```

### 4.2 Configuración TOML

```toml
[workflow]
preset = "software-dev"  # "software-dev" | "research" | "single-agent" | custom

[workflow.states]
initial = "draft"
terminal = ["done", "failed"]

[workflow.task_format]
id_pattern = "TASK-\\d+"
fields = ["description", "priority"]
section_markers = { status = "## Status" }

# ── Roles ──────────────────────────────────────

[[workflow.roles]]
name = "developer"
system_prompt = """Eres un desarrollador senior. ..."""
model = "gpt4o"

[[workflow.roles]]
name = "reviewer"
system_prompt = """Eres un revisor de código. ..."""
model = "claude"

# ── Fases ──────────────────────────────────────

[[workflow.phases]]
name = "plan"
from = "draft"
to = "ready"
role = "product_owner"
model = "claude"
prompt = """
Refina la tarea {{task_id}}.
{{task_fields}}
Responde con [STATUS: ready] o [REJECT: motivo].
"""
on_reject = "draft"
max_reject_cycles = 3
```

### 4.3 Tipos en Rust

```rust
pub struct WorkflowConfig {
    pub preset: Option<String>,
    pub states: StatesConfig,
    pub task_format: TaskFormatConfig,
    pub roles: Vec<RoleConfig>,
    pub phases: Vec<PhaseConfig>,
}

pub struct PhaseConfig {
    pub name: String,
    pub from: String,
    pub to: String,
    pub role: String,
    pub model: String,          // referencia a [models.xxx]
    pub prompt: String,         // template con {{variables}}
    pub on_reject: Option<String>,
    pub max_reject_cycles: Option<u32>,
    pub timeout_seconds: Option<u64>,
}
```

---

## 5. Task genérico (`domain/task.rs`)

### 5.1 Formato de archivo (.md)

El formato de task es configurable. El preset define los campos esperados.

Ejemplo con el preset `software-dev` (compatible con el formato actual):

```markdown
# STORY-001: Login de usuarios

## Status
**draft**

## Epic
EPIC-001

## Descripción
Implementar pantalla de login...

## Criterios de aceptación
- [ ] CA1: El usuario puede iniciar sesión con email y contraseña
- [ ] CA2: Muestra error si las credenciales son inválidas

## Dependencias
- Bloqueado por: STORY-002

## Activity Log
- 2026-05-08 | product_owner | tarea creada
```

Ejemplo con formato genérico:

```markdown
# TASK-001: Investigar mercado de X

## Status
**pending**

## Descripción
Analizar las 3 principales empresas...

## Priority
high

## Activity Log
- 2026-05-08 | researcher | tarea iniciada
```

### 5.2 Estructura Rust

```rust
pub struct Task {
    pub id: String,                     // TASK-001 o STORY-001 según preset
    pub path: PathBuf,
    pub status: String,                 // cualquier string, no un enum fijo
    pub fields: HashMap<String, String>, // campos definidos en task_format
    pub blockers: Vec<String>,
    pub activity_log: Vec<LogEntry>,
    pub raw_content: String,
    pub reject_cycles: u32,
}

pub struct LogEntry {
    pub date: String,
    pub actor: String,
    pub description: String,
}
```

### 5.3 Parseo

El parseo usa las secciones definidas en `task_format.section_markers`:

```toml
[workflow.task_format]
id_pattern = "TASK-\\d+"
section_markers = { status = "## Status", description = "## Descripción" }
dependency_marker = "Bloqueado por:"
```

El parser es genérico: extrae cualquier sección cuyo marcador esté en `section_markers`,
y el resto lo almacena en `raw_content` para inyectarlo en los prompts.

---

## 6. Pipeline genérico (`app/pipeline.rs`)

### 6.1 Loop principal

```
loop {
    tasks = load_all_tasks(stories_dir, task_format)
    if all_terminal(tasks): break → PipelineComplete

    // Transiciones automáticas (sin agente)
    apply_automatic_transitions(tasks, graph, workflow)
        // task → blocked si dependencias no resueltas
        // task → unblocked si todas las dependencias done/failed
        // task → failed si reject_cycles > max

    // Detección de deadlock
    resolution = analyze_deadlock(tasks, graph, workflow)
    if resolution == PipelineComplete: break

    // Seleccionar siguiente task
    task = pick_next_actionable(tasks, workflow)

    // Buscar fase aplicable
    phases = workflow.phases_for_status(task.status)
    phase = select_phase(phases, task)  // si bifurcación, el LLM elige

    // Construir prompt
    prompt = render_template(phase.prompt, task, context)

    // Invocar LLM
    messages = build_messages(phase.role.system_prompt, prompt, task.history)
    response = llm.chat(messages, phase.model)

    // Parsear respuesta
    action = parse_agent_action(response.content)
    apply_action(task, action, phase)
    checkpoint.save()
}
```

### 6.2 Parseo de respuesta del agente

El prompt incluye instrucciones de formato estricto. El agente debe responder con:

- `[STATUS: <estado>]` — transición exitosa
- `[REJECT: <motivo>]` — rechazo, vuelve a `on_reject`
- `[DEPENDS_ON: TASK-XXX]` — añade dependencia
- `[BLOCKED: <motivo>]` — se bloquea manualmente

El orquestador parsea la respuesta con regex y valida que el estado destino
esté definido en el workflow. Si el agente no sigue el formato, se reintenta
con feedback.

### 6.3 Sistema de prompts con templates

```rust
pub fn render_template(template: &str, task: &Task, context: &HashMap<String, String>) -> String {
    // {{task_id}} → task.id
    // {{task_status}} → task.status
    // {{task_fields.description}} → task.fields["description"]
    // {{task_fields.*}} → bullet list de todos los campos
    // {{last_rejection}} → última entrada del activity log con "reject"
    // {{blockers}} → lista de dependencias
    // {{context.foo}} → valor de contexto inyectado
}
```

---

## 7. Presets de fábrica

Embebidos como constantes de Rust en `app/presets/`. El usuario elige uno
con `regista init --preset <name>`.

### 7.1 `software-dev`

Pipeline de desarrollo de software. 3 fases simplificadas (modelos modernos
no necesitan la separación QA/Dev/Reviewer de v0.x):

| Fase | De | A | Rol | Modelo |
|---|---|---|---|---|
| **plan** | draft | ready | product_owner | claude-sonnet-4 |
| **implement** | ready | review | developer | claude-sonnet-4 |
| **validate** | review | done | reviewer | gpt-4o |

Formato de task: STORY-NNN con CA, épicas, dependencias (compatible con v0.x).

Rechazos: implement → ready, review → ready.

### 7.2 `research`

Pipeline de investigación. 2 fases:

| Fase | De | A | Rol | Modelo |
|---|---|---|---|---|
| **research** | pending | draft | researcher | claude-sonnet-4 |
| **report** | draft | done | analyst | claude-sonnet-4 |

Formato de task: TASK-NNN con topic, depth, sources.

### 7.3 `single-agent`

Pipeline mínimo: 1 fase, 1 rol. El agente recibe la task y decide si está
completada o no.

| Fase | De | A | Rol | Modelo |
|---|---|---|---|---|
| **execute** | pending | done | agent | gpt-4o |

---

## 8. Configuración completa (`.regista/config.toml`)

```toml
[project]
stories_dir    = ".regista/tasks"
task_pattern   = "TASK-*.md"
decisions_dir  = ".regista/decisions"
log_dir        = ".regista/logs"

[limits]
max_iterations            = 0    # 0 = auto: nº tasks × 6 (mín 10)
max_reject_cycles         = 8
agent_timeout_seconds     = 1800
max_wall_time_seconds     = 28800
retry_delay_base_seconds  = 10

[hooks]
post_phase = "cargo build && cargo test"   # ejecutado tras cada fase

[git]
enabled = true

# ── Modelos LLM ──────────────────────────────

[models.gpt4o]
provider = "openai"
model_id = "gpt-4o"
api_key = "${OPENAI_API_KEY}"

[models.claude]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
api_key = "${ANTHROPIC_API_KEY}"

# ── Workflow ─────────────────────────────────

[workflow]
preset = "software-dev"
```

---

## 9. CLI

Mismos comandos, adaptados a los nuevos conceptos:

```
regista init --preset software-dev    # scaffolding con preset
regista init --preset research
regista run                           # ejecuta workflow configurado
regista run --dry-run                 # simulación sin LLM
regista run --resume                  # reanudar desde checkpoint
regista plan spec.md                  # generar tasks desde spec
regista board                         # dashboard (columnas del workflow)
regista board --json
regista validate                      # chequeo de config + tasks
regista logs / status / kill          # gestión del daemon (sin cambios)
regista update                        # auto-update (sin cambios)
```

---

## 10. Compatibilidad y migración

### 10.1 Para usuarios de v0.x

El preset `software-dev` mantiene el formato de historia (STORY-NNN, CA, épicas,
dependencias, Activity Log). Los proyectos existentes con `.regista/stories/`
funcionan tras:

1. Instalar regista v1.0
2. Ejecutar `regista init --preset software-dev` en el proyecto (no pisa historias)
3. Configurar API keys en `.regista/config.toml`
4. `regista run`

### 10.2 Sin dependencia de spartito

Spartito era el contrato compartido con `mezzala`. Al eliminar la dependencia
de tools CLI externas, spartito deja de ser necesario. El contrato (formato de
task, estados, transiciones) lo define el usuario en `.regista/config.toml`.
Los presets de fábrica reemplazan el `CanonicalWorkflow`.

---

## 11. Plan de implementación

**Rama**: `rework` (desarrollo independiente, `main` no se toca).

### Fase 1 — Cliente LLM nativo (2 semanas)

- Nuevo: `infra/llm/` — `mod.rs`, `openai.rs`, `anthropic.rs`, `types.rs`
- Dependencia nueva: `reqwest` con `rustls-tls`
- Soporte: OpenAI + Anthropic + Ollama (vía formato OpenAI)
- Multi-turn, retry con backoff, timeout, rate limiting
- Tests con mock server

### Fase 2 — Dominio genérico (2 semanas)

- Nuevo: `domain/task.rs` — Task con campos configurables
- Nuevo: `domain/workflow.rs` — ConfigurableWorkflow desde TOML
- Nuevo: `domain/prompts.rs` — templates con `{{variables}}`
- Adaptar: `domain/deadlock.rs` — usar Task y Workflow genéricos
- Adaptar: `config.rs` — añadir modelos, roles, fases, task_format

### Fase 3 — Pipeline genérico (2 semanas)

- Reescribir: `app/pipeline.rs` — loop con lookup dinámico de fases
  - `process_task()`: build_messages → llm.chat → parse_action → apply
  - `apply_automatic_transitions()`: dependencias y max_reject_cycles
  - `analyze_deadlock()`: adaptado a tasks genéricos
- Parseo de respuesta del agente: regex `[STATUS: X]` / `[REJECT: Y]`

### Fase 4 — CLI + Presets (2 semanas)

- `app/presets/` — `software-dev`, `research`, `single-agent`
- Adaptar `app/init.rs` — scaffolding con presets
- Adaptar `app/plan.rs` — spec → tasks con agente genérico
- Adaptar `app/board.rs` — columnas dinámicas desde workflow
- Adaptar `app/validate.rs` — validar config + tasks + modelos
- Adaptar `cli/args.rs` y `cli/handlers.rs`

### Fase 5 — Limpieza (1 semana)

- Eliminar: `infra/providers.rs`, `infra/agent.rs`
- Eliminar: `domain/story.rs`, tipos `Status`/`Actor`/`Transition`
- Eliminar: `domain/workflow.rs` (CanonicalWorkflow)
- Eliminar: prompts hardcodeados
- Quitar `ureq` de Cargo.toml, añadir `reqwest`
- Limpiar imports, actualizar `tests/architecture.rs`

### Fase 6 — Tests (1-2 semanas)

- Tests del cliente LLM con mock server
- Tests de ConfigurableWorkflow cargando TOML
- Tests de Task genérico con distintos formatos
- Tests de pipeline con mock LLM provider
- Tests de presets
- Actualizar tests de arquitectura

---

## 12. Riesgos

| Riesgo | Mitigación |
|---|---|
| **Parseo de respuesta del agente** — si no sigue el formato `[STATUS: X]` | Prompt muy estricto + retry con feedback + timeout |
| **reqwest + rustls** — nueva dependencia async | Es el estándar de facto. Compatible con tokio. |
| **Gestión de API keys** — seguridad | Variables de entorno (`${VAR}`) + archivo `.env` |
| **Coste de API calls** — sin límites visibles | Token tracking desde v1 + health metrics |
| **Modelos que rechazan seguir instrucciones de formato** | System prompt fuerte + ejemplos en el prompt |
