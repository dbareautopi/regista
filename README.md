# regista 🎬

> AI agent director — orquestación multi-provider del ciclo completo de
> desarrollo: **PO → QA → Dev → Reviewer → Done.**
>
> Compatible con [`pi`](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent),
> [Claude Code](https://github.com/anthropics/claude-code),
> [Codex CLI](https://github.com/openai/codex), y
> [OpenCode](https://github.com/anomalyco/opencode).

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## ¿Qué hace?

`regista` toma un backlog de historias de usuario (archivos `.md`) y ejecuta
el pipeline completo de desarrollo de forma **autónoma y en background**,
disparando agentes de codificación según una máquina de estados formal:

```
Draft ──PO──→ Ready ──QA──→ Tests Ready ──Dev──→ In Review ──Reviewer──→ Business Review ──PO──→ Done
  ↑                                      ↑            ↑                      ↑                    ↑
  │                           QA corrige tests      │              Reviewer rechaza    PO rechaza/revalida
  └────────────────────────────────────────────────┴──────────────────────────────────────────────────┘
                              Con detección de deadlocks y desbloqueo automático
```

- **100% daemon**: toda ejecución corre en background. Usa `--logs` para ver el progreso.
- **Spec-first**: escribe una especificación en lenguaje natural, `regista auto` hace el resto.
- **Multi-provider**: elige entre `pi`, `claude`, `codex` u `opencode` — o mezcla por rol.
- **Deadlock detection**: si el grafo se estanca, prioriza la historia que más dependencias desbloquea.
- **Checkpoint/resume**: guarda progreso tras cada iteración. Si algo interrumpe → `--resume`.
- **Dry-run**: simula el pipeline completo sin gastar créditos de LLM.

## Filosofía

Regista **no sabe nada de tu proyecto**. No le importa si usas Rust, Python
o lo que sea. Solo necesita tres cosas:

1. **Dónde están tus historias** (archivos `.md`)
2. **Qué provider y qué instrucciones de rol** usar para PO, QA, Dev, Reviewer
3. **La máquina de estados fija** que gobierna las transiciones

Todo lo demás —código, tests, builds— lo manejan los agentes a través de sus
instrucciones de rol (skills, agents, commands).

---

## Quick start

```bash
# 1. Instalar
cargo install regista

# 2. Inicializar regista en tu proyecto
cd mi-proyecto
regista init --provider claude

# 3. Escribe tu especificación (specs/mi-app.md) y lánzalo todo
regista auto specs/mi-app.md --logs

# 4. Ver progreso en otra terminal
regista logs
```

Eso es todo. `regista auto` genera el backlog desde tu spec, ejecuta el
pipeline completo, y el daemon trabaja en background hasta que todas las
historias lleguen a `Done`.

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

## Comandos

```
regista <subcomando> [args]

Subcomandos de pipeline (daemon):
  plan      <spec>   Generar historias desde una especificación
  auto      <spec>   Generar historias + ejecutar pipeline completo
  run                Ejecutar pipeline sobre historias existentes

Subcomandos de gestión del daemon:
  logs      [dir]    Ver el log del daemon en vivo (Ctrl+C no lo detiene)
  status    [dir]    Consultar si el daemon está corriendo
  kill      [dir]    Detener el daemon

Subcomandos auxiliares:
  validate  [dir]    Validar configuración e historias
  board     [dir]    Dashboard Kanban: conteo por estado, bloqueadas, fallidas
  init      [dir]    Inicializar estructura del proyecto
```

### `regista plan` — generar backlog

Lee una especificación y genera historias de usuario + épicas en `.regista/`.
Ejecuta en modo daemon.

```bash
regista plan specs/mi-app.md              # merge: añade sin borrar
regista plan specs/mi-app.md --replace    # destructivo: borra y regenera
regista plan specs/mi-app.md --max-stories 10
regista plan specs/mi-app.md --logs       # daemon + tail del log
regista plan specs/mi-app.md --dry-run    # síncrono, sin daemon
regista plan specs/mi-app.md --provider claude
```

### `regista auto` — generar y ejecutar (full-auto)

Hace `plan` + `run` en un solo paso. El "fuego y olvido".

```bash
regista auto specs/mi-app.md              # planificar + ejecutar (daemon)
regista auto specs/mi-app.md --logs       # igual + ver progreso en vivo
regista auto specs/mi-app.md --replace    # desde cero
regista auto specs/mi-app.md --epic EPIC-001 --once
```

### `regista run` — ejecutar pipeline

Ejecuta el pipeline sobre historias ya existentes en `.regista/stories/`.

```bash
regista run                               # todo el backlog (daemon)
regista run --logs                        # daemon + tail
regista run --epic EPIC-001               # filtrar
regista run --story STORY-005 --once      # una historia, una iteración
regista run --dry-run                     # simulación síncrona
regista run --resume                      # reanudar tras interrupción
regista run --clean-state                 # borrar checkpoint antes
```

### `regista board` — dashboard de historias

Muestra un tablero Kanban con el conteo de historias por estado y lista
las que están bloqueadas o fallidas con detalle.

```bash
regista board                              # tablero completo del proyecto
regista board --json                       # salida JSON para CI/CD
regista board --epic EPIC-001              # filtrar por épica
regista board --epic EPIC-001 --json       # JSON filtrado

# Salida esperada:
# 📊 Story Board — regista
# ==========================
#
#   Draft                3
#   Ready                2
#   Tests Ready          1
#   In Progress          0
#   In Review            1
#   Business Review      0
#   Done                 5
#   Blocked              2
#   Failed               1
#   ──────────────────────
#   Total               15
#
# 🔴 Blocked (2):
#   STORY-008 — blocked by: STORY-005
#   STORY-012 — blocked by: STORY-003, STORY-007
#
# ❌ Failed (1):
#   STORY-015 — falta cobertura de tests para CA3
```

### `regista logs` / `status` / `kill` — gestión del daemon

```bash
regista logs                              # tail del log en vivo
regista logs /ruta/al/proyecto            # desde fuera del proyecto
regista status                            # ¿está corriendo?
regista kill                              # detener (SIGTERM → SIGKILL)
```

### `regista validate` — chequeo pre-vuelo

```bash
regista validate                          # validación completa
regista validate --json                   # salida JSON para CI/CD
```

### `regista init` — scaffolding

```bash
regista init                              # estructura completa (provider pi)
regista init --provider claude            # para Claude Code
regista init --provider codex             # para Codex
regista init --provider opencode          # para OpenCode
regista init --light                      # solo .regista/config.toml
regista init --with-example               # incluye historia de ejemplo
```

### Flags comunes a `plan`, `auto`, `run`

| Flag | Descripción |
|------|-------------|
| `--logs` | Tail del log tras spawnear el daemon |
| `--dry-run` | Simulación síncrona (sin agentes, sin coste) |
| `--config <PATH>` | Ruta al archivo `.regista/config.toml` |
| `--provider <NAME>` | Provider a usar (pi, claude, codex, opencode) |
| `--quiet` | Suprimir logs de progreso |

### Flags de pipeline (`auto`, `run`)

| Flag | Descripción |
|------|-------------|
| `--story <ID>` | Filtrar por historia |
| `--epic <ID>` | Filtrar por épica |
| `--epics <RANGO>` | Filtrar por rango (`EPIC-001..EPIC-003`) |
| `--once` | Una sola iteración |
| `--resume` | Reanudar desde checkpoint |
| `--clean-state` | Borrar checkpoint antes de arrancar |

### Flags de planificación (`plan`, `auto`)

| Flag | Descripción |
|------|-------------|
| `--replace` | Borrar historias existentes antes de generar |
| `--max-stories <N>` | Límite de historias (0 = sin límite, default) |

---

## Caso de uso: de spec a Done en un solo comando

### 1. Inicializa el proyecto

```bash
cd mi-app
regista init --provider claude --with-example
```

Esto crea `.regista/config.toml`, las instrucciones de rol para Claude Code,
y una historia de ejemplo para que veas el formato.

### 2. Escribe tu especificación

Crea un archivo como `specs/mi-app.md`. Este es el **único input** que necesitas
proporcionar. Aquí tienes un formato recomendado:

```markdown
# Mi App — Especificación de producto

## Descripción general
Plataforma de blogging con soporte multi-idioma y sistema de comentarios.

## Usuarios objetivo
- Autores: crean y gestionan contenido
- Lectores: consumen artículos y comentan
- Moderadores: revisan comentarios

## Funcionalidades

### 1. Gestión de artículos
- Crear, editar, borrar artículos (borrador/publicado)
- Soporte para Markdown con vista previa
- Programar fecha de publicación

### 2. Traducción de contenido
- Cada artículo puede tener traducciones a 3 idiomas
- Los autores asignan traductores por idioma
- Las traducciones tienen su propio flujo de revisión

### 3. Sistema de comentarios
- Lectores comentan al final de cada artículo
- Moderación opcional (comentarios quedan en "pendiente")
- Respuestas anidadas (hilos)

### 4. Dashboard de analytics
- Vistas por artículo (diarias, semanales, mensuales)
- Tiempo medio de lectura
- Gráficos exportables (PNG, CSV)

### 5. Notificaciones por email
- Aviso de nuevos comentarios al autor
- Resumen semanal de actividad
- Notificaciones de traducción completada

## Requisitos técnicos
- API RESTful
- Base de datos PostgreSQL
- Autenticación JWT
- Tests con ≥ 80% de cobertura

## Restricciones
- Los artículos publicados no se pueden borrar, solo archivar
- Un usuario solo puede tener 3 sesiones simultáneas
- Límite de 5 comentarios por minuto por IP
```

### 3. Lánzalo todo

```bash
regista auto specs/mi-app.md --logs
```

**¿Qué pasa?**

1. El Product Owner lee `specs/mi-app.md`
2. La descompone en ~15-25 historias atómicas con criterios de aceptación
3. Las agrupa en 5 épicas (una por cada funcionalidad)
4. Detecta dependencias entre historias (`Bloqueado por: STORY-XXX`)
5. Valida el grafo de dependencias en bucle hasta que esté limpio
6. Escribe todo en `.regista/stories/STORY-NNN.md` y `.regista/epics/EPIC-NNN.md`
7. El orquestador arranca el pipeline:
   - **PO** refina cada Draft → Ready
   - **QA** escribe tests → Tests Ready
   - **Dev** implementa → In Review
   - **Reviewer** revisa → Business Review o rechaza
   - **PO** valida → Done (o rechaza para otra iteración)
8. El daemon sigue hasta que todas las historias están en `Done` o `Failed`

Mientras tanto, `--logs` te muestra el progreso en vivo. Puedes hacer Ctrl+C
en cualquier momento: el daemon **sigue corriendo**.

```bash
# Si cierraste el --logs, puedes volver a verlo
regista logs

# Consultar cómo va
regista status
# → ✅ Daemon corriendo (PID: 12345, log: .regista/daemon.log)

# Si quieres pararlo
regista kill
```

### 4. Itera si hace falta

Si alguna historia queda en `Failed` (superó `max_reject_cycles`), revisa qué
falló:

```bash
cat .regista/daemon.log | grep -i "error\|failed\|rechaz"
```

Ajusta la spec, y vuelve a lanzar:

```bash
regista auto specs/mi-app.md --logs
```

O si solo cambiaste instrucciones de rol pero las historias están bien, lanza
solo el pipeline:

```bash
regista run --resume --logs
```

---

## Flujo de trabajo completo

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Flujo spec-first                            │
│                                                                      │
│  1. regista init                   Estructura inicial                │
│  2. Escribe specs/mi-app.md        Tu especificación de producto     │
│  3. regista auto specs/mi-app.md   Genera backlog + ejecuta pipeline │
│     --logs                         (daemon, ves progreso en vivo)    │
│  4. regista logs                   Re-conectar al progreso           │
│  5. Itera sobre Failed ajustando   Mejora la spec, repite            │
│     la spec o las instrucciones                                      │
└──────────────────────────────────────────────────────────────────────┘

     🌍 Tu input                  │    📁 .regista/ (gestionado por regista)
     ────────────────────────────┼─────────────────────────────────────────
     specs/mi-app.md             │    stories/STORY-*.md      ← backlog
     (especificación)            │    epics/EPIC-*.md         ← épicas
                                 │    decisions/              ← logs de agentes
                                 │    state.toml              ← checkpoint
                                 │    daemon.log              ← log del daemon
                                 │    daemon.pid              ← PID
```

---

## Formato de especificación recomendado

Tu spec es el **contrato** entre tú y regista. Sé concreto y estructurado.
Un buen formato incluye:

```markdown
# Título del producto — Especificación

## Descripción general
[2-3 frases explicando qué hace el producto y para quién]

## Usuarios objetivo
- [Rol 1]: [lo que hace]
- [Rol 2]: [lo que hace]

## Funcionalidades

### N. [Nombre de la funcionalidad]
- [Funcionalidad concreta 1]
- [Funcionalidad concreta 2]

## Requisitos técnicos (opcional)
- [Stack, base de datos, autenticación...]

## Restricciones (opcional)
- [Reglas de negocio importantes]
```

**Consejos:**

- **Sé concreto**: "Dashboard con gráficos de vistas diarias" > "Analytics".
- **Describe el qué, no el cómo**: el cómo lo deciden los agentes.
- **No hace falta descomponer en historias**: el PO lo hace por ti.
- **Pon restricciones importantes**: "no se puede borrar un artículo publicado"
  evita que el Dev tome decisiones equivocadas.

---

## Estructura del proyecto

```
mi-proyecto/
├── specs/                           ← tus especificaciones (input)
│   └── mi-app.md
│
├── .regista/                        ← gestionado por regista
│   ├── config.toml                  ← configuración del pipeline
│   ├── stories/                     ← historias de usuario (*.md)
│   │   ├── STORY-001.md
│   │   └── STORY-002.md
│   ├── epics/                       ← épicas
│   ├── decisions/                   ← decisiones documentadas por agentes
│   ├── state.toml                   ← checkpoint para --resume
│   ├── daemon.pid                   ← PID del proceso daemon
│   └── daemon.log                   ← log del daemon
│
├── .pi/skills/                      ← instrucciones si provider=pi
│   ├── product-owner/SKILL.md
│   ├── qa-engineer/SKILL.md
│   ├── developer/SKILL.md
│   └── reviewer/SKILL.md
│
├── .claude/agents/                  ← instrucciones si provider=claude
│   ├── product_owner.md
│   ├── qa_engineer.md
│   ├── developer.md
│   └── reviewer.md
│
├── .agents/skills/                  ← instrucciones si provider=codex
│   ├── product-owner/SKILL.md
│   └── ...
│
├── .opencode/agents/                ← instrucciones si provider=opencode
│   ├── product_owner.md
│   └── ...
│
└── src/                             ← tu código
```

---

## Configuración

### `.regista/config.toml` de referencia

```toml
[project]
stories_dir    = ".regista/stories"
story_pattern  = "STORY-*.md"
epics_dir      = ".regista/epics"
decisions_dir  = ".regista/decisions"
log_dir        = ".regista/logs"

[agents]
provider = "pi"                          # provider global

# Opcional: sobreescribir provider y/o skill por rol
[agents.product_owner]
# provider = "claude"                    # este rol usa otro provider
# skill = ".claude/agents/po-custom.md"  # instrucciones explícitas

[limits]
max_iterations            = 0   # 0 = auto: nº historias × 6 (mín 10)
max_retries_per_step      = 5
max_reject_cycles         = 3
agent_timeout_seconds     = 1800
max_wall_time_seconds     = 28800
retry_delay_base_seconds  = 10
plan_max_iterations      = 5
inject_feedback_on_retry  = true

[hooks]
# Comandos de verificación post-fase. Si fallan → rollback (con git.enabled)
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings"
post_reviewer = "cargo test"

[git]
enabled = true

[stack]
# Comandos del stack tecnológico. Opcionales: si no se definen,
# los agentes usan instrucciones genéricas y su skill interpreta el stack.
build_command = "npm run build"
test_command  = "npm test"
lint_command  = "eslint ."
fmt_command   = "prettier --check ."
src_dir       = "src/"
```

Todos los campos son opcionales. Si no se define `[stack]`, los prompts
usan instrucciones genéricas ("compila/construye el proyecto") y el skill del
agente interpreta el stack automáticamente.

### Providers soportados

| Provider | Binario | Directorio de instrucciones |
|----------|---------|----------------------------|
| `pi` | `pi` | `.pi/skills/<rol>/SKILL.md` |
| `claude` | `claude` | `.claude/agents/<rol>.md` |
| `codex` | `codex` | `.agents/skills/<rol>/SKILL.md` |
| `opencode` | `opencode` | `.opencode/agents/<rol>.md` |

Puedes sobreescribir el provider desde la CLI:

```bash
regista run --provider claude
regista auto specs/spec.md --provider codex
```

### `max_iterations = 0` — auto-escalado

Cuando se deja en 0, el orquestador calcula:

```
máx iteraciones = max(10, historias × 6)
```

Para 21 historias → 126 iteraciones. Suficiente para todo el backlog.

---

## Formato de historias

Formato que el PO genera en `.regista/stories/STORY-NNN.md`:

```markdown
# STORY-001: Título descriptivo

## Status
**Draft**

## Epic
EPIC-001

## Descripción
Como [rol], quiero [acción] para que [beneficio].

## Criterios de aceptación
- [ ] CA1: criterio específico y verificable
- [ ] CA2: ...

## Dependencias
- Bloqueado por: STORY-000

## Activity Log
- YYYY-MM-DD | PO | Creada en Draft
```

### Estados

| Estado | Significado |
|--------|-------------|
| `Draft` | Sin refinar |
| `Ready` | Refinada, lista para QA |
| `Tests Ready` | Tests escritos, lista para Dev |
| `In Progress` | Dev implementando |
| `In Review` | Reviewer evaluando |
| `Business Review` | PO validando |
| `Done` | Completada ✅ |
| `Blocked` | Dependencias sin resolver |
| `Failed` | Ciclos de rechazo agotados |

---

## Máquina de estados

### Flujo feliz

```
Draft ──PO──→ Ready ──QA──→ Tests Ready ──Dev──→ In Review ──Reviewer──→ Business Review ──PO──→ Done
```

### Rechazos y correcciones

```
Ready           ──QA──→ Draft              (no testeable)
Tests Ready     ──QA──→ Tests Ready        (Dev reporta tests rotos)
In Review       ──Reviewer──→ In Progress  (rechazo técnico)
Business Review ──PO──→ In Review          (rechazo leve)
Business Review ──PO──→ In Progress        (rechazo grave)
```

### Transiciones automáticas

| De | A | Disparador |
|----|---|------------|
| Cualquiera | `Blocked` | Dependencias ≠ Done |
| `Blocked` | `Ready` | Dependencias → Done |
| Cualquiera | `Failed` | Supera `max_reject_cycles` |

### Deadlock detection

Cuando no hay historias accionables, el orquestador:

1. Identifica historias en Draft → las refina el PO
2. Prioriza la que desbloquea más dependencias
3. Si hay ciclos, el PO debe romperlos

---

## Arquitectura interna

```
src/
├── main.rs                ← CLI (clap subcommands), handlers, dispatch
├── config.rs              ← Config, AgentsConfig, carga TOML
├── state.rs               ← Status, Actor, Transition (14 canónicas)
├── story.rs               ← Story, parseo .md, set_status() atómico
├── dependency_graph.rs    ← Grafo, DFS, ciclos
├── deadlock.rs            ← Detección y priorización de bloqueos
├── providers.rs           ← trait AgentProvider + 4 implementaciones
├── agent.rs               ← invoke_with_retry(), backoff, feedback
├── prompts.rs             ← 7 funciones de prompt por transición
├── orchestrator.rs        ← Loop principal, dry-run, auto-escalado
├── checkpoint.rs          ← Save/load/remove state.toml
├── validator.rs           ← validate: chequeo pre-vuelo multi-provider
├── init.rs                ← init: scaffolding multi-provider
├── plan.rs               ← plan: generación de backlog + bucle validate
├── board.rs              ← board: dashboard Kanban de historias
├── hooks.rs               ← Ejecución de hooks post-fase
├── git.rs                 ← Snapshots + rollback
└── daemon.rs              ← Modo daemon (detach/logs/status/kill)
```

---

## Tests

```bash
cargo test    # 143 tests, 0 fallos
cargo clippy  # 0 warnings
```

---

## Licencia

MIT © 2026 [dbareautopi](https://github.com/dbareautopi)
