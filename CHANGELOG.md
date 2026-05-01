# Changelog

Todas las cambios notables de `regista` están documentados aquí.

El formato sigue [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
y el versionado sigue [SemVer](https://semver.org/spec/v2.0.0.html).

---

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

[0.3.3]: https://github.com/dbareautopi/regista/compare/v0.3.2...v0.3.3
[0.3.4]: https://github.com/dbareautopi/regista/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/dbareautopi/regista/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/dbareautopi/regista/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/dbareautopi/regista/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/dbareautopi/regista/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dbareautopi/regista/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/dbareautopi/regista/releases/tag/v0.1.1
