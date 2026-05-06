# Changelog

Todas las cambios notables de `regista` están documentados aquí.

El formato sigue [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
y el versionado sigue [SemVer](https://semver.org/spec/v2.0.0.html).

---

## [0.9.4] — 2026-05-06

### Changed
- **QA Engineer skill → TDD puro (fase roja)**: prohibido modificar firmas de
  producción, no necesita `cargo test` en verde para avanzar a Tests Ready,
  máximo 2 iteraciones por historia. Los tests en rojo son el contrato que
  recibe el Developer.
- **Developer skill → TDD puro (fase verde)**: ciclo explícito rojo-verde-azul,
  recibe tests en rojo como contrato, implementa solo el código mínimo para
  hacerlos pasar, no reescribe tests del QA.
- **Eliminado `model` hardcodeado de `init.rs`**: las plantillas de skill ya
  no incluyen `model: opencode/minimax-m2.5-free` en el frontmatter YAML.
  Pi usará su modelo por defecto.

### Fixed
- **STORY-024 — bucle del QA resuelto**: el QA modificó la firma de
  `build_run_options()` rompiendo los call sites existentes. Revertido el
  cambio de producción y adaptados los tests TDD para construir `RunOptions`
  directamente sin depender de la firma nueva.

## [0.9.3] — 2026-05-06

### Fixed
- **Falso negativo en detección de cambio de estado corregido**: cuando un
  agente completaba exitosamente (exit code 0) y cambiaba el estado de la
  historia, el pipeline podía no detectar el cambio por condiciones de
  carrera del filesystem. Esto provocaba un `git reset --hard` que destruía
  todo el trabajo del agente, forzando reintentos innecesarios (timeouts de
  30 min).
  - Bucle de relectura del archivo con delays (0ms → 200ms → 500ms) para
    manejar buffering del sistema operativo.
  - Eliminado el rollback cuando el agente retorna exit code 0: los cambios
    se preservan siempre, incluso si el parseo de estado falla.
  - Log de confirmación cuando el archivo fue modificado (mtime) aunque el
    estado parseado no cambie.
- **`RunOptions.compact` añadido a `handlers.rs`**: el constructor de
  `RunOptions` en `build_run_options()` no incluía el campo `compact`
  añadido en STORY-027.

## [0.9.2] — 2026-05-06

### Fixed
- **Skills de agentes corregidos para prevenir deadlocks**: los 4 roles
  (Developer, QA Engineer, Reviewer, Product Owner) ahora incluyen reglas
  explícitas para detectar y romper ciclos de rechazos infinitos:
  - Developer: corrige errores de compilación triviales del QA en vez de
    reportarlos en bucle. Límite de 3 reintentos sin progreso.
  - QA Engineer: obligatorio ejecutar `cargo test` antes de marcar Tests Ready.
  - Reviewer: detecta conflictos Dev-QA (>5 iteraciones sin cambio de estado).
  - Product Owner: detecta deadlocks (>10 entradas sin cambio de estado).
- **STORY-022 (streaming de agente) completada**: la funcionalidad de
  streaming línea a línea con `invoke_once_verbose()` ya estaba implementada
  y pasando 25/25 tests, pero estaba bloqueada en un deadlock Dev-QA de 129
  iteraciones.

## [0.9.1] — 2026-05-05

### Fixed
- **Logs usan `log_dir` configurable**: los logs del daemon ahora se guardan en
  `log_dir` (`.regista/logs/` por defecto) con nombres timestamped
  (`regista-log-YYYYMMDD-HHMMSS.log`), usando la configuración que ya existía
  en `DESIGN.md` y `config.rs` pero no se aplicaba.
- **Rotación automática de logs**: se conservan solo los últimos 10 archivos
  de log, eliminando los más antiguos automáticamente para evitar crecimiento
  ilimitado del disco.
- Actualizado el fallback y documentación en `daemon.rs` para reflejar
  los nuevos paths.

## [0.9.0] — 2026-05-05

### Changed
- **Migración a tokio (async/await)**: `agent.rs` y `pipeline.rs` migrados a
  async con `tokio::process::Command` + `tokio::time::timeout`. Timeout real
  mata procesos por PID (zero zombies). Operaciones de bloqueo (git, hooks)
  usan `spawn_blocking`. Se añade dependencia `tokio` con features
  `rt-multi-thread`, `process`, `time`, `fs`.
- **Trait `Workflow` + `CanonicalWorkflow`**: la lógica de transiciones
  (`next_status`, `map_status_to_role`, `canonical_column_order`) se abstrae
  en un trait en `domain/workflow.rs`. `pipeline.rs` y `board.rs` consumen
  `&dyn Workflow` en lugar de funciones hardcodeadas. Prepara el terreno
  para workflows configurables (#04).
- **`SharedState` con `Arc<RwLock<>>`**: los contadores del orquestador
  (`reject_cycles`, `story_iterations`, `story_errors`) ahora usan
  `Arc<RwLock<HashMap<>>>` en lugar de `&mut HashMap` pasado por la pila.
  Clonable y compartible entre tareas — preparado para paralelismo (#01).
- **`max_reject_cycles` 3→8**: más tolerante a iteraciones de rechazo.

### Added
- **`app/health.rs` — Health & Metrics**: nuevo módulo con `HealthReport`
  (iteraciones/hora, tiempo medio agente, tasa rechazo, throughput, coste
  estimado). Escritura atómica a `.regista/health.json` (tmp → rename).
  `is_health_checkpoint()` dispara cada N iteraciones. Preparado para
  TUI (#11) y cost tracking (#12). 19 tests.
- **`tests/architecture.rs`**: 11 tests que verifican las reglas R1-R5 de
  dependencias entre capas (domain sin IO, infra sin lógica, etc.).
  Ejecutable con `cargo test --test architecture`.
- **Skills inline con YAML frontmatter**: `init.rs` incluye las instrucciones
  de rol como constantes con frontmatter completo (`name`, `model`,
  `description`). OpenCode usa `model:` para pasar `-m <model>`.

### Fixed
- **OpenCode agent name mismatch**: `instruction_dir()` convierte underscores
  a guiones para coincidir con el `name` del YAML frontmatter.
- **`--dry-run` ignorado**: los handlers no propagaban `dry_run: true`.

### Docs
- **AGENTS.md, HANDOFF.md, README.md** actualizados para reflejar la
  arquitectura en capas, nuevos módulos, y el flujo spec-first.

---

## [0.8.0] — 2026-05-04

### Changed
- **Arquitectura en capas**: el código se reorganiza en 4 capas (`cli/`, `app/`,
  `domain/`, `infra/`) con reglas de dependencia verificables automáticamente.
  `main.rs` pasa de 1,335 líneas a 18. La máquina de estados, el grafo de
  dependencias y los prompts ahora viven en `domain/` sin dependencias externas.
- **`config.rs` como datos puros**: los métodos `provider_for_role()` y
  `skill_for_role()` se mueven a `infra/providers.rs` como funciones libres,
  eliminando la dependencia inversa `config → infra`.
- **`DomainStackConfig`**: reemplaza a `StackConfig` en los prompts de dominio,
  eliminando la dependencia `domain → config`.

### Added
- **Test de arquitectura**: `tests/architecture.rs` con 11 tests que verifican
  las reglas R1-R5 (domain sin IO, infra sin lógica de negocio, etc.).
  Ejecutable con `cargo test --test architecture`.

### Fixed
- **OpenCode: agent name mismatch**: `instruction_dir()` ahora convierte
  underscores a guiones (`product_owner` → `product-owner`) para que coincida
  con el campo `name` del YAML frontmatter. Antes opencode no encontraba los
  agentes `product_owner` ni `qa_engineer` y usaba el agente `build` por defecto.
- **OpenCode: modelo no especificado**: `build_args()` ahora lee el campo
  `model:` del YAML frontmatter del archivo de instrucción y lo pasa a opencode
  vía `-m`. Los templates de `init` incluyen `model: opencode/minimax-m2.5-free`.
- **`--dry-run` ignorado**: los handlers de `plan`, `auto` y `run` no pasaban
  `dry_run: true` al orquestador, por lo que `--dry-run` ejecutaba agentes reales.
  Ahora el flag se propaga correctamente a `RunOptions`.

### Added
- Función `read_yaml_field()` en `providers.rs` para extraer campos del
  YAML frontmatter de archivos markdown (usada para leer `model:`).
- 4 tests nuevos en `providers::tests`: underscore conversion, model en YAML,
  y `read_yaml_field` con y sin frontmatter.

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

[0.9.4]: https://github.com/dbareautopi/regista/compare/v0.9.3...v0.9.4
[0.9.3]: https://github.com/dbareautopi/regista/compare/v0.9.2...v0.9.3
[0.9.2]: https://github.com/dbareautopi/regista/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/dbareautopi/regista/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/dbareautopi/regista/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/dbareautopi/regista/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/dbareautopi/regista/compare/v0.7.1...v0.7.2
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
