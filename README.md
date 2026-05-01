# regista 🎬

> AI agent director for [`pi`](https://github.com/mariozechner/pi-coding-agent).  
> Orquestación autónoma del ciclo completo de desarrollo:  
> **PO → QA → Dev → Reviewer → Done.**

[![Crates.io](https://img.shields.io/crates/v/regista)](https://crates.io/crates/regista)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## ¿Qué hace?

`regista` toma un backlog de historias de usuario (archivos `.md`) y ejecuta
el pipeline completo de desarrollo de forma **autónoma**, disparando agentes
de `pi` según una máquina de estados formal:

```
Draft ──PO──→ Ready ──QA──→ Tests Ready ──Dev──→ In Review ──Reviewer──→ Business Review ──PO──→ Done
  ↑                                      ↑            ↑                      ↑                    ↑
  │                           QA corrige tests      │              Reviewer rechaza    PO rechaza/revalida
  └────────────────────────────────────────────────┴──────────────────────────────────────────────────┘
                              Con detección de deadlocks y desbloqueo automático
```

- **Deadlock detection**: si el grafo se estanca, prioriza la historia que más dependencias desbloquea
- **Checkpoint/resume**: guarda progreso tras cada iteración. Si algo interrumpe → `--resume`
- **Dry-run**: simula el pipeline completo sin gastar créditos de LLM
- **Salida JSON**: lista para CI/CD, con exit codes diferenciados (0 = éxito, 2 = fallos, 3 = parada temprana)

## Filosofía

Regista **no sabe nada de tu proyecto**. No le importa si usas Rust, Python
o lo que sea. Solo necesita tres cosas:

1. **Dónde están tus historias** (archivos `.md`)
2. **Qué skills de `pi`** actúan como PO, QA, Dev, Reviewer
3. **La máquina de estados fija** que gobierna las transiciones

Todo lo demás —código, tests, builds— lo manejan los agentes a través de las skills.

---

## Quick start

```bash
# 1. Instalar
cargo install regista

# 2. Crear un proyecto nuevo
cd mi-proyecto
regista init --with-example

# 3. Simular antes de ejecutar
regista --dry-run

# 4. Ejecutar el pipeline real
regista
```

---

## Instalación

```bash
# Desde crates.io
cargo install regista

# Desde el repositorio
git clone https://github.com/dbareautopi/regista
cd regista
cargo build --release
```

El binario queda en `~/.cargo/bin/regista` (añadido al PATH automáticamente por Rust).

---

## Estructura del proyecto

Todo lo que genera y gestiona `regista` vive bajo `.regista/` en la raíz:

```
mi-proyecto/
├── .regista/
│   ├── config.toml        ← configuración del pipeline
│   ├── stories/            ← historias de usuario (*.md)
│   │   ├── STORY-001.md
│   │   └── STORY-002.md
│   ├── epics/              ← épicas
│   ├── decisions/          ← decisiones documentadas por los agentes
│   ├── logs/               ← logs del orquestador
│   ├── state.toml          ← checkpoint para --resume
│   ├── daemon.pid          ← PID del proceso daemon
│   └── daemon.log          ← log del daemon
├── .pi/
│   └── skills/             ← skills de pi (PO, QA, Dev, Reviewer)
│       ├── product-owner/SKILL.md
│       ├── qa-engineer/SKILL.md
│       ├── developer/SKILL.md
│       └── reviewer/SKILL.md
└── src/                    ← tu código
```

---

## Configuración

Genera la estructura inicial con:

```bash
regista init                     # estructura completa (config + skills + carpetas)
regista init --light             # solo .regista/config.toml
regista init --with-example      # incluye historia y épica de ejemplo
```

### `.regista/config.toml` de referencia

```toml
[project]
stories_dir    = ".regista/stories"       # dónde están las historias
story_pattern  = "STORY-*.md"            # glob para encontrarlas
epics_dir      = ".regista/epics"        # opcional: para filtrar
decisions_dir  = ".regista/decisions"    # decisiones de los agentes
log_dir        = ".regista/logs"         # logs del orquestador

[agents]
product_owner = ".pi/skills/product-owner/SKILL.md"
qa_engineer   = ".pi/skills/qa-engineer/SKILL.md"
developer     = ".pi/skills/developer/SKILL.md"
reviewer      = ".pi/skills/reviewer/SKILL.md"

[limits]
max_iterations            = 0   # 0 = auto: nº historias × 6 (mín 10)
max_retries_per_step      = 5
max_reject_cycles         = 3
agent_timeout_seconds     = 1800
max_wall_time_seconds     = 28800
retry_delay_base_seconds  = 10
groom_max_iterations      = 5
inject_feedback_on_retry  = true

[hooks]
# Comandos opcionales de verificación post-fase.
# Si fallan, se hace rollback automático (si git.enabled = true).
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings"
post_reviewer = "cargo test"

[git]
enabled = true   # snapshots + rollback automáticos
```

Todos los campos son opcionales. Si no existe `.regista/config.toml`, se usan
los defaults mostrados arriba.

### `max_iterations = 0` — auto-escalado

Cuando se deja en 0, el orquestador calcula automáticamente el límite como:

```
máximo de iteraciones = max(10, número_de_historias × 6)
```

Para un proyecto de 21 historias, esto da 126 iteraciones, suficiente para
completar todo el backlog sin intervención. Si quieres un límite fijo,
pon el número que quieras (ej: `max_iterations = 50`).

---

## Formato de historias

Cada historia es un archivo `.md` dentro de `.regista/stories/`:

```markdown
# STORY-001: Título de la historia

## Status
**Draft**

## Epic
EPIC-001

## Descripción
Como [rol], quiero [acción] para que [beneficio].

## Criterios de aceptación
- [ ] CA1: Descripción del criterio
- [ ] CA2: Otro criterio testeable

## Dependencias
- Bloqueado por: STORY-000

## Activity Log
- YYYY-MM-DD | PO | Creada en Draft
```

### Estados válidos

| Estado | Significado |
|--------|-------------|
| `Draft` | Sin refinar, necesita al PO |
| `Ready` | Refinada, lista para QA |
| `Tests Ready` | Tests escritos, lista para Dev |
| `In Progress` | Dev está implementando o corrigiendo |
| `In Review` | En revisión técnica por el Reviewer |
| `Business Review` | En validación de negocio por el PO |
| `Done` | Completada ✅ |
| `Blocked` | Dependencias no resueltas ⛔ |
| `Failed` | Ciclos de rechazo agotados ❌ |

---

## Uso

### `regista help`

Muestra todos los comandos y flags disponibles:

```bash
regista help
```

### Generar el backlog (`groom`)

Descompone un documento de especificación en historias automáticamente:

```bash
regista groom product/spec.md

# Con límite de historias
regista groom product/spec.md --max-stories 8

# Regenerar desde cero
regista groom product/spec.md --replace
```

`groom` invoca al PO, escribe los `.md` y ejecuta un **bucle de validación**
de dependencias hasta que el grafo esté limpio.

### Validar el proyecto (`validate`)

Chequeo pre-vuelo completo:

```bash
regista validate

# Salida JSON para CI
regista validate --json
```

Verifica: configuración, existencia de skills, parseo de historias,
Activity Log, referencias a dependencias, ciclos, y estado de git.

### Pipeline completo

```bash
# Procesar todo el backlog
regista

# Una sola iteración (procesa una historia y sale)
regista --once

# Solo una historia concreta
regista --story STORY-007

# Solo historias de una épica
regista --epic EPIC-001

# Rango de épicas (inclusivo)
regista --epics "EPIC-001..EPIC-003"
```

### Dry-run — simular sin gastar

```bash
# Ver qué haría el orquestador sin invocar agentes
regista --dry-run

# Simular solo una iteración
regista --dry-run --once

# Simular con salida JSON
regista --dry-run --json
```

### Checkpoint y reanudación

```bash
# El pipeline guarda su estado en .regista/state.toml tras cada iteración
regista

# Si se interrumpe (crash, timeout, Ctrl+C), reanuda:
regista --resume

# Borrar el checkpoint manualmente
regista --clean-state
```

### Salida JSON para CI/CD

```bash
# Reporte estructurado a stdout, logs a stderr
regista --json

# Solo el JSON, sin logs de progreso
regista --json --quiet
```

Ejemplo de salida JSON:

```json
{
  "regista_version": "0.2.0",
  "project_dir": ".",
  "result": "completed",
  "exit_code": 0,
  "stopped_early": false,
  "stop_reason": null,
  "summary": {
    "total": 21,
    "done": 9,
    "failed": 0,
    "blocked": 6,
    "draft": 5,
    "iterations": 10,
    "elapsed_seconds": 3169
  },
  "stories": [
    {
      "id": "STORY-001",
      "status": "Done",
      "epic": "EPIC-001",
      "iterations": 2,
      "reject_cycles": 0
    }
  ]
}
```

Exit codes:

| Código | Significado |
|--------|-------------|
| `0` | Pipeline completado, todas las historias Done |
| `2` | Pipeline completado pero hay historias Failed |
| `3` | Parada temprana por límite (`max_iterations` o `max_wall_time`) |

### Modo daemon

```bash
# Lanzar en segundo plano
regista --detach

# Consultar si sigue corriendo
regista --status

# Ver el log en vivo (Ctrl+C para salir, el daemon sigue)
regista --follow

# Detener el daemon
regista --kill

# Log personalizado
regista --detach --log-file logs/mi-log.log
```

El daemon sobrevive a la desconexión SSH y su log por defecto está en
`.regista/daemon.log`.

### Archivo de configuración alternativo

```bash
regista --config mi-config.toml
regista validate --config mi-config.toml
```

---

## Máquina de estados

### Flujo feliz

```
Draft ──PO(groom)──→ Ready ──QA(tests)──→ Tests Ready ──Dev(implement)──→ In Review
                                                                                │
                                                                         Reviewer │
                                                                                ▼
                               Done ←──PO(validate)── Business Review
```

### Rechazos y correcciones

```
Ready ──QA──→ Draft                       (historia no testeable)
Tests Ready ──QA──→ Tests Ready            (Dev reporta tests rotos → QA corrige)
In Review ──Reviewer──→ In Progress        (rechazo técnico → Dev corrige)
Business Review ──PO──→ In Review          (rechazo leve)
Business Review ──PO──→ In Progress        (rechazo grave → Dev re-implementa)
```

### Transiciones automáticas (sin agente)

| # | De | A | Condición |
|---|---|---|---|
| 12 | Cualquiera | **Blocked** | Tiene dependencias no resueltas (`≠ Done`) |
| 13 | **Blocked** | **Ready** | Todas las dependencias pasan a `Done` |
| 14 | Cualquiera | **Failed** | Supera `max_reject_cycles` (3 por defecto) |

---

### Deadlock detection

Cuando el grafo no tiene historias accionables (todo está en Draft o Blocked),
el orquestador analiza las dependencias:

1. **Historias en Draft** → son candidatas a ser refinadas por el PO
2. **Historias bloqueadas por Drafts** → prioriza el Draft que más desbloquea
3. **Ciclos de dependencias** → el PO debe romper el ciclo

Se elige la historia que **más dependencias desbloquea**. En caso de empate,
gana el ID más bajo.

---

### Feedback rico en reintentos

Cuando un agente falla, `regista`:

1. Guarda stdout/stderr en `.regista/decisions/`
2. En el reintento, inyecta el error truncado (2000 bytes) en el prompt
3. Usa backoff exponencial entre reintentos (delay × 2)

Configurable con `inject_feedback_on_retry = false`.

---

## Referencia completa de CLI

```
regista [DIR]                        Pipeline completo
regista validate [DIR]               Validación pre-vuelo
regista init [DIR]                   Scaffolding de proyecto
regista groom <SPEC.md>              Generar historias desde spec
regista help                         Mostrar esta ayuda

Flags del pipeline:
  --config <FILE>        Archivo de configuración alternativo
  --story <ID>           Procesar solo una historia (STORY-001)
  --epic <ID>            Filtrar por épica (EPIC-001)
  --epics <RANGO>        Rango de épicas ("EPIC-001..EPIC-003")
  --once                 Una iteración y salir
  --dry-run              Simular sin invocar agentes (sin coste)
  --json                 Salida JSON a stdout para CI/CD
  --quiet                Suprimir logs, solo errores
  --resume               Reanudar desde el último checkpoint
  --clean-state          Borrar el checkpoint
  --log-file <FILE>      Archivo de log (default: stderr)

Flags del daemon:
  --detach               Lanzar en segundo plano
  --follow               Ver log en vivo del daemon
  --status               Consultar si el daemon sigue corriendo
  --kill                 Detener el daemon

Flags de groom:
  --max-stories <N>      Máximo de historias (0 = sin límite)
  --replace              Regenerar desde cero
  --config <FILE>        Archivo de configuración alternativo

Flags de init:
  --light                Solo config, sin skills
  --with-example         Incluir historia y épica de ejemplo

Flags de validate:
  --json                 Salida JSON estructurada
  --config <FILE>        Archivo de configuración alternativo
```

---

## Arquitectura interna

```
src/
├── main.rs                ← CLI (clap), subcomandos, JSON, exit codes
├── config.rs              ← Config, carga TOML, defaults
├── state.rs               ← Status, Actor, Transition (14 transiciones canónicas)
├── story.rs               ← Story, parseo .md, set_status() con backup atómico
├── dependency_graph.rs    ← Grafo de dependencias, DFS para ciclos
├── deadlock.rs            ← Detección de bloqueos + algoritmo de priorización
├── agent.rs               ← invoke_with_retry(), backoff exponencial, feedback
├── prompts.rs             ← 7 funciones de prompt (una por transición)
├── orchestrator.rs        ← Loop principal, dry-run, auto-escalado de iteraciones
├── checkpoint.rs          ← Save/load/remove de .regista/state.toml
├── validator.rs           ← Comando validate (pre-vuelo)
├── init.rs                ← Comando init (scaffolding)
├── groom.rs               ← Comando groom (backlog con bucle validate)
├── hooks.rs               ← Ejecución de hooks post-fase
├── git.rs                 ← Snapshots + rollback con git
└── daemon.rs              ← Modo daemon (detach/follow/status/kill)
```

---

## Tests

```bash
cargo test    # 104 tests, 0 fallos
cargo clippy  # 0 warnings
```

---

## Licencia

MIT © 2026 [dbareautopi](https://github.com/dbareautopi)
