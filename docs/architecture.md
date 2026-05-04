# Arquitectura de regista

## Motivación

`main.rs` superó las 1300 líneas mezclando definición de CLI, handlers, helpers y tests.  
Módulos como `orchestrator.rs` importan de 11 módulos distintos — sin capas claras.

Este documento define la **arquitectura objetivo** con reglas de dependencia verificables.

---

## Principios

1. **Dependencia hacia adentro**: las capas externas dependen de las internas, nunca al revés.
2. **Domain sin IO**: la lógica de negocio no hace filesystem, red, ni procesos.
3. **Infra sin lógica de negocio**: los adaptadores solo traducen entre el mundo exterior y el dominio.
4. **CLI fina**: `main.rs` y `cli/` solo parsean argumentos y delegan.

---

## Estructura de directorios

```
src/
├── main.rs                    ← ~50 líneas: parsear CLI, init tracing, dispatch
│
├── cli/                       ← CAPA 1: Interfaz de usuario
│   ├── mod.rs
│   ├── args.rs                ← Structs #[derive(Parser, Args)] de clap
│   └── handlers.rs            ← Funciones handle_*(): reciben args, llaman app, exit codes
│
├── app/                       ← CAPA 2: Casos de uso / Orquestación
│   ├── mod.rs
│   ├── plan.rs                ← Generación de backlog (antes plan.rs)
│   ├── pipeline.rs            ← Loop del pipeline (antes orchestrator.rs)
│   ├── validate.rs            ← Validación pre-vuelo (antes validator.rs)
│   ├── init.rs                ← Scaffolding (antes init.rs)
│   ├── board.rs               ← Tablero Kanban (antes board.rs)
│   └── update.rs              ← Auto-update (antes update.rs)
│
├── domain/                    ← CAPA 3: Lógica de negocio pura
│   ├── mod.rs
│   ├── state.rs               ← Status, Actor, Transition, máquina de estados
│   ├── story.rs               ← Story, parseo de .md, set_status()
│   ├── graph.rs               ← DependencyGraph, DFS, ciclos
│   ├── deadlock.rs            ← Análisis de deadlocks, priorización
│   └── prompts.rs             ← PromptContext, generación de prompts
│
├── infra/                     ← CAPA 4: Infraestructura (IO, servicios externos)
│   ├── mod.rs
│   ├── providers.rs           ← Trait AgentProvider + Pi/Claude/Codex/OpenCode
│   ├── agent.rs               ← invoke_with_retry()
│   ├── daemon.rs              ← detach(), status(), kill(), follow()
│   ├── checkpoint.rs          ← OrchestratorState, save/load/remove
│   ├── git.rs                 ← snapshot(), rollback(), init_git()
│   └── hooks.rs               ← run_hook()
│
└── config.rs                  ← Configuración (transversal a todas las capas)
```

---

## Reglas de dependencia

### Matriz de permisos

| Origen ↓ / Destino → | cli | app | domain | infra | config |
|-----------------------|-----|-----|--------|-------|--------|
| **cli/**              |  ✅  |  ✅  |   ✅   |  ✅   |   ✅   |
| **app/**              |  ❌  |  ✅  |   ✅   |  ✅   |   ✅   |
| **domain/**           |  ❌  |  ❌  |   ✅   |  ❌   |   ❌   |
| **infra/**            |  ❌  |  ❌  |   ❌   |  ✅   |   ✅   |
| **config.rs**         |  ❌  |  ❌  |   ❌   |  ❌   |   —    |

### Reglas detalladas

| Regla | Descripción | Justificación |
|-------|-------------|---------------|
| **R1** | `domain/` solo depende de `std` y crates externos. No importa `crate::infra`, `crate::app`, `crate::cli`, ni `crate::config`. | La lógica de negocio debe ser portable y testeable sin IO. |
| **R2** | `infra/` solo depende de `config.rs` y otros módulos de `infra/`. No importa `crate::domain`, `crate::app`, ni `crate::cli`. | Los adaptadores no deben contener lógica de negocio. |
| **R3** | `app/` puede depender de `domain/`, `infra/`, y `config`. No importa `crate::cli`. | Los casos de uso orquestan dominio + infraestructura. |
| **R4** | `cli/` puede depender de todo excepto lógica interna de otros handlers. | La CLI coordina todo. Es la capa más externa. |
| **R5** | `config.rs` no depende de nada del crate (excepto `std`). | La configuración es datos puros. |

### Excepciones conocidas (deuda técnica temporal)

| Módulo actual | Problema | Solución en el refactor |
|---------------|----------|------------------------|
| `prompts.rs` → `config::StackConfig` | Domain depende de config | Mover `StackConfig` a `domain/` o crear struct separado en domain |
| `orchestrator.rs` → 11 módulos | God module | Separar en `app/pipeline.rs` + delegar a domain e infra |

---

## Flujo de control típico

```
Usuario ejecuta "regista run --story STORY-001"
        │
        ▼
   main.rs           ← Cli::parse() → Commands::Run(args)
        │
        ▼
   cli/handlers.rs   ← handle_run(args):
        │               1. Resolver proyecto
        │               2. Cargar config
        │               3. Construir RunOptions
        │               4. Decidir: dry-run? daemon? directo?
        │
        ▼
   app/pipeline.rs   ← orchestrator::run(project, config, options, resume)
        │               Orquesta el loop principal:
        │                 - Carga historias via domain::story
        │                 - Detecta deadlocks via domain::deadlock
        │                 - Invoca agentes via infra::agent
        │                 - Guarda checkpoint via infra::checkpoint
        │
        ├──► domain/state.rs     ← máquina de estados
        ├──► domain/story.rs     ← modelo y parseo
        ├──► domain/graph.rs     ← grafo de dependencias
        ├──► domain/deadlock.rs  ← análisis
        ├──► domain/prompts.rs   ← generación de prompts
        │
        ├──► infra/providers.rs  ← trait AgentProvider
        ├──► infra/agent.rs      ← invoke_with_retry
        ├──► infra/checkpoint.rs ← persistencia
        ├──► infra/git.rs        ← snapshots
        └──► infra/hooks.rs      ← hooks
```

---

## Transición desde el estado actual

El refactor es **mecánico**: mover archivos a subdirectorios sin cambiar su contenido (salvo ajustes de `mod` y `use`).

### Fases

| Fase | Acción | Riesgo |
|------|--------|--------|
| **0** | Crear `tests/architecture.rs` con las reglas (este test falla ahora → verde tras refactor) | Ninguno |
| **1** | Crear directorios `cli/`, `app/`, `domain/`, `infra/` | Ninguno |
| **2** | Mover módulos existentes a su capa: `state.rs` → `domain/`, `providers.rs` → `infra/`, etc. | Bajo |
| **3** | Actualizar `mod.rs` de cada capa con `pub mod ...` | Bajo |
| **4** | Actualizar `use crate::...` para usar rutas nuevas (`use crate::domain::state`) | Medio |
| **5** | Extraer handlers de `main.rs` a `cli/handlers.rs` | Medio |
| **6** | Extraer Args de `main.rs` a `cli/args.rs` | Bajo |
| **7** | Mover tests de `main.rs` a `cli/args.rs` (como `#[cfg(test)]`) | Bajo |
| **8** | `cargo test` → todo verde | — |

---

## Verificación automática

El archivo `tests/architecture.rs` contiene tests que verifican las reglas R1-R5.

Para ejecutar solo los tests de arquitectura:

```bash
cargo test architecture
```

Si alguna regla se viola, el test falla con un mensaje indicando:
- Qué archivo viola la regla
- Qué regla se violó
- Qué import hizo falta

Esto permite que en CI se detecten automáticamente violaciones de la arquitectura.
