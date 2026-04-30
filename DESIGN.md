# 🏗️ regista — Diseño

🎬 AI agent director para `pi`. Independiente del proyecto: no sabe nada
de Rust, de Purist, ni de qué construyen los agentes. Solo sabe tres cosas:

1. **Dónde están las historias** y cómo leer su estado
2. **Qué skills de `pi` invocar** para cada rol del workflow
3. **La máquina de estados fija** que gobierna las transiciones

El proyecto anfitrión se configura mediante un archivo `.regista.toml` en su raíz.

---

## 1. Configuración (`.regista.toml`)

```toml
[project]
# Dónde encontrar los artefactos del workflow
stories_dir    = "product/stories"
story_pattern  = "STORY-*.md"          # glob para encontrar historias
epics_dir      = "product/epics"       # opcional: para filtrar por épica
decisions_dir  = "product/decisions"
log_dir        = "product/logs"

[agents]
# Rutas a los skills de pi (relativas a la raíz del proyecto)
product_owner = ".pi/skills/product-owner/SKILL.md"
qa_engineer   = ".pi/skills/qa-engineer/SKILL.md"
developer     = ".pi/skills/developer/SKILL.md"
reviewer      = ".pi/skills/reviewer/SKILL.md"

[limits]
max_iterations        = 10
max_retries_per_step  = 5
max_reject_cycles     = 3
agent_timeout_seconds = 1800
max_wall_time_seconds = 28800
retry_delay_base_seconds = 10

[hooks]
# Comandos opcionales que se ejecutan tras cada fase para verificar artefactos.
# Si fallan, se hace rollback. Si no se definen, se salta la verificación.
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt -- --check"
post_reviewer = "cargo test && cargo clippy -- -D warnings"

[git]
enabled = true    # si false, no se usan snapshots/rollback
```

---

## 2. Máquina de Estados

### 2.1 Diagrama

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

### 2.2 Tabla canónica de transiciones

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
> al evaluar el grafo de dependencias y los contadores de ciclos. No invocan agentes.

### 2.3 Tipo en Rust

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    Draft,
    Ready,
    TestsReady,
    InProgress,
    InReview,
    BusinessReview,
    Done,
    Blocked,
    Failed,
}

pub enum Actor {
    ProductOwner,
    QaEngineer,
    Developer,
    Reviewer,
    Orchestrator,
}
```

---

## 3. Detección de bloqueos (deadlock)

Antes de cada iteración, el orquestador evalúa si existe **progreso posible**.
Si no, identifica la causa y dispara al agente corrector (casi siempre el PO).

### 3.1 Algoritmo

```
Para cada historia no terminal (≠ Done, ≠ Failed):

  1. Si status == Draft         → "stuck": necesita PO (groom)     [caso A]
  2. Si status == Blocked:
     a. Si algún bloqueador está en Draft → "stuck": PO debe groom [caso B]
     b. Si hay ciclo de dependencias     → "stuck": PO debe romper [caso C]
     c. Si todos los bloqueadores están Done → automático → Ready    [OK]
  3. Si status está en el flujo normal (Ready, Tests Ready, InProgress,
     InReview, BusinessReview) → el propio loop lo maneja (retry,
     reject cycles, etc.)

Si tras evaluar todas las historias:
  - ninguna es "accionable" por el loop normal
  Y
  - existen historias "stuck"
  → disparar al PO para la historia stuck de mayor prioridad
    (la que desbloquea más historias)

Si no hay historias stuck Y no hay accionables → pipeline completo (todas Done/Failed)
```

### 3.2 Prioridad de desbloqueo

Se elige la historia que **desbloquea más historias** (conteo de referencias inversas
en el grafo de dependencias). En caso de empate, el ID secuencial más bajo.

---

## 4. Arquitectura del crate

```
regista/
├── Cargo.toml
├── DESIGN.md                  ← este documento
├── src/
│   ├── main.rs                ← CLI (clap), entrada
│   ├── config.rs              ← Config, carga de .regista.toml
│   ├── state.rs               ← Status, Actor, Transition, allowed_transitions()
│   ├── story.rs               ← Story, parseo de .md, set_status()
│   ├── dependency_graph.rs    ← Grafo de dependencias, ciclo DFS, refs inversas
│   ├── deadlock.rs            ← Detección de stuck, priorización
│   ├── orchestrator.rs        ← Loop principal, process_story()
│   ├── agent.rs               ← Invocación de `pi` con timeout + retry
│   ├── prompts.rs             ← Generación de prompts por actor y transición
│   ├── hooks.rs               ← Comandos post-fase opcionales
│   └── git.rs                 ← Snapshot + rollback (si git.enabled)
└── tests/
    ├── state_tests.rs
    ├── deadlock_tests.rs
    ├── story_tests.rs
    └── fixtures/
        ├── story_draft.md
        ├── story_blocked.md
        ├── story_business_review.md
        └── ...
```

---

## 5. Formato de historia esperado (contrato fijo)

```markdown
# STORY-NNN: Título

## Status
**<Draft|Ready|Tests Ready|In Progress|In Review|Business Review|Done|Blocked|Failed>**

## Epic
EPIC-XXX

## Descripción
...

## Criterios de aceptación
- [ ] CA1
- [ ] CA2

## Dependencias       ← opcional
- Bloqueado por: STORY-XXX, STORY-YYY

## Activity Log
- 2026-04-30 | PO | Movida de Draft a Ready
```

Reglas de parseo:
- **Status**: busca `## Status`, lee la línea siguiente, extrae valor entre `**...**` o texto limpio
- **Bloqueadores**: busca `Bloqueado por:` (case-insensitive), extrae `STORY-\d+`
- **Epic**: busca `## Epic`, lee la línea siguiente, extrae `EPIC-\d+`
- **Activity Log**: busca `## Activity Log`, lee hasta la siguiente sección `## ...`. Última línea con "rechaz" se usa para prompt de Dev fix.

---

## 6. CLI

```
regista <PROJECT_DIR> [FLAGS]

FLAGS:
  --config <FILE>         Ruta al archivo de configuración
                          [default: <PROJECT_DIR>/.regista.toml]

  --epics <RANGE>         Filtrar por rango de épicas ("EPIC-001..EPIC-003")
  --epic <ID>             Filtrar por una sola épica

  --story <ID>            Procesar solo una historia concreta
  --once                  Ejecutar una sola iteración y salir

  --detach                Lanzar en segundo plano (modo daemon)
  --follow                Ver log en vivo de un orquestador corriendo
  --status                Consultar si el orquestador sigue vivo
  --kill                  Detener orquestador en segundo plano

  --log-file <FILE>       Ruta específica para el archivo de log
```

---

## 7. Plan de implementación

| Fase | Qué | Resultado |
|------|-----|-----------|
| **F1** | Estructura del crate, `Cargo.toml`, `main.rs` con clap | Binario compila, acepta flags |
| **F2** | `config.rs` — carga de `.regista.toml` | Lee configuración |
| **F3** | `state.rs` — Status, Actor, Transition, allowed_transitions | Tipo de la máquina de estados |
| **F4** | `story.rs` — parseo de .md, set_status() | Lee y escribe historias |
| **F5** | `dependency_graph.rs` — grafo, ciclo DFS, conteo inverso | Grafo de dependencias |
| **F6** | `deadlock.rs` — detección y priorización | Decide qué desbloquear |
| **F7** | `agent.rs` — pi con timeout + retry | Ejecuta agentes |
| **F8** | `prompts.rs` — prompts por transición | Prompts generados |
| **F9** | `orchestrator.rs` — loop principal, process_story | Pipeline completo |
| **F10** | `hooks.rs` + `git.rs` — verificación y rollback | Verificación post-fase |
| **F11** | Tests: estado, deadlock, parseo, integración | Cobertura de tests |
| **F12** | Wrapper `scripts/regista.sh` → binario | Reemplazo del .sh |
