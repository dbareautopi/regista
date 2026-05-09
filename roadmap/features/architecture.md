# 🏗️ Análisis arquitectónico — regista v1.0 rework

> **Rol**: Arquitecto
> **Fecha**: 2026-05-09
> **Objetivo**: Evaluar la arquitectura propuesta en DESIGN.md, identificar riesgos,
> y proponer mejoras antes de implementar las 24 historias del roadmap.

---

## 1. Arquitectura propuesta (DESIGN.md)

```
cli/            ← 🟢 CLI: args + handlers (importa cualquier capa)
app/            ← 🟡 Casos de uso: pipeline, board, plan, init, validate, health, update, presets/
domain/         ← 🔴 Lógica pura: task, workflow, prompts, graph, deadlock, state
infra/          ← 🔵 Infraestructura: llm/, checkpoint, daemon, git, hooks
config.rs       ← ⚪ Configuración (en raíz)
```

### Reglas de dependencia (R1-R5)

| Regla | Descripción | Estado actual |
|-------|-------------|---------------|
| R1 | `domain/` no importa otras capas del crate | ✅ Cumple (salvo prompts que importa `config`, corregido en v0.x) |
| R2 | `infra/` solo importa `config` | ✅ Cumple |
| R3 | `app/` no importa `cli/` | ✅ Cumple |
| R4 | `cli/` puede importar cualquier capa | ✅ Cumple |
| R5 | `config` no importa otras capas del crate | ❌ Violación: `AgentsConfig` importa `providers` |

---

## 2. Riesgos identificados en la arquitectura propuesta

### Riesgo A: `config.rs` crece descontroladamente

**Problema**: Con la v1.0, `config.rs` debe albergar: `ProjectConfig`, `LimitsConfig`, `ModelsConfig` (nuevo),
`WorkflowConfig` (nuevo con roles, fases, task_format), `HooksConfig`, `GitConfig`.
Un solo archivo fácilmente superará 500 líneas.

**Impacto**: Dificultad de mantenimiento, conflictos de merge frecuentes, violación de SRP.

### Riesgo B: Lógica de dominio acoplada a tipos de configuración

**Problema**: `ConfigurableWorkflow` vive en `domain/workflow.rs` pero se deserializa desde TOML
usando los mismos structs definidos en `config.rs`. Esto crea un acoplamiento implícito:
si cambia el formato TOML, cambian tanto `config.rs` como `domain/workflow.rs`.

**Impacto**: Cambios en cascada, tests frágiles, dificultad para versionar el formato de configuración.

### Riesgo C: `infra/llm/` sin separación de responsabilidades

**Problema**: El diseño actual mete en `infra/llm/` el trait, los providers, los tipos,
y la lógica de retry. Esto mezcla:
- Contrato (trait + tipos) → debería ser casi de dominio
- Implementaciones concretas (OpenAI, Anthropic) → infraestructura pura
- Políticas de reintento → lógica de aplicación

**Impacto**: Si se añade un nuevo provider (ej. Gemini), hay que tocar el módulo de retry.
Si cambia la política de backoff, hay que tocar el módulo de providers.

### Riesgo D: `app/presets/` como constantes Rust

**Problema**: Los presets embebidos como constantes requieren recompilar para cambiar un prompt.
No son inspeccionables por el usuario sin leer código fuente.

**Impacto**: Mala DX (developer experience). El usuario no puede ver ni tunear un preset sin
copiarlo a su `config.toml` manualmente.

### Riesgo E: Dependencia circular entre `config` y `domain`

**Problema**: `WorkflowConfig` se define en `config.rs` (con anotaciones serde) pero `domain/workflow.rs`
necesita usarlo para `ConfigurableWorkflow`. O bien `domain` importa `config` (viola R1),
o bien `config` importa `domain` (viola R5), o bien se duplican los tipos.

**Impacto**: Cualquiera de las tres opciones es problemática. Es el mismo problema que ya
se resolvió con `DomainStackConfig` en el refactor v0.x.

---

## 3. Arquitectura mejorada propuesta

### 3.1 Nueva estructura de capas

```
src/
├── main.rs
│
├── config/                    ← ⚪ Configuración: solo datos + serde (sin imports del crate)
│   ├── mod.rs                 ← Config, load(), save()
│   ├── project.rs             ← ProjectConfig (dirs, patterns)
│   ├── limits.rs              ← LimitsConfig (timeouts, iteraciones, rechazos)
│   ├── models.rs              ← ModelConfig, ModelsMap, expand_env()
│   ├── workflow.rs            ← WorkflowConfig, PhaseConfig, RoleConfig, TaskFormatConfig
│   ├── hooks.rs               ← HooksConfig
│   └── git.rs                 ← GitConfig
│
├── cli/                       ← 🟢 Presentación: args, handlers, tracing setup
│   ├── mod.rs
│   ├── args.rs
│   └── handlers.rs
│
├── app/                       ← 🟡 Aplicación: casos de uso
│   ├── mod.rs
│   ├── pipeline.rs            ← loop genérico (reescrito)
│   ├── board.rs               ← columnas dinámicas (adaptado)
│   ├── plan.rs                ← spec → tasks con LLM nativo (adaptado)
│   ├── init.rs                ← scaffolding con presets (adaptado)
│   ├── validate.rs            ← chequeo pre-vuelo genérico (adaptado)
│   ├── health.rs              ← métricas (conservado)
│   ├── update.rs              ← auto-update (conservado, migrado a reqwest)
│   └── presets/               ← presets de fábrica
│       ├── mod.rs             ← Preset trait + registry
│       ├── software_dev.rs    ← 3 fases: plan → implement → validate
│       ├── research.rs        ← 2 fases: research → report
│       └── single_agent.rs    ← 1 fase: execute
│
├── domain/                    ← 🔴 Dominio: lógica pura (sin imports del crate)
│   ├── mod.rs
│   ├── task.rs                ← Task genérico + parseo configurable
│   ├── workflow.rs            ← ConfigurableWorkflow (runtime, usa tipos de config)
│   ├── phase.rs               ← PhaseResolver, bifurcaciones, guards
│   ├── templates.rs           ← render_template() con {{variables}}
│   ├── graph.rs               ← DependencyGraph, DFS, ciclos
│   ├── deadlock.rs            ← analyze(), DeadlockResolution
│   ├── state.rs               ← SharedState, TokenCount
│   └── error.rs               ← DomainError (errores tipados de dominio)
│
└── infra/                     ← 🔵 Infraestructura: I/O, HTTP, procesos
    ├── mod.rs
    ├── llm/                   ← Cliente LLM multi-provider
    │   ├── mod.rs             ← trait LlmProvider + factory
    │   ├── types.rs           ← Message, ChatResponse, TokenUsage
    │   ├── openai.rs          ← OpenAiProvider (compatible Ollama)
    │   ├── anthropic.rs       ← AnthropicProvider
    │   └── retry.rs           ← invoke_with_retry(), backoff, rate limiting
    ├── checkpoint.rs          ← OrchestratorState save/load
    ├── daemon.rs              ← detach, status, kill, follow
    ├── git.rs                 ← snapshot, rollback, diff
    └── hooks.rs               ← run_hook()
```

### 3.2 Cambios clave respecto a DESIGN.md

| Cambio | Razón | Beneficio |
|--------|-------|-----------|
| `config.rs` → `config/` (7 sub-módulos) | Evita archivo monolítico de 500+ líneas | Cada aspecto de configuración se mantiene aislado |
| `domain/phase.rs` (nuevo) | Extrae `phases_for_status()` y lógica de bifurcaciones de `workflow.rs` | SRP: workflow define estructura, phase resuelve transiciones |
| `infra/llm/retry.rs` (nuevo) | Separa política de reintentos de la implementación de providers | Cambiar backoff no toca providers; añadir provider no toca retry |
| `domain/error.rs` (nuevo) | Errores tipados de dominio en lugar de `anyhow` | Mejor testabilidad, pattern matching en app layer |
| `app/presets/` con trait `Preset` | En lugar de constantes sueltas, un trait que permite listar, validar y serializar | `regista init --list-presets`, validación automática |
| Eliminación de `domain/story.rs`, `infra/providers.rs`, `infra/agent.rs` | Ya previsto en DESIGN.md Fase 5 | Sin cambios |

### 3.3 Reglas de dependencia actualizadas

| Regla | Descripción |
|-------|-------------|
| **R1** | `domain/` **solo** importa `std`, crates externos (`serde`, `regex`), y **tipos de `config/`** (structs de datos, no lógica). No importa `infra/`, `app/`, `cli/`. |
| **R2** | `infra/` solo importa `config/` (tipos) y crates externos (`reqwest`, `tokio`, `tracing`). No importa `domain/`, `app/`, `cli/`. |
| **R3** | `app/` importa `domain/`, `infra/`, `config/`. No importa `cli/`. |
| **R4** | `cli/` puede importar cualquier capa. |
| **R5** | `config/` **solo** importa `std`, `serde`, `toml`. No importa ninguna otra capa del crate. |
| **R6** | `domain/error.rs` solo importa `std::fmt`. No depende de `anyhow` ni `thiserror` (evita dependencias de infraestructura en dominio). |

> **Nota sobre R1**: La excepción de "tipos de config/" es necesaria porque `ConfigurableWorkflow`
> opera sobre `WorkflowConfig` y `PhaseConfig` que están definidos en `config/`. Para evitar
> el acoplamiento, `domain/` recibe estos tipos por referencia en sus funciones públicas
> (inyección de dependencias), no los construye ni los deserializa.

### 3.4 Contrato entre `config/` y `domain/`

El problema del acoplamiento `config ↔ domain` se resuelve con un patrón de **tipos compartidos
sin lógica**:

```
config/workflow.rs        ← define WorkflowConfig CON anotaciones serde (datos puros)
domain/workflow.rs        ← define ConfigurableWorkflow CON métodos de runtime
                           ↑ recibe &WorkflowConfig como parámetro
```

Es decir:
- `config/` **posee** los tipos de datos (structs con `#[derive(Deserialize)]`)
- `domain/` **consume** esos tipos para implementar la lógica de negocio
- `domain/` NO deserializa TOML, NO conoce `serde`
- `app/` es quien carga `Config` desde TOML, extrae `WorkflowConfig`, y lo pasa a `ConfigurableWorkflow::new(config)`

Esto cumple R1 (domain no depende de infraestructura de serialización) y R5 (config no depende de lógica de dominio).

### 3.5 Trait `Preset` para `app/presets/`

```rust
/// Un preset de fábrica que define un workflow completo.
pub trait Preset: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn workflow_config(&self) -> WorkflowConfig;
    fn default_models(&self) -> Vec<ModelConfig>;
    fn example_task(&self) -> &str;  // contenido .md de ejemplo
}

/// Registry de presets disponibles.
pub fn all_presets() -> Vec<Box<dyn Preset>> {
    vec![
        Box::new(software_dev::SoftwareDevPreset),
        Box::new(research::ResearchPreset),
        Box::new(single_agent::SingleAgentPreset),
    ]
}
```

Esto permite:
- `regista init --list-presets` → itera `all_presets()` y muestra nombre + descripción
- `regista init --preset research` → busca por nombre, genera config
- Validación automática: `Preset::workflow_config()` se puede validar con `ConfigurableWorkflow::validate()`

---

## 4. Matriz de trazabilidad: capas × historias

| Capa | Módulo | Historias |
|------|--------|-----------|
| **config/** | models, workflow | STORY-V10-005 |
| **domain/** | task | STORY-V10-006 |
| **domain/** | workflow | STORY-V10-007 |
| **domain/** | templates | STORY-V10-008 |
| **domain/** | deadlock, graph | STORY-V10-009 |
| **domain/** | error | (nuevo, sin historia propia) |
| **infra/llm/** | mod, types | STORY-V10-001 |
| **infra/llm/** | openai | STORY-V10-002 |
| **infra/llm/** | anthropic | STORY-V10-003 |
| **infra/llm/** | retry | STORY-V10-004 |
| **app/** | pipeline | STORY-V10-010, 011, 012 |
| **app/presets/** | software_dev | STORY-V10-013 |
| **app/presets/** | research, single_agent | STORY-V10-014 |
| **app/** | init | STORY-V10-015 |
| **app/** | board | STORY-V10-016 |
| **app/** | validate | STORY-V10-017 |
| **app/** | plan | STORY-V10-018 |
| **app/** + **infra/** | — (eliminación) | STORY-V10-019 |
| **domain/** | — (eliminación) | STORY-V10-020 |
| **tests/** | llm, domain, pipeline, presets | STORY-V10-021, 022, 023, 024 |

---

## 5. Recomendaciones para la implementación

1. **Empezar por `config/`**: Es la base de datos sin dependencias. Todos los demás módulos lo referencian.
2. **Crear `domain/error.rs` antes que `domain/task.rs`**: Los errores tipados deben estar listos para que `Task::load()` los use.
3. **`infra/llm/types.rs` no debe depender de `config/`**: Los tipos de mensaje son autónomos. La factory (`from_config`) es el punto de unión.
4. **`app/pipeline.rs` no debe conocer providers concretos**: Solo debe usar `Box<dyn LlmProvider>`. La factory en `infra/llm/mod.rs` es la única que instancia providers.
5. **Los tests de arquitectura deben actualizarse en cada fase**: No esperar a la Fase 6 para verificar que las reglas R1-R6 se cumplen.
6. **Mantener `anyhow` en `app/` y `cli/`**: Los errores tipados (`DomainError`) son para `domain/`. Las capas superiores pueden usar `anyhow` para ergonomía.
