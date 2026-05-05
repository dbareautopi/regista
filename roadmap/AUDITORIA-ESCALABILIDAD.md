
# 🔬 Auditoría de Escalabilidad — regista v0.8.0

> **Fecha**: 2026-05-04
> **Versión analizada**: v0.8.0 (post-refactor a capas `app/domain/infra`)
> **Alcance**: arquitectura actual, riesgos de crecimiento, preparación para features futuras (#01 paralelismo, #04 workflow configurable, #10 cross-story context, #11 TUI).

---

## 📋 Tabla de contenidos

1. [Máquina de Estados — Resistencia al Workflow Configurable](#1--máquina-de-estados--resistencia-estructural-al-workflow-configurable-04)
2. [I/O Síncrono en el Hot Path](#2--io-síncrono-en-el-hot-path--escalabilidad-lineal-al-nº-de-historias)
3. [Checkpoint — Crecimiento No Acotado](#3--checkpoint--crecimiento-no-acotado-sin-compresión)
4. [Git Snapshots y Rollback — Incompatibles con Paralelismo](#4--git-snapshots-y-rollback--incompatibles-con-paralelismo)
5. [PromptContext — Clonaciones y Allocations Innecesarias](#5--promptcontext--clonaciones-y-allocations-innecesarias)
6. [Daemon — Robustez y Consumo de Recursos](#6--daemon--robustez-y-consumo-de-recursos)
7. [Providers — `from_name()` con `panic!`, Sin Validación](#7--providers--from_name-con-panic-sin-validación-de-disponibilidad)
8. [Config — Carga y Validación Parcial](#8--config--carga-y-validación-parcial)
9. [Formato de Historia — Contrato Frágil](#9--formato-de-historia--contrato-frágil)
10. [Preparación para Paralelismo — Lo Bueno y lo Pendiente](#10--preparación-para-paralelismo-01--lo-bueno-y-lo-pendiente)
11. [Observaciones Menores](#11--observaciones-menores)
12. [Resumen de Impacto](#-resumen-de-impacto)
13. [Recomendaciones Priorizadas (por ROI)](#-recomendaciones-prioritarias-ordenadas-por-roi)

---

## 1. 🚨 Máquina de Estados — Resistencia Estructural al Workflow Configurable (#04)

### 1.1 La dualidad enum/string ya está fragmentando el código

`Status` es un `enum` de 9 variantes (fijo, inmutable). Pero `board.rs` ya opera con `status.to_string()` porque anticipa #04. Esto crea **dos APIs paralelas** para lo mismo:

| Módulo | Cómo accede al estado | Problema |
|--------|----------------------|----------|
| `pipeline.rs` | `Status` enum (`story.status == Status::Blocked`) | ✅ |
| `board.rs` | `status.to_string()` → `"Blocked"` literal | ❌ frágil |
| `deadlock.rs` | `Status` enum + `status_map: HashMap<&str, Status>` | ⚠️ mezcla |
| `prompts.rs` | `Status` enum en `PromptContext` | ✅ |

**Consecuencia**: el día que `Status` pase a ser dinámico, `board.rs` es el único que sobrevive intacto. El resto necesita reescritura completa. Si llega un bug entre medias, habrá que razonar sobre dos representaciones distintas del mismo concepto simultáneamente.

### 1.2 `next_status()` está hardcodeado a un solo flujo

```rust
// src/app/pipeline.rs
fn next_status(current: Status) -> Status {
    match current {
        Status::Draft => Status::Ready,
        Status::Ready => Status::TestsReady,
        Status::TestsReady => Status::InReview,
        Status::InProgress => Status::InReview,
        Status::InReview => Status::BusinessReview,
        Status::BusinessReview => Status::Done,
        _ => current,
    }
}
```

Esto asume exactamente **1 sucesor por estado, en 1 workflow fijo**. Equipos que no usen QA (sin `TestsReady`) necesitarán otro `next_status()` dinámico. La función debería ser `next_status(current, workflow) -> Status`.

### 1.3 `map_status_to_role()` también hardcodea la asignación

```rust
// src/app/pipeline.rs
fn map_status_to_role(status: Status) -> &'static str {
    match status {
        Status::Draft | Status::BusinessReview => "product_owner",
        Status::Ready => "qa_engineer",
        Status::TestsReady | Status::InProgress => "developer",
        Status::InReview => "reviewer",
        _ => "product_owner", // fallback seguro
    }
}
```

En un workflow custom, `TestsReady` podría no existir, y `InProgress` podría asignarse a `"qa_engineer"`. Esta función debe leerse de la configuración del workflow.

### 1.4 `apply_automatic_transitions()` tiene transiciones hardcodeadas

```rust
// src/app/pipeline.rs - apply_automatic_transitions()
if story.status != Status::Blocked { continue; }
// ...
if all_blockers_done {
    story.set_status(Status::Ready)?;  // ← hardcodeado: Blocked → Ready
}
// ...
if cycles >= cfg.limits.max_reject_cycles {
    story.set_status(Status::Failed)?; // ← hardcodeado: * → Failed
}
```

Si un workflow custom define estados con nombres distintos, este código no se entera.

### 1.5 `print_human()` en `board.rs` depende del orden canónico

```rust
// src/app/board.rs
let canonical_order = [
    "Draft", "Ready", "Tests Ready", "In Progress",
    "In Review", "Business Review", "Done", "Blocked", "Failed",
];
```

Cualquier workflow custom que añada estados nuevos los mostrará en orden arbitrario, o peor, los estados eliminados aparecerán con `count = 0` permanentemente.

**Impacto acumulado para #04**: ~500 líneas a cambiar en `state.rs`, `pipeline.rs`, `deadlock.rs`, `prompts.rs`, y `board.rs`.

---

## 2. 🚨 I/O Síncrono en el Hot Path — Escalabilidad Lineal al Nº de Historias

### 2.1 `load_all_stories()` en cada iteración

El pipeline carga y parsea **todas** las historias al inicio de cada iteración del bucle principal.

```
Iteración 1: 50 lecturas de archivo
Iteración 2: 50 lecturas de archivo
...
Iteración 300: 50 lecturas de archivo
─────────────────────────────────
Total: 15.000 lecturas de archivo
```

La inmensa mayoría de archivos **no cambian** entre iteraciones. Solo la historia procesada en la iteración N modifica su archivo.

**Solución propuesta**: `StoryCache` con invalidación basada en `mtime` del sistema de archivos. Solo re-parsear historias cuyo archivo fue modificado desde la última lectura.

```rust
// Idea:
struct StoryCache {
    stories: HashMap<String, CachedStory>,
}
struct CachedStory {
    story: Story,
    mtime: SystemTime,
}
```

### 2.2 `DependencyGraph::from_stories()` reconstruido 2-3 veces por iteración

```rust
// En run_real():
let full_graph = DependencyGraph::from_stories(&stories);          // 1ª construcción
let stories = apply_automatic_transitions(stories, &full_graph, ...)?;
let graph = DependencyGraph::from_stories(&stories);               // 2ª construcción

// En pick_next_actionable (llamada indirecta):
let graph = DependencyGraph::from_stories(&stories);               // 3ª (en dry-run)
```

Cada reconstrucción itera sobre todas las historias y sus bloqueadores: **O(N × B)** donde B = media de dependencias por historia. Para 100 historias con 3 dependencias de media:

- 3 reconstrucciones × 100 historias × 3 dependencias = **900 inserciones en HashMap por iteración**
- 300 iteraciones → **270.000 inserciones en HashMap totales**

En dry-run, el grafo se reconstruye aún más veces (bucle interno del simulador + bucle externo).

### 2.3 `git add -A` en cada snapshot

```rust
// src/infra/git.rs
Command::new("git").arg("-C").arg(project_root).arg("add").arg("-A")...
```

Esto stajea **todo** el working tree del repositorio, no solo los cambios realizados por el agente. En un repositorio de 50.000 archivos:

- Cada `git add -A` escanea el árbol completo del sistema de archivos
- 300 iteraciones → 300 `git add -A`
- Tiempo estimado por `git add -A`: 0.5-2 segundos → **2.5-10 minutos solo en git add**

**Solución propuesta**: `git add` limitado a los paths conocidos que los agentes modifican:

```rust
Command::new("git").arg("-C").arg(project_root)
    .arg("add")
    .arg(".regista/stories/")
    .arg(".regista/decisions/")
    .arg("src/")  // configurable según stack
```

### 2.4 Busy-polling en `invoke_once()`

```rust
// src/infra/agent.rs
let poll = Duration::from_millis(250);
loop {
    match child.try_wait() {
        Ok(Some(_status)) => { /* proceso terminó */ }
        Ok(None) => {
            if start.elapsed() >= timeout { /* matar */ }
            std::thread::sleep(poll);  // ← bloquea 250ms
        }
    }
}
```

Este loop **bloquea un thread del sistema operativo** durante toda la ejecución del agente (2-10 minutos). En un futuro con paralelismo (#01), esto es insostenible: necesitarías un thread por agente concurrente.

El trait `AgentProvider` devuelve `Vec<String>` (no `Command`) precisamente para ser async-compatible, pero `agent.rs` no lo aprovecha. La migración de `thread::sleep` a `tokio` es **invasiva**: toca `invoke_once`, `invoke_with_retry`, y toda la cadena de callers que asumen sincronía.

---

## 3. 🟠 Checkpoint — Crecimiento No Acotado, Sin Compresión

### 3.1 Los HashMaps crecen sin límite

```rust
// src/infra/checkpoint.rs
pub struct OrchestratorState {
    pub iteration: u32,
    pub reject_cycles: HashMap<String, u32>,     // 1 entrada por historia rechazada
    pub story_iterations: HashMap<String, u32>,  // 1 entrada por historia procesada
    pub story_errors: HashMap<String, String>,   // 1 entrada por historia con error
}
```

En un proyecto de 200 historias donde todas fallan al menos una vez, el checkpoint serializa **400+ entradas** en TOML. Cada `save()` reescribe el archivo completo (no hay escritura incremental).

### 3.2 No hay compaction

Las historias que llegan a `Done` o `Failed` mantienen sus entradas en `story_iterations` y `reject_cycles` para siempre. No hay ningún mecanismo que elimine entradas de historias terminales del checkpoint.

### 3.3 `story_errors` guarda strings completos

Errores como:

> `"agotados 5 reintentos invocando pi (.pi/skills/developer/SKILL.md)"`

...se repiten para cada historia. En 200 historias fallidas con el mismo error, eso son ~80KB de strings redundantes serializados.

### 3.4 `save_checkpoint()` clona todos los HashMaps en cada iteración

```rust
// src/app/pipeline.rs
fn save_checkpoint(...) {
    let state = OrchestratorState {
        reject_cycles: reject_cycles.clone(),     // O(R)
        story_iterations: story_iterations.clone(), // O(S)
        story_errors: story_errors.clone(),       // O(E)
    };
}
```

Tres clones completos de HashMap por iteración. Para 300 iteraciones con 50 entradas cada uno: **45.000 clonaciones de entradas de HashMap**.

---

## 4. 🟠 Git Snapshots y Rollback — Incompatibles con Paralelismo

### 4.1 Snapshots globales, no por historia

Un snapshot (`git commit`) captura el estado de **todo** el repositorio. Dos agentes paralelos modificando `STORY-001.md` y `STORY-002.md` no pueden hacer rollback independiente: el hash del commit es compartido.

El roadmap reconoce este problema y propone restringir el paralelismo a **épicas distintas** en v1. Pero esto es un parche, no una solución: asume que épicas distintas nunca comparten archivos fuente, lo cual no es cierto en proyectos reales.

### 4.2 Rollback con `git reset --hard` es destructivo para trabajo concurrente

```rust
// src/infra/git.rs
pub fn rollback(project_root: &Path, prev_hash: &str, label: &str) -> bool {
    Command::new("git").arg("-C").arg(project_root)
        .arg("reset").arg("--hard").arg(prev_hash)...
}
```

Si el Agente A falla y el Agente B tuvo éxito en paralelo, `git reset --hard` al hash pre-A **borra el trabajo exitoso de B**. No hay merge strategy ni conflict resolution.

### 4.3 No hay garbage collection de snapshots

Un pipeline de 300 iteraciones con `git.enabled = true` genera **300 commits de snapshot**. El historial de git crece linealmente sin límite:

- No hay squash de snapshots antiguos
- No hay `git gc` automático
- El tamaño del repositorio crece proporcionalmente al número de iteraciones

---

## 5. 🟠 PromptContext — Clonaciones y Allocations Innecesarias

### 5.1 Clonación ad-hoc en `process_story()` para `qa_fix_tests()`

```rust
// src/app/pipeline.rs
let qa_ctx = PromptContext {
    to: Status::TestsReady,
    story_id: ctx.story_id.clone(),          // malloc + memcpy de String
    stories_dir: ctx.stories_dir.clone(),    // malloc + memcpy de String
    decisions_dir: ctx.decisions_dir.clone(), // malloc + memcpy de String
    last_rejection: ctx.last_rejection.clone(), // malloc + memcpy de Option<String>
    from: ctx.from,
    stack: ctx.stack.clone(),                // malloc de DomainStackConfig (5 Option<String>)
};
```

Esto ocurre en el hot path del pipeline. Para cada historia procesada, se realiza este patrón de clonación.

### 5.2 `DomainStackConfig::render()` asigna un `Vec<String>` cada llamada

```rust
// src/domain/prompts.rs
pub fn render(&self) -> String {
    let mut parts: Vec<String> = Vec::new();  // nuevo Vec cada vez
    if let Some(ref cmd) = self.build { parts.push(...); }
    if let Some(ref cmd) = self.test { parts.push(...); }
    // ...
}
```

Este método se llama en 4 de los 7 prompts (`qa_tests`, `qa_fix_tests`, `dev_implement`, `dev_fix`, `reviewer`). El `Vec<String>` intermedio es efímero: el resultado se formatea inmediatamente en un `String` más grande y se descarta.

**Solución**: devolver directamente el `String` formateado sin el `Vec` intermedio, o cachear el resultado de `render()` en `DomainStackConfig` (es inmutable durante toda la vida del pipeline).

### 5.3 Templates inline con `format!()` — sin reutilización

Los 7 prompts usan `format!()` con literales de plantilla inline en cada método:

```rust
format!("{header}\nValídala contra el DoR...\n{suffix}", ...)
```

Esto implica:
- Si el formato de prompt cambia, hay que modificar 7 funciones
- Si se añade una variable nueva (ej: `cross_story_context` para #10), hay que modificar 7 firmas de método
- No hay pre-compilación de plantillas ni reutilización de strings comunes

---

## 6. 🟡 Daemon — Robustez y Consumo de Recursos

### 6.1 `follow()` con polling a 200ms

```rust
// src/infra/daemon.rs
match file.read(&mut buf) {
    Ok(0) => {
        thread::sleep(Duration::from_millis(200)); // ← polling
    }
}
```

Es ineficiente para logs grandes. `inotify` (Linux) o `ReadDirectoryChangesW` (Windows) permitirían seguimiento reactivo sin polling.

### 6.2 `drain_remaining()` carga todo el resto del archivo en memoria

```rust
// src/infra/daemon.rs
fn drain_remaining(file: &mut fs::File) -> anyhow::Result<()> {
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;  // ← carga todo en RAM
    ...
}
```

Si el log tiene 10MB en el momento en que el daemon termina, esto aloca 10MB de una vez.

### 6.3 `kill()` con `sleep()` fijos — condición de carrera

```rust
// src/infra/daemon.rs
send_signal(child_pid, 15); // SIGTERM
thread::sleep(Duration::from_secs(2)); // ← espera fija
if is_process_alive(child_pid) {
    send_signal(child_pid, 9); // SIGKILL
    thread::sleep(Duration::from_millis(500));
}
```

No hay garantía de que 2 segundos sean suficientes. Un agente LLM puede tardar 30s en hacer cleanup de su estado (guardar conversación, cerrar archivos). Si el proceso muere justo entre el `sleep` y el `send_signal(SIGKILL)`, el `SIGKILL` podría enviarse a un PID **reutilizado** por el SO para otro proceso.

### 6.4 `detach()` usa `current_exe()` — frágil ante actualizaciones

```rust
// src/infra/daemon.rs
let exe = std::env::current_exe()?;
let child = Command::new(&exe).args(child_args)...spawn()?;
```

Si `regista` se actualiza con `cargo install regista` mientras el daemon corre, `current_exe()` apunta al binario nuevo o al antiguo dependiendo de la plataforma y del momento exacto. El daemon no puede garantizar que se re-spawnee a sí mismo con la misma versión.

### 6.5 Sin rotación de logs

`daemon.log` crece indefinidamente. Un pipeline de 8 horas con tracing a nivel `info` produce aproximadamente **50-100MB de log**. No hay `tracing-appender` con rotación por tamaño o tiempo configurada.

---

## 7. 🟡 Providers — `from_name()` con `panic!`, Sin Validación de Disponibilidad

### 7.1 `from_name()` hace `panic!` en vez de devolver `Result`

```rust
// src/infra/providers.rs
pub fn from_name(name: &str) -> Box<dyn AgentProvider> {
    match name.to_lowercase().as_str() {
        "pi" => Box::new(PiProvider),
        "claude" | "claude-code" | "claude_code" => Box::new(ClaudeCodeProvider),
        "codex" => Box::new(CodexProvider),
        "opencode" | "open-code" | "open_code" => Box::new(OpenCodeProvider),
        other => panic!("provider desconocido: '{other}'..."),
    }
}
```

Un `panic` en Rust **aborta el proceso**. Si un usuario escribe mal el nombre del provider en `.regista/config.toml`, el pipeline muere sin cleanup, sin rollback, y sin mensaje de error útil (solo el backtrace del panic). Debería ser:

```rust
pub fn from_name(name: &str) -> anyhow::Result<Box<dyn AgentProvider>>
```

### 7.2 No se verifica que el binario del provider exista

Ningún provider verifica que `pi`, `claude`, `codex`, u `opencode` estén instalados en el sistema antes de intentar invocarlos. El error se detecta recién en `invoke_once()` cuando `Command::new().spawn()` falla con:

> `"no se pudo ejecutar 'pi': No such file or directory (os error 2). ¿Está instalado?"`

**Solución**: `validate.rs` ya verifica que los archivos de skill existen. Podría extender la validación para verificar que los binarios de los providers configurados están en el PATH (usando `which::which` o similar).

### 7.3 `CodexProvider` ignora `instruction_path` — skill path fantasma

```rust
// src/infra/providers.rs
impl AgentProvider for CodexProvider {
    fn build_args(&self, _instruction: &Path, prompt: &str) -> Vec<String> {
        vec!["exec".to_string(), "--sandbox".to_string(), "workspace-write".to_string(), ...]
    }
}
```

Codex auto-descubre skills desde `.agents/skills/`. Esto es correcto, pero significa que `skill_for_role(&cfg.agents, "developer")` para Codex devuelve un path (`.agents/skills/developer/SKILL.md`) que **nunca se usa** en `build_args`. La validación en `validate.rs` comprueba que el archivo existe... para un path que luego se ignora.

---

## 8. 🟡 Config — Carga y Validación Parcial

### 8.1 `Config::load()` no valida `epics_dir`

Solo verifica que `stories_dir` existe. Si el usuario configura:

```toml
[project]
epics_dir = "epicas"
```

...y el directorio no existe, el pipeline falla en runtime cuando `plan.rs` intenta escribir épicas, sin un mensaje de error claro en la validación inicial.

### 8.2 `Config::validate()` crea directorios como efecto secundario

```rust
// src/config.rs
fn validate(&self, project_root: &Path) -> anyhow::Result<()> {
    for dir in [&self.project.decisions_dir, &self.project.log_dir] {
        std::fs::create_dir_all(&path)?;  // ← side effect
    }
    Ok(())
}
```

Una función llamada `validate` no debería tener efectos secundarios en el filesystem. La creación de directorios debería ser responsabilidad del orchestrator al arrancar, o de `init`.

### 8.3 No hay límite configurable para `story_errors`

El HashMap `story_errors: HashMap<String, String>` crece sin cota. Si 500 historias fallan, el checkpoint incluye 500 entradas de error serializadas. No hay un `max_story_errors` configurable que limite el crecimiento.

---

## 9. 🟡 Formato de Historia — Contrato Frágil

### 9.1 `set_status()` usa reemplazo posicional frágil

```rust
// src/domain/story.rs
for i in 0..lines.len() {
    if lines[i].to_lowercase().trim() == "## status" {
        if i + 1 < lines.len() {
            let old_line = &lines[i + 1];
            let leading = old_line.len() - old_line.trim_start().len();
            let trailing = old_line.len() - old_line.trim_end().len();
            // Reconstruir la línea con la nueva indentación
            lines[i + 1] = format!("{}{}{}", " ".repeat(leading), new_status_str, " ".repeat(trailing));
        }
    }
}
```

Si un agente modifica accidentalmente el formato de `## Status` (añade comentarios, cambia `**Draft**` por `_Draft_`, o inserta líneas extra), la escritura o bien falla silenciosamente o corrompe el archivo. La verificación posterior (`Story::load`) detecta la corrupción, pero:

- Entre la escritura corrupta y la detección + restauración del backup, hay una **ventana de lectura** donde otro agente (en un futuro con paralelismo) podría leer el estado corrupto.
- No hay locking a nivel de archivo para prevenir lecturas concurrentes durante escrituras.

### 9.2 `advance_status_in_memory()` usa `replacen` con riesgo de colisión

```rust
// src/domain/story.rs
pub fn advance_status_in_memory(&mut self, new_status: Status) {
    let old = format!("**{}**", self.status);
    let new = format!("**{}**", new_status);
    self.raw_content = self.raw_content.replacen(&old, &new, 1);
}
```

Si el contenido de la historia contiene el string `**Draft**` en otro lugar (por ejemplo, en el `## Activity Log`: `"- 2026-01-01 | PO | Movida de **Draft** a **Ready**"`), `replacen` reemplaza la **primera** ocurrencia, que podría no ser la línea de `## Status`.

### 9.3 Sin versión del formato de historia

No hay forma de que el parser (`parse_status`, `parse_epic`, etc.) sepa si una historia sigue el formato v1 o v2. Si el contrato evoluciona (ej: se añade `## Priority`), todas las historias existentes necesitan migración manual o un migrador automático que no existe.

---

## 10. 🔵 Preparación para Paralelismo (#01) — Lo Bueno y lo Pendiente

### 10.1 ✅ El trait `AgentProvider` está bien diseñado para async

Devolver `Vec<String>` en vez de `std::process::Command` fue la decisión arquitectónica correcta. Permite construir `tokio::process::Command` sin tocar el trait:

```rust
// Futuro código async:
let args = provider.build_args(&skill_path, &prompt);
let output = tokio::process::Command::new(provider.binary())
    .args(args)
    .output()
    .await?;
```

El provider ni se entera de que es async. Esto valida el orden del roadmap: #20 → #01.

### 10.2 ❌ Pero `agent.rs` es totalmente síncrono

`invoke_with_retry`, `invoke_once`, y `save_agent_decision` bloquean el thread. Migrar a async requiere:

| Cambio | Módulo | Impacto |
|--------|--------|---------|
| `invoke_once` con `tokio::process::Command` | `agent.rs` | ~30 líneas |
| `invoke_with_retry` async con `tokio::time::sleep` | `agent.rs` | ~20 líneas |
| `process_story` async | `pipeline.rs` | ~40 líneas |
| Loop principal con `tokio::spawn` para oleadas | `pipeline.rs` | ~150 líneas |
| `run_hook` async con `tokio::process::Command` | `hooks.rs` | ~10 líneas |
| `snapshot` y `rollback` async (o spawn_blocking) | `git.rs` | ~15 líneas |

### 10.3 ❌ Los HashMaps de estado no son `Send + Sync`

```rust
// Actualmente en pipeline.rs (variables locales):
let mut reject_cycles: HashMap<String, u32> = ...;
let mut story_iterations: HashMap<String, u32> = ...;
```

Necesitarán `Arc<Mutex<HashMap<...>>>` o `Arc<RwLock<HashMap<...>>>` para ser compartidos entre tasks de tokio. Pero el código actual los pasa como `&mut` a través de la pila de llamadas. Cambiar a `Arc` implica modificar las firmas de:

- `process_story()`
- `apply_automatic_transitions()`
- `save_checkpoint()`
- `handle_deadlock()` (indirectamente)

### 10.4 ❌ Las operaciones de archivo no son atómicas entre agentes concurrentes

Dos agentes paralelos escribiendo a archivos distintos (`STORY-001.md`, `STORY-002.md`) es seguro. Pero dos agentes escribiendo al mismo archivo fuente (ej: ambos modifican `src/lib.rs`) generan conflictos de escritura sin detección ni resolución.

El roadmap propone limitar el paralelismo a **épicas distintas** en v1, asumiendo que épicas diferentes no comparten archivos. Esto no es cierto en la práctica y el código no impone esta restricción.

---

## 11. 🔵 Observaciones Menores

### 11.1 `extract_numeric()` duplicada en 4 módulos

La misma función aparece idéntica en:

```
src/app/pipeline.rs    → fn extract_numeric(id: &str) -> u32
src/domain/deadlock.rs → fn extract_numeric(id: &str) -> u32
src/app/board.rs       → fn extract_numeric(id: &str) -> u32
```

Debería ser un único free function en `domain` (ej: `domain::story::extract_numeric`), o un método asociado a `Story` (`Story::numeric_id()`).

### 11.2 `provider_for_role` / `skill_for_role` en `providers.rs` mezclan concerns

```rust
// src/infra/providers.rs
pub fn provider_for_role(agents: &crate::config::AgentsConfig, role: &str) -> String { ... }
pub fn skill_for_role(agents: &crate::config::AgentsConfig, role: &str) -> String { ... }
```

`providers.rs` debería contener solo el trait `AgentProvider` y sus implementaciones. La lógica de resolución de configuración (`provider_for_role`, `skill_for_role`) depende de `crate::config::AgentsConfig`, creando un acoplamiento desde `infra` hacia `config`. Estas funciones deberían residir en `config.rs` (como métodos de `AgentsConfig`) o en una capa intermedia.

### 11.3 No hay health checks ni métricas

No existe endpoint o archivo de métricas para monitorizar el pipeline en ejecución. Para las features pendientes #11 (TUI/dashboard) y #12 (cost tracking) se necesitará:

| Métrica | Definición |
|---------|-----------|
| `iterations_per_hour` | Velocidad del pipeline |
| `mean_agent_time_seconds` | Tiempo medio por invocación de agente |
| `rejection_rate` | % de transiciones que fueron rechazos |
| `stories_per_hour` | Throughput de historias completadas |
| `estimated_cost` | Coste acumulado en USD (requiere pricing de cada provider) |

### 11.4 Sin benchmarks

No hay tests de rendimiento para escenarios grandes:
- Proyecto con 100+ historias
- Historias con cadenas de dependencias profundas (10+ niveles)
- Pipelines de larga duración (>1000 iteraciones)

---

## 📊 Resumen de Impacto

| # | Problema | Severidad | Módulos afectados | Líneas est. | Bloquea |
|---|----------|-----------|-------------------|-------------|---------|
| 1 | Estado enum vs string | 🔴 Crítica | `state.rs`, `pipeline.rs`, `deadlock.rs`, `prompts.rs` | ~500 | #04 |
| 2 | I/O síncrono en hot path | 🔴 Crítica | `pipeline.rs`, `agent.rs` | ~200 | #01 |
| 3 | Checkpoint no acotado | 🟠 Alta | `checkpoint.rs`, `pipeline.rs` | ~80 | Crecimiento |
| 4 | Git snapshots globales | 🟠 Alta | `git.rs`, `pipeline.rs` | ~150 | #01 |
| 5 | PromptContext clones | 🟠 Alta | `prompts.rs`, `pipeline.rs` | ~100 | #10 |
| 6 | Daemon polling/sleep | 🟡 Media | `daemon.rs` | ~120 | #11 |
| 7 | Provider `panic` | 🟡 Media | `providers.rs` | ~20 | Robustez |
| 8 | Config sin validación épicas | 🟡 Media | `config.rs`, `plan.rs` | ~15 | Plan |
| 9 | Formato historia frágil | 🟡 Media | `story.rs` | ~80 | Integridad |
| 10 | `agent.rs` síncrono | 🟠 Alta | `agent.rs`, `pipeline.rs` | ~200 | #01 |
| 11 | HashMaps no `Send+Sync` | 🟠 Alta | `pipeline.rs` | ~120 | #01 |
| 12 | Duplicación `extract_numeric` | 🟢 Baja | 4 módulos | ~10 | Manten. |
| 13 | Sin métricas | 🟢 Baja | Nuevo módulo | ~100 | #11, #12 |
| 14 | Sin benchmarks | 🟢 Baja | `benches/` | ~80 | Confianza |

---

## 🎯 Recomendaciones Prioritarias (ordenadas por ROI)

### 🔴 Críticas — Resolver antes de iniciar #04 o #01

#### 1. `StoryCache` con invalidación por `mtime`

**Problema**: 15.000 lecturas de archivo en un pipeline típico.  
**Solución**: cachear `Story` en un `HashMap<String, CachedStory>` y re-parsear solo si `mtime` cambió.  
**Esfuerzo**: ~60 líneas en `pipeline.rs` o nuevo `story_cache.rs`.  
**ROI**: elimina el 95% de las re-lecturas. Impacto inmediato con >10 historias.

#### 2. `from_name()` → `Result`

**Problema**: `panic!` aborta el proceso si el usuario escribe mal el provider.  
**Solución**: `pub fn from_name(name: &str) -> anyhow::Result<Box<dyn AgentProvider>>`.  
**Esfuerzo**: ~5 líneas en `providers.rs` + adaptar callers (3-4 sitios).  
**ROI**: previene crashes en producción, mejora mensajes de error.

#### 3. `DependencyGraph` memoizado

**Problema**: 2-3 reconstrucciones del grafo por iteración.  
**Solución**: cachear el grafo en el orchestrator y recalcular solo cuando `apply_automatic_transitions` modifica estados.  
**Esfuerzo**: ~50 líneas en `pipeline.rs`.  
**ROI**: reduce inserciones en HashMap de 270.000 a ~3.000 en un pipeline típico (99% menos).

### 🟠 Altas — Resolver antes de #01 (paralelismo)

#### 4. Extraer `Workflow` trait

**Problema**: `next_status()`, `map_status_to_role()`, y `apply_automatic_transitions()` están hardcodeados al workflow fijo.  
**Solución**: trait `Workflow` con implementación `CanonicalWorkflow` por defecto. El orchestrator recibe `&dyn Workflow`.  
**Esfuerzo**: ~120 líneas en `state.rs`, `pipeline.rs`.  
**ROI**: desacopla la preparación para #04 sin romper nada. Permite testear workflows alternativos sin tocar el loop principal.

#### 5. `git add` selectivo

**Problema**: `git add -A` stajea TODO el repositorio.  
**Solución**: limitar a `.regista/stories/`, `.regista/decisions/`, y paths de código configurados.  
**Esfuerzo**: ~10 líneas en `git.rs` + opción de config.  
**ROI**: reduce tiempo de snapshot en 10-100x para repositorios grandes.

#### 6. Eliminar busy-polling en `agent.rs`

**Problema**: `thread::sleep(250ms)` bloquea un thread del SO.  
**Solución**: migrar `invoke_once` a `tokio::process::Command` con `tokio::time::timeout`.  
**Esfuerzo**: ~80 líneas en `agent.rs` + async propagation a `pipeline.rs`.  
**ROI**: prerequisito para paralelismo. Sin esto, #01 no puede arrancar.

#### 7. `Arc<RwLock<>>` para shared state

**Problema**: `reject_cycles` y `story_iterations` son variables locales mutables.  
**Solución**: wrappear en `Arc<RwLock<>>` y pasar a `process_story` por referencia compartida.  
**Esfuerzo**: ~120 líneas en `pipeline.rs`.  
**ROI**: prerequisito para paralelismo. Permite que múltiples agentes lean/escriban contadores concurrentemente.

### 🟡 Medias — Mejoran robustez y experiencia

#### 8. Mover `extract_numeric` a `domain`

**Problema**: código duplicado 4 veces.  
**Solución**: `pub fn extract_numeric(id: &str) -> u32` en `domain/mod.rs` o `domain/story.rs`.  
**Esfuerzo**: ~10 líneas.  
**ROI**: elimina fuente de bugs por divergencia.

#### 9. Compactación de checkpoint

**Problema**: entradas de historias terminales nunca se eliminan del checkpoint.  
**Solución**: al guardar, filtrar entradas de historias en `Done` o `Failed`.  
**Esfuerzo**: ~20 líneas en `checkpoint.rs`.  
**ROI**: checkpoint O(activas) en vez de O(todas).

#### 10. Verificación de binarios de providers en `validate`

**Problema**: el error de "binary not found" se detecta en runtime, no en validación.  
**Solución**: `validate.rs` verifica que los binarios de los providers configurados existen en PATH.  
**Esfuerzo**: ~20 líneas en `validate.rs` (usando `which` crate o similar).  
**ROI**: feedback inmediato al usuario, evita pipelines que fallan a los 10 minutos.

#### 11. Rotación de logs con `tracing-appender`

**Problema**: `daemon.log` crece sin límite.  
**Solución**: usar `tracing-appender` con `Rotation::DAILY` y cleanup de archivos viejos.  
**Esfuerzo**: ~30 líneas en `handlers.rs` (`setup_daemon_tracing`).  
**ROI**: previene que el disco se llene en pipelines largos.

### 🟢 Bajas — Nice to have

#### 12. `health.rs` — endpoint de métricas

Volcar `RunReport` parcial cada N iteraciones a `.regista/health.json`.  
**Esfuerzo**: ~100 líneas en módulo nuevo.

#### 13. Benchmarks con `criterion`

Tests de rendimiento para `DependencyGraph`, `load_all_stories`, y `deadlock::analyze` con 100/500/1000 historias.  
**Esfuerzo**: ~80 líneas en `benches/`.

#### 14. `provider_for_role` / `skill_for_role` a `config.rs`

Corregir el acoplamiento `infra → config`.  
**Esfuerzo**: ~30 líneas.

---

*Documento generado automáticamente tras auditoría de la arquitectura completa de regista v0.6.0.*
