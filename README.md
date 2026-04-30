# regista 🎬

AI agent director for [`pi`](https://github.com/mariozechner/pi-coding-agent).

Automates the full development pipeline with agents:
**PO → QA → Dev → Reviewer → Done**, governed by a formal
state machine with deadlock detection, checkpoint/resume,
and CI/CD-ready JSON output.

## Filosofía

Regista **no sabe nada de tu proyecto**. No importa si usas Rust,
Python, o lo que sea. Solo necesita tres cosas:

1. **Dónde están tus historias** de usuario (archivos `.md`)
2. **Qué skills de `pi`** actúan como PO, QA, Dev, Reviewer
3. **La máquina de estados fija** que gobierna las transiciones

## Instalación

```bash
# Instalación oficial desde crates.io
cargo install regista

# O desde el repositorio
git clone https://github.com/dbareautopi/regista
cd regista
cargo build --release
```

El binario se instalará en `~/.cargo/bin/regista` (añadido al PATH automáticamente por Rust).

## Configuración

Crea un archivo `.regista.toml` en la raíz de tu proyecto, o ejecuta:

```bash
regista init                     # genera estructura completa
regista init --light             # solo .regista.toml
regista init --with-example      # incluye historia de ejemplo
```

Configuración de referencia:

```toml
[project]
stories_dir    = "product/stories"     # dónde están las historias
story_pattern  = "STORY-*.md"          # glob para encontrarlas
epics_dir      = "product/epics"       # opcional: para filtrar
decisions_dir  = "product/decisions"   # decisiones de los agentes
log_dir        = "product/logs"        # logs del orquestador

[agents]
product_owner = ".pi/skills/product-owner/SKILL.md"
qa_engineer   = ".pi/skills/qa-engineer/SKILL.md"
developer     = ".pi/skills/developer/SKILL.md"
reviewer      = ".pi/skills/reviewer/SKILL.md"

[limits]
max_iterations            = 10
max_retries_per_step      = 5
max_reject_cycles         = 3
agent_timeout_seconds     = 1800
max_wall_time_seconds     = 28800
retry_delay_base_seconds  = 10
groom_max_iterations      = 5     # bucle groom→validate→corregir
inject_feedback_on_retry  = true  # inyectar stderr en reintentos

[hooks]
# Comandos opcionales de verificación post-fase.
# Si fallan, se hace rollback automático.
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings"
post_reviewer = "cargo test"

[git]
enabled = true   # snapshots + rollback automáticos
```

Todos los campos tienen valores por defecto razonables. Un proyecto mínimo
solo necesita definir `[agents]`.

## Formato de historias

Tus historias deben seguir este formato `.md`:

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
- [ ] CA2: ...

## Dependencias       ← opcional
- Bloqueado por: STORY-000

## Activity Log       ← obligatorio
- 2026-04-30 | PO | Creada en Draft
```

### Estados válidos

`Draft` · `Ready` · `Tests Ready` · `In Progress` · `In Review` · `Business Review` · `Done` · `Blocked` · `Failed`

## Uso

### Generar el backlog automáticamente

```bash
# Desde un documento de requisitos
regista groom product/spec.md

# Con límite de historias
regista groom product/spec.md --max-stories 8

# Regenerar desde cero
regista groom product/spec.md --replace
```

`groom` invoca al PO para descomponer la spec en historias, escribe los `.md`,
y ejecuta un **bucle de validación** de dependencias hasta que el grafo esté limpio.

### Validar el proyecto

```bash
# Chequeo pre-vuelo (config, historias, skills, dependencias, git)
regista validate

# Salida JSON para CI
regista validate --json
```

### Pipeline completo

```bash
# Procesar todo el backlog
regista /ruta/a/tu/proyecto

# Una sola iteración (procesa una historia y sale)
regista /ruta/a/tu/proyecto --once
```

### Simular antes de ejecutar

```bash
# Ver qué haría el orquestador sin invocar agentes
regista --dry-run

# Simular solo una iteración
regista --dry-run --once

# Simular con salida JSON
regista --dry-run --json
```

### Salida JSON para CI/CD

```bash
# Reporte estructurado a stdout, logs a stderr
regista --json

# Solo el JSON, sin logs de progreso
regista --json --quiet

# Validar en CI y capturar reporte
regista validate --json && regista --json --once > report.json
```

Exit codes: 0 = éxito, 1 = error de configuración, 2 = hay historias `Failed`.

### Checkpoint y reanudación

```bash
# El pipeline guarda su estado en .regista.state.toml tras cada iteración
regista

# Si se interrumpe, reanuda desde donde estaba
regista --resume

# Borrar el checkpoint manualmente
regista --clean-state
```

### Filtros de historias

```bash
# Solo una historia concreta
regista /ruta/a/tu/proyecto --story STORY-007

# Solo historias de una épica
regista /ruta/a/tu/proyecto --epic EPIC-001

# Rango de épicas (inclusivo)
regista /ruta/a/tu/proyecto --epics "EPIC-001..EPIC-003"
```

### Archivo de configuración alternativo

```bash
regista /ruta/a/tu/proyecto --config mi-config.toml
```

### Archivo de log personalizado

```bash
regista /ruta/a/tu/proyecto --log-file logs/debug.log
```

### Modo daemon

```bash
# Lanzar en segundo plano
regista /ruta/a/tu/proyecto --detach

# Consultar si sigue corriendo
regista /ruta/a/tu/proyecto --status

# Ver el log en vivo (Ctrl+C para salir)
regista /ruta/a/tu/proyecto --follow

# Detener el daemon
regista /ruta/a/tu/proyecto --kill
```

## Máquina de estados

### Flujo feliz

```
Draft ──PO(groom)──→ Ready ──QA──→ Tests Ready ──Dev──→ In Review
                                                           │
                                                    Reviewer │
                                                           ▼
                       Done ←──PO(validate)── Business Review
```

### Rechazos

```
In Review ──Reviewer──→ In Progress ──Dev(fix)──→ In Review
Business Review ──PO──→ In Review  (rechazo leve)
                 ──PO──→ In Progress (rechazo grave)
```

### Transiciones automáticas (sin agente)

| Transición | Condición |
|---|---|
| Cualquier estado → **Blocked** | Tiene dependencias no resueltas (`≠ Done`) |
| **Blocked** → **Ready** | Todas las dependencias pasan a `Done` |
| Cualquier estado → **Failed** | Se superan `max_reject_cycles` (3 por defecto) |

### Feedback rico en reintentos

Cuando un agente falla, regista:
1. Guarda stdout/stderr en `product/decisions/`
2. En el reintento, inyecta el error en el prompt: «Tu intento anterior falló. Corrígelo.»
3. Esto aumenta la probabilidad de éxito en reintentos.

Configurable: `inject_feedback_on_retry = false` para desactivarlo.

## Referencia completa de CLI

```
regista [PROJECT_DIR] [FLAGS]            # pipeline normal
regista validate [PROJECT_DIR] [FLAGS]   # validación pre-vuelo
regista init [PROJECT_DIR] [FLAGS]       # scaffolding de proyecto
regista groom <SPEC.md> [FLAGS]          # generación de historias

FLAGS (pipeline):
  --config <FILE>         Archivo de configuración alternativo
  --epics <RANGE>         Rango de épicas ("EPIC-001..EPIC-003")
  --epic <ID>             Una sola épica
  --story <ID>            Una sola historia
  --once                  Una iteración y salir
  --json                  Salida JSON a stdout
  --quiet                 Suprimir logs de progreso
  --dry-run               Simular sin invocar agentes
  --resume                Reanudar desde último checkpoint
  --clean-state           Borrar checkpoint
  --detach                Lanzar en segundo plano (daemon)
  --follow                Ver log en vivo del daemon
  --status                Consultar si el daemon sigue vivo
  --kill                  Detener el daemon
  --log-file <FILE>       Archivo de log (por defecto: stderr)

FLAGS (groom):
  --max-stories <N>       Máximo de historias (0 = sin límite)
  --replace               Regenerar desde cero (default: merge)
  --config <FILE>         Archivo de configuración alternativo

FLAGS (init):
  --light                 Solo .regista.toml, sin skills
  --with-example          Incluir historia y épica de ejemplo

FLAGS (validate):
  --json                  Salida JSON estructurada
  --config <FILE>         Archivo de configuración alternativo
```

## Estructura del proyecto

```
regista/
├── src/
│   ├── main.rs                ← CLI, subcomandos, JSON output, exit codes
│   ├── config.rs              ← Config, carga TOML, defaults
│   ├── state.rs               ← Status, Actor, Transition
│   ├── story.rs               ← Story, parseo .md, set_status()
│   ├── dependency_graph.rs    ← Grafo, ciclos DFS, conteo inverso
│   ├── deadlock.rs            ← Detección de bloqueos + priorización
│   ├── agent.rs               ← pi con timeout, retry, feedback rico
│   ├── prompts.rs             ← Prompts para PO/QA/Dev/Reviewer
│   ├── orchestrator.rs        ← Loop principal, dry-run, checkpoint
│   ├── checkpoint.rs          ← Save/load/resume del estado
│   ├── validator.rs           ← Comando validate (pre-vuelo)
│   ├── init.rs                ← Comando init (scaffolding)
│   ├── groom.rs               ← Comando groom (generar backlog)
│   ├── hooks.rs               ← Comandos post-fase
│   ├── git.rs                 ← Snapshots + rollback
│   └── daemon.rs              ← Modo daemon (detach/follow/status/kill)
└── roadmap/                   ← Documentos de diseño de features futuras
```

## Tests

```bash
cargo test   # 104 tests, 0 fallos, 0 warnings
```

## Licencia

MIT
