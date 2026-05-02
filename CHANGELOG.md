# Changelog

Todas las cambios notables de `regista` están documentados aquí.

El formato sigue [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
y el versionado sigue [SemVer](https://semver.org/spec/v2.0.0.html).

---

## [0.5.0] — 2026-05-02

### Changed
- **Refactor completo de la CLI**: migración a `#[derive(Subcommand)]` de clap. Todos los comandos son ahora subcomandos propios con su `--help`.
- **100% daemon**: toda ejecución de pipeline spawnea un proceso en background. Ya no existe el modo bloqueante. Usa `--logs` para ver el progreso en vivo (Ctrl+C no detiene el daemon).
- **Nuevos subcomandos**:
  - `plan <spec>` — genera historias desde especificación (sustituye a `groom` standalone).
  - `auto <spec>` — planifica + ejecuta pipeline completo en un solo paso (sustituye a `groom --run`).
  - `run` — ejecuta pipeline sobre historias existentes.
  - `logs [dir]` — tail del log del daemon en vivo.
  - `status [dir]` — consulta si el daemon está corriendo.
  - `kill [dir]` — detiene el daemon.
  - `validate [dir]` — validación pre-vuelo.
  - `init [dir]` — scaffolding del proyecto.
- **Flag `--replace`** en `plan` y `auto` para modo destructivo (borrar historias antes de generar).
- **Flag `--logs`** en `plan`, `auto` y `run` para tail del log tras spawnear el daemon.

### Removed
- **`--detach`**: eliminado. Detach es ahora el comportamiento por defecto.
- **`--follow`**: renombrado a `--logs`.
- **`--json`**: deprecado. Se rediseñará en una versión futura.
- **`regista groom`**: sustituido por `regista plan` y `regista auto`.
- **`regista help`**: sustituido por `--help` automático de clap.
- **`regista [DIR]` a secas**: ahora requiere subcomando (`regista run`).

### Fixed
- **`daemon::detach()`** ahora acepta `child_args: &[String]` explícitos en vez de leer `std::env::args()`, permitiendo que `plan`, `auto` y `run` construyan sus propios argumentos para el proceso hijo.

## [0.4.1] — 2026-05-02

### Fixed
- **`groom` resolvía `project_root` incorrectamente**: usaba el directorio padre del spec como raíz del proyecto, haciendo que las historias se generaran en `specs/.regista/stories/` en vez de `.regista/stories/`. Ahora el project root es siempre el directorio actual (`.`), igual que el resto de comandos.
- **README actualizado** con nuevo flujo de trabajo "Specification-Driven Development" como flujo principal: el usuario escribe specs en la raíz del repo y regista genera el backlog en `.regista/`.

## [0.4.0] — 2026-05-02

### Added
- **`groom --run`**: flag que encadena groom → pipeline automáticamente. Tras generar las historias desde la spec, ejecuta `validate` completo y lanza el ciclo de desarrollo sin intervención manual.
- **Validación pre-pipeline en `--run`**: antes de lanzar el pipeline, se ejecuta `validator::validate()` (config, skills, historias, dependencias, Activity Log, git). Si hay errores, el pipeline se omite; los warnings se muestran pero se continúa.
- **Forwarding de flags de pipeline en `groom --run`**: `--once`, `--story`, `--epic`, `--epics`, `--dry-run`, `--json`, `--quiet` y `--resume` se aplican al pipeline lanzado automáticamente.
- **Modo `groom --run --json`**: suprime la salida legible del groom para no contaminar stdout (donde va el JSON del pipeline).

### Fixed
- **`--provider` en `groom`**: el flag se parseaba pero no se aplicaba a la configuración. Ahora afecta tanto al groom como al pipeline lanzado con `--run`.
- **Guardas de seguridad en `--run`**: el pipeline no se lanza si el groom generó 0 historias o si el grafo de dependencias quedó con errores.

## [0.3.4] — 2026-05-01

### Fixed
- **Compatibilidad con pi**: los SKILL.md generados por `regista init` ahora incluyen frontmatter YAML con `name` y `description`, requeridos por pi.
- **Nombres de skill**: corregidos nombres de skill con underscores (`product_owner`, `qa_engineer`) que pi rechazaba. Ahora usan hyphens (`product-owner`, `qa-engineer`).

---

## [0.3.3] — 2026-05-01

### Fixed
- **Subcomandos `init` y `validate`**: corregido bug donde flags (`--light`, `--json`, etc.) se interpretaban erróneamente como directorio del proyecto. Ahora detectan correctamente si el primer argumento es un flag y usan `.` como directorio por defecto.

---

## [0.3.2] — 2026-05-01

### Fixed
- **Links en README**: corregidos links al repo de `pi` (`badlogic/pi-mono`) y `opencode` (`anomalyco/opencode`)
- **Integración con OpenCode**: la invocación usaba flags incorrectos (`-p`/`-q`). Ahora usa `run --agent <role> --dangerously-skip-permissions`, que es el API real de OpenCode
- **OpenCode `instruction_dir`**: cambiado de `.opencode/commands/` a `.opencode/agents/`. OpenCode lee agentes desde `.opencode/agents/*.md` y usa el contenido del archivo como system prompt del agente

## [0.3.1] — 2026-05-01

### Fixed
- Silenciar warnings de `dead_code` introducidos tras la implementación multi-provider (#20)

## [0.3.0] — 2026-05-01

### Added
- **Sistema multi-provider**: soporte para `pi`, `claude` (Claude Code), `codex` (Codex), y `opencode` (OpenCode)
- Provider por rol: cada rol (PO, QA, Dev, Reviewer) puede usar un provider distinto, configurable en `.regista/config.toml`
- Flag `--provider` en CLI para sobreescribir el provider global
- `init` genera scaffolding específico del provider (`.pi/skills/`, `.claude/agents/`, `.agents/skills/`, `.opencode/commands/`)
- `validate` chequea paths de instrucciones según el provider de cada rol

### Changed
- `AgentProvider` trait devuelve `Vec<String>` (args de CLI) en vez de `Command` — compatible con sync y async
- `invoke_with_retry` recibe `&dyn AgentProvider` como primer argumento en vez de hardcodear `pi`
- `AgentsConfig` con `provider` global + `AgentRoleConfig` por rol con `provider` y `skill` opcionales
- `init` usa `AgentProvider::instruction_dir(role)` para colocar instrucciones según el provider

## [0.2.0] — 2026-04-30

### Added
- Comando `help` — muestra todos los comandos y flags disponibles
- Migración a `.regista/` como directorio de configuración

### Changed
- Estructura de directorios: config en `.regista/config.toml`, estado en `.regista/state.toml`
- Documentación completa: README, DESIGN, HANDOFF, AGENTS y ROADMAP actualizados

## [0.1.1] — 2026-04-29

### Added
- Release inicial con pipeline base: Draft → Ready → Tests Ready → In Review → Business Review → Done
- Máquina de estados con 14 transiciones canónicas
- Detección de deadlocks con priorización
- Checkpoint/resume del estado del orquestador
- Subcomandos `validate`, `init`, `groom`
- Dry-run, salida JSON, feedback rico en reintentos
- Hooks post-fase y snapshots git

[0.5.0]: https://github.com/dbareautopi/regista/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/dbareautopi/regista/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/dbareautopi/regista/compare/v0.3.4...v0.4.0
[0.3.4]: https://github.com/dbareautopi/regista/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/dbareautopi/regista/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/dbareautopi/regista/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/dbareautopi/regista/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/dbareautopi/regista/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dbareautopi/regista/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/dbareautopi/regista/releases/tag/v0.1.1
