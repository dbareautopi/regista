# Changelog

Todas las cambios notables de `regista` están documentados aquí.

El formato sigue [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
y el versionado sigue [SemVer](https://semver.org/spec/v2.0.0.html).

---

## [0.7.1] — 2026-05-04

### Fixed
- **Seguridad PowerShell en Windows**: el escape del prompt para el provider
  OpenCode ahora neutraliza caracteres especiales de PowerShell (`$`, `` ` ``,
  `"`) además de las comillas dobles. Evita inyección de comandos maliciosos.
- **Mensaje de error condicional en `daemon::kill()`**: sugiere `taskkill /F
  /PID` en Windows en lugar de `kill -9` (que no existe en Windows).

### Changed
- **Paths con `Path::join` en vez de `format!`**: reemplazadas concatenaciones
  manuales de paths con separador `/` por `Path::join()`, evitando separadores
  mixtos en Windows.
- **Tests sin paths hardcodeados `/tmp`**: los tests de `main.rs` y `daemon.rs`
  ahora usan paths relativos neutros en lugar de `/tmp/proj`, `/tmp/foo.log`,
  etc.

## [0.7.0] — 2026-05-04

### Added
- **`regista board` (#22)**: nuevo subcomando que muestra un dashboard Kanban
  con el conteo de historias por estado y lista las bloqueadas/fallidas con
  detalle (qué dependencias las bloquean, motivo del último rechazo).
  - `--json` para salida estructurada (CI/CD)
  - `--epic` para filtrar por épica
  - `--config` para ruta de configuración personalizada
  - Diseñado contra strings (`HashMap<String, usize>`), no contra variantes
    del enum `Status`, para resistir el futuro refactor de workflows
    configurables (#04).
- **`orchestrator::load_all_stories()`** promovida a `pub(crate)` para
  reutilización desde `board.rs` (antes era privada del orquestador).

### Changed
- **HANDOFF.md** actualizado: sesión v0.7.0, 173 tests, módulo `board.rs`
  documentado.
- **README.md** actualizado: sección `regista board` con ejemplos de uso
  y salida esperada. Arquitectura interna incluye `board.rs`.

## [0.6.3] — 2026-05-03

### Fixed
- **Prompt QA reescrito con reglas estrictas**: el QA ya no crea módulos,
  fake providers ni infraestructura de testing. Tampoco ejecuta build/tests —
  eso es trabajo del Developer. Solo escribe tests unitarios mínimos para
  los criterios de aceptación.
- **Prompt Dev con handoff explícito al QA**: cuando el Developer encuentra
  tests que no compilan, ahora sabe que NO debe corregirlos (solo reportar
  en el Activity Log y dejar el estado en TestsReady). El orquestador
  enruta automáticamente al QA vía la transición #5.
- **Timeout de agente ahora se aplica realmente**: `invoke_once()` ignoraba
  el parámetro `_timeout` y delegaba en `.output()` (bloqueante sin límite).
  Ahora usa `spawn()` + `try_wait()` en bucle, matando el proceso si supera
  `agent_timeout_seconds` (configurable, default 1800s).

## [0.6.2] — 2026-05-03

### Fixed
- **`log_file` relativo en `daemon.pid`**: el path del archivo de log se guardaba
  como ruta relativa (`./.regista/daemon.log`) en el archivo PID, lo que rompía
  `regista logs` si se ejecutaba desde otro directorio. Ahora se resuelve contra
  el directorio canónico del proyecto.
- **Mensajes de `status` y `kill` más informativos**: ahora muestran la ruta
  del directorio consultado y sugieren `regista status <dir>` para consultar
  otros proyectos, en lugar del genérico «no se encontró archivo PID».

## [0.6.1] — 2026-05-03

### Fixed
- **QA prompts ahora incluyen stack commands**: `qa_tests()` y `qa_fix_tests()`
  inyectan `stack.render()` igual que `dev_implement()`, `dev_fix()` y `reviewer()`.
  El QA ya no está ciego a la toolchain del proyecto.
- **`regista kill` mata procesos hijos**: ahora se recorren recursivamente
  `/proc/<pid>/task/*/children` y se mata a todos los descendientes antes
  que al daemon. Sin huérfanos.

### Changed
- **`max_reject_cycles` default sube de 3 a 8**: evita que historias casi
  completas mueran por un detalle trivial en el tercer ciclo de rechazo.

## [0.6.0] — 2026-05-03

### Added
- **Prompts agnósticos al stack (#09)**: los prompts ya no contienen hardcodeos
  de herramientas (`cargo`, `src/`). Los comandos de build, test, lint y
  formato se configuran en la nueva sección `[stack]` de `.regista/config.toml`.
- **`StackConfig`**: nuevo struct con `build_command`, `test_command`,
  `lint_command`, `fmt_command` y `src_dir` (todos opcionales).
- **`StackConfig::render()`**: genera el bloque de comandos para el prompt.
  Si no hay comandos definidos, devuelve instrucción genérica.
- **Helpers `header()` / `suffix()`** en `PromptContext`: componen prompts
  sin repetir el esqueleto común. Reutilizables para workflows custom (#04).

### Changed
- **7 prompts refactorizados**: `reviewer()`, `dev_implement()` y `dev_fix()`
  usan `stack.render()` en vez de comandos hardcodeados. `qa_tests()` usa
  `stack.src_dir` para placeholders. PO y QA fix prompts son stack-agnósticos.
- **Retrocompatibilidad total**: sin `[stack]` en TOML, los prompts funcionan
  exactamente igual que antes.
- **Documentación actualizada**: README, DESIGN, AGENTS, HANDOFF y ROADMAP
  reflejan la nueva sección `[stack]` y los prompts stack-agnósticos.

## [0.5.2] — 2026-05-03

### Changed
- **Renombrado `groom.rs` → `plan.rs`**: el módulo, structs, funciones y configuración usan ahora `plan` en vez de `groom` en todo el código fuente. Elimina la ambigüedad entre el nombre interno del módulo y el comando CLI.
  - `GroomResult` → `PlanResult`, `GroomCtx` → `PlanCtx`
  - `po_groom()` → `po_plan()`, `groom_prompt_*()` → `plan_prompt_*()`
  - `groom_max_iterations` → `plan_max_iterations`
- **Documentación sincronizada**: `AGENTS.md`, `HANDOFF.md`, `DESIGN.md` y `README.md` reflejan el renombrado.

## [0.5.1] — 2026-05-03

### Added
- **Comando `regista update`**: comprueba si hay una nueva versión en crates.io y la instala automáticamente con `cargo install`. Flag `--yes` para omitir la confirmación interactiva.
- **`regista --version` / `-V`**: muestra la versión instalada (nativo de clap).

### Changed
- **Repriorización del roadmap**: paralelismo (#01) movido de Fase 2 a Fase 7 (último). El orden ahora es: #20 → #09 → #14 → #10 → #04 → (#11, #12, #15) → #01.
- **Documentación actualizada**: `ROADMAP.md`, `HANDOFF.md` y `AGENTS.md` reflejan la CLI real de v0.5.0 (9 subcomandos, daemon, `plan` en vez de `groom`).

### Fixed
- **Clippy warning**: `map_or` → `is_none_or` en `update.rs`.

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

[0.7.1]: https://github.com/dbareautopi/regista/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/dbareautopi/regista/compare/v0.6.3...v0.7.0
[0.6.3]: https://github.com/dbareautopi/regista/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/dbareautopi/regista/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/dbareautopi/regista/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/dbareautopi/regista/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/dbareautopi/regista/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/dbareautopi/regista/compare/v0.5.0...v0.5.1
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
