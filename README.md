# regista 🎬

> **El director del ecosistema mezzala-regista.**  
> Lee la partitura (`spartito`), coordina a los músicos (agentes).  
> Pipeline completo de desarrollo: **PO → QA → Dev → Reviewer → Done.**

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
- **Multi-provider**: elige entre [`pi`](https://github.com/mariozechner/pi-coding-agent), [Claude Code](https://github.com/anthropics/claude-code), [Codex CLI](https://github.com/openai/codex), u [OpenCode](https://github.com/anomalyco/opencode) — o mezcla por rol.
- **Workflow configurable**: define tus propios estados, transiciones y bifurcaciones en `.regista/config.toml` (vía `spartito`, el contrato compartido con [`mezzala`](https://github.com/dbareautopi/mezzala)).
- **Deadlock detection**: si el grafo se estanca, prioriza la historia que más dependencias desbloquea.
- **Checkpoint/resume**: guarda progreso tras cada iteración. Si algo interrumpe → `--resume`.
- **Dry-run**: simula el pipeline completo sin gastar créditos de LLM.

## Ecosistema

Regista es parte de un ecosistema de 3 piezas:

| Pieza | Rol | Descripción |
|-------|-----|-------------|
| **[spartito](https://github.com/dbareautopi/mezzala/tree/main/crates/spartito)** | 📜 Partitura | Contrato compartido: estados, workflow, formato de historia, DoD/DoR |
| **regista** | 🎬 Director | Orquestador: lee la partitura, decide quién actúa y cuándo |
| **[mezzala](https://github.com/dbareautopi/mezzala)** | 🎻 Músico | Agent harness: ejecuta siguiendo la partitura (TUI, WASM, multi-provider) |

## Filosofía

Regista **no sabe nada de tu proyecto**. No le importa si usas Rust, Python
o lo que sea. Solo necesita tres cosas:

1. **Dónde están tus historias** (archivos `.md`)
2. **Qué provider y qué instrucciones de rol** usar para PO, QA, Dev, Reviewer
3. **La partitura** (`spartito`) que define estados, transiciones y el formato
   del contrato. Es el source of truth compartido con `mezzala`.

Todo lo demás —código, tests, builds— lo manejan los agentes a través de sus
instrucciones de rol (skills, agents, commands).

---

## 🚀 Flujo de trabajo habitual

El ciclo completo de regista tiene **4 pasos**:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Flujo spec-first                                 │
│                                                                         │
│  1. regista init                   Estructura inicial del proyecto      │
│  2. Escribe specs/mi-app.md        Tu especificación de producto        │
│  3. regista auto specs/mi-app.md   Genera backlog + ejecuta pipeline    │
│     --logs                         (daemon, ves progreso en vivo)       │
│  4. regista logs                   Re-conectar al progreso              │
│                                                                         │
│  Si alguna historia queda en Failed → revisa .regista/daemon.log,       │
│  ajusta la spec o las instrucciones de rol, y repite desde el paso 3.   │
└─────────────────────────────────────────────────────────────────────────┘

    🌍 Lo que tú creas           │   📁 .regista/ (gestionado por regista)
    ────────────────────────────┼─────────────────────────────────────────
    specs/mi-app.md             │   stories/STORY-*.md      ← backlog
    (tu especificación)         │   epics/EPIC-*.md         ← épicas
                                │   decisions/              ← logs de agentes
                                │   state.toml              ← checkpoint
                                │   daemon.log              ← log del daemon
```

---

## 🎯 Ejemplo completo: de spec a Done

Vamos a construir una **app de notas** desde cero. Solo necesitas 3 comandos.

### Paso 1 — Inicializar el proyecto

```bash
mkdir notas-app && cd notas-app
regista init --provider claude --with-example
```

Esto crea:
```
notas-app/
├── .regista/
│   ├── config.toml              ← provider = "claude", auto-escalado, hooks
│   ├── stories/STORY-001.md     ← historia de ejemplo (puedes borrarla)
│   └── epics/EPIC-001.md        ← épica de ejemplo
├── .claude/agents/              ← instrucciones de rol para Claude Code
│   ├── product_owner.md
│   ├── qa_engineer.md
│   ├── developer.md
│   └── reviewer.md
```

### Paso 2 — Escribir tu especificación

Crea `specs/notas-app.md`. Este es el **único input** que necesitas:

```markdown
# Notas App — Especificación de producto

## Descripción general
Aplicación de notas con organización por etiquetas y búsqueda full-text.
Interfaz de línea de comandos (CLI).

## Usuarios objetivo
- Usuarios técnicos que quieren tomar notas rápidas desde la terminal

## Funcionalidades

### 1. CRUD de notas
- Crear nota con título, contenido, y etiquetas opcionales
- Listar todas las notas (tabla con id, título, fecha)
- Ver una nota completa por ID
- Editar título, contenido o etiquetas de una nota existente
- Eliminar nota por ID (con confirmación)
- Las notas se guardan en una base de datos SQLite local

### 2. Etiquetas
- Asignar una o varias etiquetas a cada nota
- Filtrar notas por etiqueta
- Listar todas las etiquetas existentes

### 3. Búsqueda
- Buscar notas por palabra clave en título y contenido
- Resultados ordenados por relevancia (match exacto primero)
- Soporte para búsqueda combinada: etiqueta + keyword

### 4. Exportación
- Exportar una nota a Markdown (.md)
- Exportar todas las notas de una etiqueta a un solo archivo

## Requisitos técnicos
- Lenguaje: Rust
- CLI: clap 4
- Base de datos: SQLite (rusqlite)
- Búsqueda: FTS5 de SQLite
- Tests unitarios y de integración

## Restricciones
- No se puede eliminar una nota si es la única que tiene cierta etiqueta
  (prevenir etiquetas huérfanas)
- Los títulos no pueden superar 200 caracteres
```

### Paso 3 — Lanzar regista en modo auto

```bash
regista auto specs/notas-app.md --logs
```

**¿Qué pasa ahora?**

1. 🔍 El **Product Owner** lee tu spec y la descompone en ~15-20 historias atómicas
2. 📦 Las agrupa en épicas (CRUD, Etiquetas, Búsqueda, Exportación)
3. 🔗 Detecta dependencias (ej: "buscar por etiqueta" depende de "crear notas con etiquetas")
4. ✅ Valida el grafo de dependencias en bucle hasta que esté limpio

5. 🏭 El **orquestador** arranca el pipeline:
   - **PO** refina cada Draft → Ready
   - **QA** escribe tests → Tests Ready
   - **Dev** implementa → In Review
   - **Reviewer** revisa → Business Review o rechaza
   - **PO** valida → Done (o rechaza para otra iteración)

6. 🔁 El daemon sigue hasta que **todas las historias** estén en `Done` o `Failed`

```bash
# Ver el progreso en vivo:
regista logs

# El daemon trabaja en background. Puedes cerrar el terminal.
# Para ver cómo va más tarde:
regista status
# → ✅ Daemon corriendo (PID: 12345, log: .regista/daemon.log)

# Cuando termine, revisa el dashboard:
regista board
# → 📊 Story Board — regista
#   Draft         0
#   Ready         0
#   Tests Ready   0
#   Done          17 ✅
#   Failed        0
```

### Paso 4 — Iterar si es necesario

Si alguna historia queda en `Failed`, revisa qué falló:

```bash
cat .regista/daemon.log | grep -i "failed\|rechaz"
regista board --json | jq '.failed'
```

Ajusta tu spec o las instrucciones de rol (`.claude/agents/developer.md`, etc.),
y relanza solo el pipeline (sin regenerar historias):

```bash
regista run --resume --logs
```

O empieza de cero con una spec mejorada:

```bash
regista auto specs/notas-app.md --replace --logs
```

---

## 📋 Comandos

```
regista <subcomando> [args]
```

| Comando | Descripción |
|---------|-------------|
| `plan <spec>` | Generar historias desde una especificación (daemon) |
| `auto <spec>` | Generar historias + ejecutar pipeline completo (daemon) |
| `run [dir]` | Ejecutar pipeline sobre historias existentes (daemon) |
| `logs [dir]` | Ver el log del daemon en vivo (Ctrl+C no lo detiene) |
| `status [dir]` | Consultar si el daemon está corriendo |
| `kill [dir]` | Detener el daemon |
| `board [dir]` | Dashboard Kanban: conteo por estado, bloqueadas, fallidas |
| `validate [dir]` | Validar configuración e historias (sin ejecutar agentes) |
| `init [dir]` | Inicializar estructura del proyecto |
| `update` | Comprobar e instalar nueva versión desde crates.io |

### `regista auto` — generar y ejecutar (full-auto)

El comando principal. "Fuego y olvida":

```bash
regista auto specs/mi-app.md              # planificar + ejecutar (daemon)
regista auto specs/mi-app.md --logs       # igual + ver progreso en vivo
regista auto specs/mi-app.md --replace    # desde cero (borra historias anteriores)
regista auto specs/mi-app.md --epic EPIC-001 --once  # una épica, una iteración
regista auto specs/mi-app.md --dry-run    # simulación síncrona (sin agentes)
regista auto specs/mi-app.md --provider claude
```

### `regista plan` — solo generar backlog

Genera las historias y épicas pero **no ejecuta el pipeline**:

```bash
regista plan specs/mi-app.md              # merge: añade sin borrar
regista plan specs/mi-app.md --replace    # destructivo: borra y regenera
regista plan specs/mi-app.md --max-stories 10
regista plan specs/mi-app.md --logs       # daemon + tail del log
```

### `regista run` — solo pipeline

Ejecuta el pipeline sobre historias **ya existentes**:

```bash
regista run                               # todo el backlog (daemon)
regista run --logs                        # daemon + ver progreso
regista run --epic EPIC-001               # filtrar por épica
regista run --story STORY-005 --once      # una historia, una iteración
regista run --dry-run                     # simulación síncrona
regista run --resume                      # reanudar tras interrupción
regista run --clean-state                 # borrar checkpoint antes
```

### `regista board` — dashboard

```bash
regista board                              # tablero completo
regista board --json                       # salida JSON para CI/CD
regista board --epic EPIC-001              # filtrar por épica
regista board --epic EPIC-001 --json       # JSON filtrado

# Salida:
# 📊 Story Board — regista
#   Draft                3
#   Ready                2
#   Tests Ready          1
#   Done                 5
#   Blocked              2
#   Failed               1
#   Total               14
#
# 🔴 Blocked (2):
#   STORY-008 — blocked by: STORY-005
#   STORY-012 — blocked by: STORY-003, STORY-007
#
# ❌ Failed (1):
#   STORY-015 — falta cobertura de tests para CA3
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

---

## 📦 Instalación

```bash
# Desde crates.io
cargo install regista

# Desde el repositorio
git clone https://github.com/dbareautopi/regista
cd regista
cargo build --release
```

El binario queda en `~/.cargo/bin/regista`.

---

## 🔧 Configuración

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
# provider = "claude"
# skill = ".claude/agents/po-custom.md"

[limits]
max_iterations            = 0   # 0 = auto: nº historias × 6 (mín 10)
max_retries_per_step      = 5
max_reject_cycles         = 8
agent_timeout_seconds     = 1800
max_wall_time_seconds     = 28800
retry_delay_base_seconds  = 10
plan_max_iterations      = 5
inject_feedback_on_retry  = true

[hooks]
# post_qa       = "cargo check --tests"
# post_dev      = "cargo build && cargo test"
# post_reviewer = "cargo test && cargo clippy -- -D warnings"

[git]
enabled = true

[stack]
# Comandos del stack. Opcionales: si no se definen, los agentes
# usan instrucciones genéricas y su skill interpreta el stack.
build_command = "npm run build"
test_command  = "npm test"
lint_command  = "eslint ."
fmt_command   = "prettier --check ."
src_dir       = "src/"
```

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

---

## 📁 Estructura del proyecto

```
mi-proyecto/
├── specs/                           ← tus especificaciones (input)
│   └── mi-app.md
│
├── .regista/                        ← gestionado por regista
│   ├── config.toml                  ← configuración del pipeline
│   ├── stories/                     ← historias de usuario (*.md)
│   ├── epics/                       ← épicas
│   ├── decisions/                   ← decisiones documentadas por agentes
│   ├── state.toml                   ← checkpoint para --resume
│   ├── health.json                  ← métricas del pipeline
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
│   └── ...
│
└── src/                             ← tu código
```

---

## 📝 Formato de especificación

Tu spec es el **contrato** entre tú y regista. Sé concreto y estructurado:

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
- **Pon restricciones importantes**: "no se puede borrar un artículo publicado"
  evita que el Dev tome decisiones equivocadas.

---

## 🧪 Tests

```bash
cargo test    # 357 tests, 0 fallos
cargo clippy  # 0 warnings
```

---

## 📐 Arquitectura interna

Arquitectura en **4 capas** con dependencias unidireccionales:

```
src/
├── cli/        ← 🟢 CLI (args + handlers) → importa cualquier capa
├── app/        ← 🟡 Casos de uso → importa domain + infra + config + spartito
├── domain/     ← 🔴 Lógica pura → importa spartito (crate externo), no otras capas
├── infra/      ← 🔵 I/O, procesos → solo importa config
└── config.rs   ← ⚪ Configuración → no importa nada del crate
```

El contrato de workflow (`Status`, `Actor`, `Workflow` trait, `story_format`)
está externalizado en el crate **[`spartito`](../mezzala/docs/spec-spartito.md)**.

Verificada automáticamente por `tests/architecture.rs` (11 tests, reglas R1-R5).
Para más detalle, consulta [`AGENTS.md`](AGENTS.md).

---

## Licencia

MIT © 2026 [dbareautopi](https://github.com/dbareautopi)
