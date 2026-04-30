# regista 🎬

AI agent director for [`pi`](https://github.com/mariozechner/pi-coding-agent).

Automates the full development pipeline with agents:
**PO → QA → Dev → Reviewer → Done**, governed by a formal
state machine with deadlock detection.

## Filosofía

Regista **no sabe nada de tu proyecto**. No importa si usas Rust,
Python, o lo que sea. Solo necesita tres cosas:

1. **Dónde están tus historias** de usuario (archivos `.md`)
2. **Qué skills de `pi`** actúan como PO, QA, Dev, Reviewer
3. **La máquina de estados fija** que gobierna las transiciones

## Instalación

```bash
git clone https://github.com/tu/regista
cd regista
cargo build --release
```

El binario estará en `target/release/regista`.

## Configuración

Crea un archivo `.regista.toml` en la raíz de tu proyecto:

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
max_iterations        = 10
max_retries_per_step  = 5
max_reject_cycles     = 3
agent_timeout_seconds = 1800
max_wall_time_seconds = 28800
retry_delay_base_seconds = 10

[hooks]
# Comandos opcionales de verificación post-fase.
# Si fallan, se hace rollback automático.
post_qa       = "npm test"
post_dev      = "npm run build && npm test && npm run lint"
post_reviewer = "npm test"

[git]
enabled = true   # snapshots + rollback automáticos
```

Todos los campos tienen valores por defecto razonables. Un proyecto mínimo
solo necesita definir `[agents]` y ajustar las rutas de `[project]`.

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

### Pipeline completo

```bash
# Procesar todo el backlog
regista /ruta/a/tu/proyecto

# Una sola iteración (procesa una historia y sale)
regista /ruta/a/tu/proyecto --once
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

Los filtros se pueden combinar solo si no son mutuamente excluyentes
(`--story` excluye `--epic` y `--epics`).

### Archivo de configuración alternativo

```bash
regista /ruta/a/tu/proyecto --config mi-config.toml
```

### Archivo de log personalizado

```bash
# Guardar logs en un archivo específico (en vez de stderr)
regista /ruta/a/tu/proyecto --log-file logs/debug.log
```

### Modo daemon

Regista puede correr en segundo plano, sobreviviendo a desconexiones SSH:

```bash
# Lanzar en segundo plano
regista /ruta/a/tu/proyecto --detach
# → Daemon lanzado con PID: 12345

# Consultar si sigue corriendo
regista /ruta/a/tu/proyecto --status
# → ✅ Daemon corriendo (PID: 12345, log: /ruta/.regista.log)

# Ver el log en vivo (como tail -f)
regista /ruta/a/tu/proyecto --follow
# Ctrl+C para salir (el daemon sigue corriendo)

# Detener el daemon
regista /ruta/a/tu/proyecto --kill
# → ✅ Daemon (PID: 12345) detenido correctamente.

# Log personalizado en modo daemon
regista /ruta/a/tu/proyecto --detach --log-file logs/orch.log
```

El estado del daemon se guarda en `<project_dir>/.regista.pid` (TOML).

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
                         ┌─── Reviewer rechaza ───┐
                         ▼                         │
In Review ────────────────────→ In Progress ──────┘
                         │                    Dev corrige → In Review
Business Review ──PO──→  In Review  (rechazo leve)
                   ──PO──→ In Progress (rechazo grave)
```

### Transiciones automáticas (sin agente)

| Transición | Condición |
|---|---|
| Cualquier estado → **Blocked** | Tiene dependencias no resueltas (`≠ Done`) |
| **Blocked** → **Ready** | Todas las dependencias pasan a `Done` |
| Cualquier estado → **Failed** | Se superan `max_reject_cycles` (3 por defecto) |

### Detección de QA fix

Cuando una historia está en `Tests Ready` y el último actor en el Activity Log
es **Dev** (reportó problemas con los tests), regista dispara al **QA**
para corregir los tests (`TestsReady → TestsReady`) en vez de al Developer.

### Deadlock detection

Si no hay historias accionables (`Ready`, `Tests Ready`, `InProgress`,
`InReview`, `BusinessReview`), regista analiza el grafo de dependencias
y dispara al **PO** para desatascar la historia que más bloqueos resuelve.

Prioridad de desbloqueo:
1. Historia que **bloquea más historias** (conteo de referencias inversas)
2. En empate, el **ID más bajo**

## Hooks de verificación

Se ejecutan comandos shell tras cada fase. Si fallan, se hace rollback
(vía `git reset --hard` si `git.enabled = true`):

| Hook | Cuándo se ejecuta |
|---|---|
| `post_qa` | Tras QA escribir/corregir tests |
| `post_dev` | Tras Dev implementar/corregir |
| `post_reviewer` | Tras Reviewer aprobar |

```toml
[hooks]
post_qa       = "cargo check --tests"
post_dev      = "cargo build && cargo test && cargo clippy -- -D warnings"
post_reviewer = "cargo test && cargo clippy -- -D warnings"
```

## Rollback con Git

Si `git.enabled = true`, antes de cada paso se crea un commit snapshot.
Si el agente falla o el hook no pasa, se hace `git reset --hard` al estado
anterior. Si no existe el repo, se inicializa automáticamente.

## Referencia completa de CLI

```
regista <PROJECT_DIR> [FLAGS]

FLAGS:
  --config <FILE>         Archivo de configuración alternativo
  --epics <RANGE>         Rango de épicas ("EPIC-001..EPIC-003")
  --epic <ID>             Una sola épica
  --story <ID>            Una sola historia
  --once                  Una iteración y salir
  --detach                Lanzar en segundo plano (daemon)
  --follow                Ver log en vivo del daemon
  --status                Consultar si el daemon sigue vivo
  --kill                  Detener el daemon
  --log-file <FILE>       Archivo de log (por defecto: stderr)
```

## Tests

```bash
cargo test   # 82 tests, 0 fallos, 0 warnings
```

## Licencia

MIT
