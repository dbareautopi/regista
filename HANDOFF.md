# 🧠 regista — Session Handoff

> **Fecha**: 2026-04-30 (actualizado)  
> **Sesión**: Migración de `scripts/orchestrator.sh` (bash) → `regista/` (Rust)  
> **Estado**: Funcional, 82 tests pasando. Daemon mode implementado. Pipeline completo.

---

## 📍 Dónde está todo

```
/root/repos/purist/
├── .regista.toml                 ← Config de regista para Purist
├── scripts/orchestrator.sh       ← Wrapper (thin, llama al binario)
├── regista/                      ← 🆕 Crate Rust
│   ├── Cargo.toml
│   ├── DESIGN.md                 ← Documento de diseño completo
│   ├── src/
│   │   ├── main.rs               ← CLI (clap)
│   │   ├── config.rs             ← Carga .regista.toml
│   │   ├── state.rs                ← Status, Actor, Transition
│   │   ├── story.rs                ← Parseo .md, set_status()
│   │   ├── dependency_graph.rs     ← Grafo, ciclos DFS, conteo inverso
│   │   ├── deadlock.rs             ← Detección de bloqueos + priorización
│   │   ├── agent.rs                ← Invoca `pi --skill ... -p "..."` con retry
│   │   ├── prompts.rs              ← 7 prompts para PO/QA/Dev/Reviewer
│   │   ├── orchestrator.rs         ← Loop principal, process_story()
│   │   ├── hooks.rs                ← Comandos post-fase
│   │   └── git.rs                  ← Snapshots + rollback
│   └── tests/fixtures/
│       ├── story_draft.md
│       ├── story_blocked.md
│       └── story_business_review.md
└── product/
    ├── stories/                    ← 21 historias (STORY-001..021)
    ├── epics/                      ← 5 épicas (EPIC-001..005)
    └── decisions/                  ← Directorio para decisiones de agentes
```

---

## ⚙️ Cómo funciona

### Pipeline de 4 agentes

```
Draft ──PO(groom)──→ Ready ──QA──→ Tests Ready ──Dev──→ In Review
                                                           │
                                                    Reviewer │
                                                           ▼
                       Done ←──PO(validate)── Business Review
                         ↑                        │
                         │    ┌───────────────────┘
                         │    ▼
                       In Review / In Progress (rechazo)
                         │    │
                         └────┘ (Dev corrige → In Review)
```

### Transiciones automáticas (sin agente)

- **Cualquier estado → Blocked**: si la historia tiene dependencias no resueltas (`!= Done`)
- **Blocked → Ready**: cuando todas sus dependencias pasan a `Done`
- **Cualquier estado → Failed**: cuando se superan `max_reject_cycles` (3 por defecto)

### Deadlock detection

Si el loop normal no encuentra historias accionables (`Ready`, `Tests Ready`, `InProgress`, `InReview`, `BusinessReview`), el módulo `deadlock.rs` analiza el grafo y:

1. **Draft** → dispara al PO para que refine (groom) la historia
2. **Blocked** con bloqueador en Draft → dispara al PO para el Draft bloqueante
3. **Blocked** con ciclo de dependencias → dispara al PO para romper el ciclo
4. Si no hay stuck ni accionables → **Pipeline Complete**

La prioridad la gana la historia que **desbloquea más historias** (conteo de referencias inversas). En empate, el ID más bajo.

### Ejemplo real con Purist (21 historias)

```
Estados actuales:
  Done:            001, 002, 003, 004, 005, 006, 011  (7)
  Draft:           007, 009, 012, 013, 014, 015       (6)
  Blocked:         008, 010, 016, 017, 018, 019, 020, 021  (8)

Análisis:
  STORY-007 (Draft) → desbloquea STORY-008, STORY-016 → score=2
  STORY-009 (Draft) → desbloquea STORY-010, STORY-018 → score=2
  STORY-012 (Draft) → desbloquea STORY-020, STORY-021 → score=2

  → Empate. Gana STORY-007 (ID más bajo).
  → Orquestador dispara PO para refinar STORY-007.
```

---

## 🔨 Cómo compilar y probar

```bash
# Compilar
cd regista && cargo build

# Tests (82 pasan, 1 ignorado)
cargo test

# Release
cargo build --release
```

## ▶️ Cómo ejecutar

```bash
# Desde la raíz del proyecto
./scripts/orchestrator.sh              # Pipeline completo
./scripts/orchestrator.sh --once       # Una sola iteración

# O directamente
./regista/target/debug/regista /root/repos/purist
```

---

## 📋 Lo implementado (F1–F10, F12)

| Módulo | Estado | Tests |
|--------|--------|-------|
| `main.rs` / CLI | ✅ 12 flags definidos con clap (incl. `--daemon` interno). Filtros, daemon commands y `--log-file` conectados | 4 tests |
| `config.rs` | ✅ Carga TOML, defaults, validación | 3 tests |
| `state.rs` | ✅ Status, Actor, Transition, `can_transition_to()` | 23 tests |
| `story.rs` | ✅ Parseo .md, `set_status()` con backup, `last_actor()` | 12 tests |
| `dependency_graph.rs` | ✅ Grafo, ciclo DFS, `blocks_count()` | 4 tests |
| `deadlock.rs` | ✅ Análisis de 4 casos, priorización | 7 tests |
| `agent.rs` | ✅ `pi --skill --no-session` con retry+backoff | 1 test (ignored) |
| `prompts.rs` | ✅ 7 funciones de prompt (incl. `qa_fix_tests` usado por `TestsReady→TestsReady`) | 4 tests |
| `orchestrator.rs` | ✅ Loop, `process_story()`, `apply_automatic_transitions()`, `filter_stories()`, `RunOptions`, `TestsReady→TestsReady` | 18 tests |
| `hooks.rs` | ✅ `run_hook()` | — |
| `git.rs` | ✅ `snapshot()`, `rollback()` | — |
| `daemon.rs` | ✅ `detach()`, `status()`, `kill()`, `follow()`, `PidCleanup` | 6 tests |
| `scripts/orchestrator.sh` | ✅ Thin wrapper, build-on-demand | — |

**Total: 82 tests pasando, 0 fallos, 1 ignorado. Cero warnings.**

---

## 🚧 Pendiente (próxima sesión)

### Baja prioridad
1. **Tests de integración con mock de `pi`**: tests del flujo completo sin depender del binario real
2. **`--epics` sin rango**: flag que acepte lista de épicas además del rango actual
3. **Manejo de señales en daemon**: capturar SIGTERM/SIGINT para limpiar PID file incluso en kills limpias
4. **Log rotation**: rotación automática del archivo de log en modo daemon

---

## 🧩 Contrato de historia (.md)

El orquestador espera este formato exacto:

```markdown
# STORY-NNN: Título

## Status
**Draft**   ← uno de: Draft, Ready, Tests Ready, In Progress, In Review, Business Review, Done, Blocked, Failed

## Epic
EPIC-XXX

## Descripción
...

## Criterios de aceptación
- [ ] CA1
- [ ] CA2

## Dependencias       ← opcional
- Bloqueado por: STORY-XXX, STORY-YYY

## Activity Log       ← obligatorio
- 2026-04-30 | PO | Movida de Draft a Ready
```

### Reglas de parseo
- **Status**: `## Status` → siguiente línea, limpia `**` y espacios
- **Bloqueadores**: busca `Bloqueado por:` (case-insensitive), extrae `STORY-\d+`
- **Epic**: `## Epic` → siguiente línea, extrae `EPIC-\d+`
- **Last rejection**: `## Activity Log` → última línea con "rechaz" (case-insensitive)

---

## 🔑 Decisiones de diseño

1. **Agnóstico al proyecto**: regista no sabe de Rust, cargo, ni Purist. Solo invoca `pi --skill <path>` con prompts genéricos. La configuración `.regista.toml` le dice dónde están las cosas.

2. **Workflow fijo**: las 14 transiciones son canónicas e inmutables. No se pueden añadir transiciones ad-hoc en runtime.

3. **Shell `true` en hooks**: `hooks.rs` ejecuta comandos con `sh -c`, igual que el `.sh` original. Los hooks son comandos shell, no binarios directos.

4. **Backoff exponencial**: `agent.rs` duplica el delay entre reintentos, mismo comportamiento que el `.sh`.

5. **`set_status()` con backup**: antes de escribir, copia a `.bak`; si la verificación falla, restaura. Operación atómica a nivel de archivo.
