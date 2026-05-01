# 01 — Paralelismo

## 🎯 Objetivo

Ejecutar múltiples historias **independientes** de forma simultánea, en lugar
del procesamiento secuencial actual (una historia por iteración).

## 📍 Posición en el roadmap

**Fase 2** — se implementa justo después del multi-provider (#20).

El trait `AgentProvider` (definido en #20) ya está en producción y devuelve
`Vec<String>` (args de CLI, agnóstico a sync/async). El paralelismo simplemente
ejecuta esos comandos concurrentemente con `tokio`.

## ❓ Problema actual

El loop principal en `orchestrator.rs` procesa **una sola historia por iteración**.
En un backlog de 20+ historias, con agentes LLM que pueden tardar de 2 a 10
minutos cada uno, el pipeline completo puede llevar **horas**. Muchas de esas
historias no tienen dependencias entre sí y podrían avanzar en paralelo.

## ✅ Solución propuesta

### Modelo de concurrencia: `tokio` async

Se usa `tokio` (no `std::thread`) porque:

| Ventaja de tokio | Por qué importa |
|---|---|
| `tokio::time::timeout` por agente | Si Claude Code se cuelga, no bloquea el pipeline eternamente |
| `JoinHandle::abort()` | Cancelar oleada completa si un agente falla catastróficamente |
| `Semaphore` para rate limiting | Respetar límites de API del proveedor LLM |
| Canales para TUI futuro | El dashboard (#11) necesitará streaming en tiempo real |
| `FuturesUnordered` | Procesar resultados según van llegando, no en orden fijo |

### Algoritmo de oleadas (`independent_waves`)

```
1. Construir grafo de dependencias (ya existe en dependency_graph.rs)
2. topological_sort() → niveles BFS desde raíces sin dependencias
3. Cada nivel es una "oleada" (wave): historias procesables en paralelo
4. Para cada historia en la oleada:
     - Resolver provider (trait AgentProvider de #20)
     - provider.build_args(skill, prompt) → Vec<String>
     - tokio::spawn(async { Command::new(binary).args(args).output().await })
5. Esperar a que todos los spawns terminen (con timeout global de oleada)
6. apply_automatic_transitions() → nuevas historias pueden desbloquearse
7. Repetir con la siguiente oleada
```

### Ejemplo visual

```
Grafo:  STORY-001 ──→ STORY-003 ──→ STORY-005
        STORY-002 ──→ STORY-004 ──→ STORY-005

Oleada 1 (paralelo):  [STORY-001, STORY-002]    ← sin dependencias
Oleada 2 (paralelo):  [STORY-003, STORY-004]    ← dependen de 001/002 (ya Done)
Oleada 3 (secuencial): [STORY-005]              ← depende de 003/004
```

### Configuración

```toml
[limits]
max_concurrent_agents = 4   # default: 1 (sin paralelismo, compatibilidad hacia atrás)
agent_timeout_seconds = 1800
```

### Shared state con `Arc<Mutex<>>`

Los contadores que actualmente son `&mut HashMap<...>` locales pasan a ser
`Arc<Mutex<HashMap<...>>>` para que múltiples tasks puedan leer/escribir:

```rust
let reject_cycles = Arc::new(Mutex::new(reject_cycles));
let story_iterations = Arc::new(Mutex::new(story_iterations));

for story in &wave {
    let rc = reject_cycles.clone();
    let si = story_iterations.clone();
    tokio::spawn(async move {
        // ... ejecutar agente ...
        if fue_rechazo {
            rc.lock().await.entry(id).or_insert(0) += 1;
        }
    });
}
```

## 🚧 Problema de git snapshots

El modelo actual (`git add -A && git commit` antes de cada agente) no funciona
con múltiples agentes modificando archivos simultáneamente.

### Estrategia para v1

**Paralelismo solo entre épicas distintas** (asumiendo que épicas diferentes
no comparten archivos fuente). Las historias de una misma épica se procesan
secuencialmente. Esto elimina el problema de conflictos sin necesidad de
worktrees.

Para v2, se puede implementar `git worktree` por agente con merge strategy.

## 📝 Notas de implementación

### Archivos modificados

| Archivo | Cambio | Líneas |
|---------|--------|--------|
| `Cargo.toml` | Añadir `tokio` (features: rt-multi-thread, process, time, sync, fs) | +3 |
| `src/dependency_graph.rs` | Nuevo método `independent_waves() -> Vec<Vec<String>>` | +60 |
| `src/agent.rs` | `invoke_with_retry` e `invoke_once` → `async fn`; `tokio::process::Command` | +50 |
| `src/orchestrator.rs` | Loop principal refactorizado a oleadas; `process_story` → `async`; `Arc<Mutex<>>` | +180 |
| `src/git.rs` | Paralelismo restringido a épicas distintas en v1 | +5 |
| `src/config.rs` | Campo `max_concurrent_agents: u32` (default 1) | +10 |
| `src/main.rs` | `#[tokio::main]` o `Runtime::new()` explícito | +5 |
| `src/checkpoint.rs` | Guardar tras cada oleada, no tras cada historia | +10 |
| `src/story.rs` | Versiones async de `load` y `set_status` (`tokio::fs`) | +40 |
| `src/hooks.rs` | `run_hook` async con `tokio::process::Command` | +10 |
| `src/prompts.rs` | Sin cambios (el provider ya abstrae la invocación) | 0 |
| Tests | Tests de `independent_waves`, tests de concurrencia con `tokio::test` | +120 |
| **Total** | | **~493 líneas** |

### El `AgentProvider` NO necesita cambios

Como el trait de #20 devuelve `Vec<String>` (no `std::process::Command`), el
código async simplemente hace:

```rust
let args = provider.build_args(&skill, &prompt);
let output = tokio::process::Command::new(provider.binary())
    .args(args)
    .output()
    .await?;
```

El provider ni se entera de que es async. Esto es la prueba de que el orden
#20 → #01 es el correcto arquitectónicamente.

### Riesgos

- **Rate limiting de LLM**: el proveedor puede rechazar llamadas simultáneas.
  El backoff exponencial ya existente ayuda. Añadir `Semaphore` para no exceder
  `max_concurrent_agents`.
- **Determinismo**: el orden secuencial garantizaba reproducibilidad. Con
  paralelismo, dos ejecuciones pueden dar resultados distintos.
- **Errores en una oleada**: si 1 de 4 agentes falla, ¿cancelamos los otros 3 o
  los dejamos terminar? Decisión de diseño: dejar terminar (su trabajo no se
  pierde) y reintentar solo el fallido en la siguiente iteración.

## 🔗 Relacionado con

- [`20-multi-provider.md`](./20-multi-provider.md) — **prerrequisito**. El
  paralelismo invoca providers a través del trait `AgentProvider`.
- [`07-checkpoint-resume.md`](./07-checkpoint-resume.md) — el checkpoint se
  vuelve más importante con concurrencia (guardar tras cada oleada).
- [`12-cost-tracking.md`](./12-cost-tracking.md) — paralelismo = más gasto
  simultáneo en LLMs.
- [`11-tui-dashboard.md`](./11-tui-dashboard.md) — el dashboard usará los
  canales de tokio para streaming en tiempo real.
