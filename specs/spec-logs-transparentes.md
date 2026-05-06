# Logs transparentes — streaming, trazabilidad de archivos y tracking de tokens

> **Fecha**: 2026-05-05
> **Estado**: ✍️ Especificación
> **Esfuerzo**: Alto
> **Dependencias**: 01-cli-subcomandos-daemon (ya implementado)

---

## 🎯 Objetivo

Refactorizar el sistema de logging para que el modo **detallado** (verbose) sea el predeterminado, proporcionando:

1. **Transparencia de archivos**: tras cada agente se muestra qué archivos fueron creados, modificados o eliminados.
2. **Streaming en tiempo real**: la salida del agente se vuelca línea a línea al log para que el desarrollador vea qué está haciendo en cada momento (adiós al "parece colgado").
3. **Metadatos de sesión**: al iniciar se muestra versión, timestamp, provider + modelo por rol, límites, git, hooks.
4. **`regista logs` con historial completo**: vuelca todo el contenido existente y luego sigue en vivo.
5. **Tracking de tokens**: se parsea el consumo de tokens de cada agente, se acumula, y se muestra en el resumen final.

---

## ❓ Problema actual

| Problema | Detalle |
|----------|---------|
| **Logs espartanos** | Solo se muestra `🎯 QA (tests) \| STORY-001 (Ready → Tests Ready)`. Cero info sobre qué archivos se tocaron, cuánto tardó realmente el agente, qué modelo se usó. |
| **Agente "colgado"** | Entre la línea de "invocando" y el resultado pueden pasar minutos sin una sola línea. El desarrollador no sabe si el agente está trabajando, bloqueado, o muerto. |
| **`regista logs` solo muestra lo nuevo** | `follow()` hace `seek(SeekFrom::End(0))`. Si el desarrollador llega tarde, no ve lo que ya pasó. |
| **Sin metadatos de sesión** | No se sabe con qué versión de regista se lanzó, qué modelo se usó, ni qué límites aplican. Para depurar un pipeline fallido hay que reconstruir el contexto manualmente. |
| **Sin tracking de tokens** | Imposible saber cuánto costó un pipeline. El usuario no tiene visibilidad del gasto. |

---

## ✅ Solución propuesta

### 1. Dos niveles de verbosidad

| Modo | Flag | Comportamiento |
|------|------|----------------|
| **Detallado** (default) | *(sin flag)* | Header de sesión + streaming de agente + diff de archivos + hooks (solo errores) + resumen con tokens |
| **Compacto** | `--compact` | Header reducido + agente + transición + errores (comportamiento actual de `info`) |

`--compact` reemplaza conceptualmente al actual `--quiet`. `--quiet` se mantiene pero solo suprime los logs de progreso del loop (iteraciones). `--compact` va más allá: suprime streaming, diffs, y detalles de hooks.

El flag `--compact` se añade a `CommonArgs` y afecta a `plan`, `auto`, `run`.

### 2. Header de sesión

Al iniciar el daemon (en `setup_daemon_tracing()` o inmediatamente después), se emite **siempre**, incluso en modo compacto:

```
══════════════════════════════════════════════════════════════
🛰️  regista v0.9.0 — sesión iniciada 2026-05-05 14:32:07 UTC
   Proyecto   : /home/user/mi-app
   Provider    : pi
   Modelos     : PO=qwen2.5-coder, QA=claude-sonnet-4, Dev=qwen2.5-coder, Reviewer=claude-sonnet-4
   Límites     : max_iter=252 (42 stories × 6), max_reject=8, timeout=1800s
   Git         : habilitado
   Hooks       : post_qa, post_dev, post_reviewer
══════════════════════════════════════════════════════════════
```

En modo compacto, el header se reduce a una línea:

```
🛰️  regista v0.9.0 | pi | 2026-05-05 14:32:07 UTC | max_iter=252
```

### 3. Streaming de stdout del agente en tiempo real

Se modifica `invoke_once()` en `infra/agent.rs` para que, en lugar de capturar toda la salida con `wait_with_output()`, lea stdout **línea a línea** usando `BufReader` sobre el pipe del proceso hijo, y emita cada línea al log con un prefijo `│ `.

#### Antes (comportamiento actual)

```rust
// agent.rs — invoke_once
let output = tokio::time::timeout(timeout_dur, child.wait_with_output()).await???;
```

#### Después (nuevo comportamiento)

```rust
// agent.rs — invoke_once
let stdout = child.stdout.take().unwrap();
let stderr = child.stderr.take().unwrap();

let start = Instant::now();

// Leer stdout línea a línea
let stdout_handle = tokio::spawn(async move {
    let mut reader = BufReader::new(stdout);
    let mut buf = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim_end();
                if !trimmed.is_empty() {
                    tracing::info!("  │ {}", trimmed);
                }
                buf.extend_from_slice(line.as_bytes());
            }
            Err(_) => break,
        }
    }
    buf
});

// Leer stderr completo (sin streaming al log, solo captura)
let stderr_handle = tokio::spawn(async move {
    let mut reader = BufReader::new(stderr);
    let mut buf = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => buf.extend_from_slice(line.as_bytes()),
            Err(_) => break,
        }
    }
    buf
});

let status = tokio::time::timeout(
    Duration::from_secs(timeout_secs),
    child.wait(),
).await;

// Si timeout, matar el proceso
let exit_status = match status {
    Ok(Ok(s)) => s,
    _ => {
        child.kill().await.ok();
        anyhow::bail!("Timeout tras {timeout_secs}s");
    }
};

let stdout_data = stdout_handle.await?;
let stderr_data = stderr_handle.await?;
```

#### Experiencia en el log (modo detallado)

```
  🎯 Dev (implement) | STORY-003 | pi [qwen2.5-coder]
  │ Analizando la historia STORY-003 — Sistema de autenticación...
  │ Veo que necesito crear src/auth/login.ts
  │ La historia depende de STORY-001 (Done), usaré la interfaz definida allí.
  │ Creando src/auth/login.ts...
  │ Escribiendo la función `authenticateUser(credentials: Credentials): Promise<Token>`
  │ Creando tests en src/auth/login.test.ts...
  │ Ejecutando npm test -- --grep login...
  │ ✓ should return a token for valid credentials
  │ ✓ should throw for invalid credentials
  │ ✓ should handle network errors gracefully
  │ Tests pasando: 3/3. Marco la historia como In Review.
  ✅ Completado en 42.3s (intento 1/5)
```

En modo compacto, **no hay streaming**. Solo se muestra la línea `🎯` y el `✅ Completado`.

#### Contrato de `invoke_with_retry`

La firma se amplía con un parámetro `verbose: bool`:

```rust
pub async fn invoke_with_retry(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
    verbose: bool,   // ← nuevo
) -> anyhow::Result<AgentResult>
```

Cuando `verbose = false`, `invoke_once` usa `wait_with_output()` (comportamiento actual, más eficiente). Cuando `verbose = true`, usa el streaming línea a línea.

### 4. Diff de archivos tras cada agente

Tras cada `process_story()` exitosa, si `git.enabled = true` y el modo es detallado, se ejecuta `git diff --stat HEAD` y se muestran los archivos modificados:

```
  📁 Archivos modificados:
    M  src/auth/login.ts        |  23 ++++-
    A  src/auth/login.test.ts   |  87 ++++++++++
    M  src/auth/index.ts        |   1 +
```

La implementación:
1. Ya existe `git::snapshot()` antes de cada agente (devuelve el hash del commit anterior).
2. Tras el agente, ejecutar `git diff --stat <hash> HEAD` (si no hubo snapshot, `git diff --stat HEAD~1 HEAD`).
3. Parsear la salida y loguearla con `tracing::info!`.

En modo compacto, **no se muestra el diff**.

Si `git.enabled = false`, este bloque se omite (no hay forma fiable de saber qué archivos cambió el agente).

### 5. `regista logs` — historial completo + tail

#### Comportamiento por defecto (nuevo)

```
regista logs [DIR]
```

1. Abre el archivo de log del daemon.
2. Vuelca **todo** el contenido existente a stdout.
3. Entra en modo `tail -f`: muestra líneas nuevas según llegan.
4. Si el daemon termina, se muestra `── Daemon terminado (PID: X) ──` y se sale.

#### Comportamiento con `--tail`

```
regista logs --tail [DIR]
```

Comportamiento actual: `seek(SeekFrom::End(0))` + tail desde el final (solo contenido nuevo).

#### Detalle de implementación

Se modifica `daemon::follow()` para aceptar un parámetro `from_beginning: bool`:

```rust
pub fn follow(project_dir: &Path, from_beginning: bool) -> anyhow::Result<()>
```

Si `from_beginning = true`, no se hace `seek(SeekFrom::End(0))`; se lee desde el byte 0.

#### Cambio en CLI args

- `RepoArgs` gana un flag `--tail` (bool, default false).
- `handle_logs()` lo propaga a `daemon::follow()`.

### 6. Tracking de tokens

#### Acumulación en `SharedState`

Se añade un nuevo campo a `SharedState`:

```rust
use std::collections::HashMap;

pub struct TokenCount {
    pub input: u64,
    pub output: u64,
}

// En SharedState
pub token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>,
```

Cada entrada es el ID de la historia → vector de conteos (uno por cada invocación de agente sobre esa historia, incluyendo reintentos).

#### Parseo de tokens

Tras cada `invoke_with_retry()`, se examina el `AgentResult.stdout` + `AgentResult.stderr` en busca de patrones conocidos. Se prueban en orden:

| Provider | Patrón |
|----------|--------|
| pi | `Tokens used: (\d+) input, (\d+) output` |
| pi (alt) | `(\d+) input tokens.*(\d+) output tokens` |
| Claude Code | `Token usage: (\d+) input, (\d+) output` |
| Claude Code (alt) | `Input tokens: (\d+).*Output tokens: (\d+)` |
| Codex | `Tokens: (\d+) in / (\d+) out` |
| OpenCode | `(\d+) prompt tokens.*(\d+) completion tokens` |

El parseo se implementa como una función en `infra/agent.rs`:

```rust
pub fn parse_token_count(text: &str) -> Option<TokenCount>
```

Se usa `regex::Regex` con `LazyLock` para compilar los patrones una sola vez.

#### Registro en el log (modo detallado)

Tras cada agente, se añade una línea con el consumo:

```
  ✅ Completado en 42.3s (intento 1/5) · Tokens: 1,234 → 567
```

#### Resumen final

Al terminar el pipeline, en el bloque de cierre:

```
══════════════════════════════════════════════════════════════
🏁 Pipeline completado — 2026-05-05 15:48:22 UTC
   Total        : 12
   ✅ Done      : 10
   ❌ Failed    : 2 (STORY-007, STORY-011)
   🔒 Blocked   : 0
   📝 Draft     : 0
   🔄 Iteraciones: 48
   ⏱️  Tiempo total: 1h 16m 15s
   📊 Tokens totales: 45,230 input + 12,450 output = 57,680
══════════════════════════════════════════════════════════════
```

### 7. Resolución del modelo

#### Fuentes (en orden de prioridad)

1. `config.toml` → `[agents.<rol>] model = "..."`  (por rol)
2. `config.toml` → `[agents] model = "..."`         (global)
3. YAML frontmatter del archivo de skill (`.pi/skills/<rol>/SKILL.md` → `model: ...`)
4. `"desconocido"`

#### Cambios en `config.rs`

```rust
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AgentsConfig {
    #[serde(default = "default_provider")]
    pub provider: String,

    // Nuevo campo
    #[serde(default)]
    pub model: Option<String>,

    #[serde(default)]
    pub product_owner: Option<AgentRoleConfig>,
    #[serde(default)]
    pub qa_engineer: Option<AgentRoleConfig>,
    #[serde(default)]
    pub developer: Option<AgentRoleConfig>,
    #[serde(default)]
    pub reviewer: Option<AgentRoleConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AgentRoleConfig {
    pub provider: Option<String>,
    pub skill: Option<String>,

    // Nuevo campo
    pub model: Option<String>,
}
```

#### Función de resolución

```rust
impl AgentsConfig {
    /// Resuelve el modelo para un rol: config rol > config global > YAML > "desconocido".
    pub fn model_for_role(&self, role: &str, skill_path: &Path) -> String {
        // 1. Intentar desde config por rol
        if let Some(cfg) = self.role_config(role) {
            if let Some(ref m) = cfg.model {
                return m.clone();
            }
        }
        // 2. Global
        if let Some(ref m) = self.model {
            return m.clone();
        }
        // 3. YAML frontmatter
        if skill_path.exists() {
            if let Some(m) = providers::read_yaml_field(skill_path, "model") {
                return m;
            }
        }
        // 4. Fallback
        "desconocido".to_string()
    }
}
```

#### Visualización en el log

- Header de sesión: `Modelos: PO=sonnet-4, QA=qwen2.5-coder, Dev=qwen2.5-coder, Reviewer=sonnet-4`
- Cada línea de agente: `🎯 Dev (implement) | STORY-003 | pi [qwen2.5-coder]`

### 8. Hooks: solo mostrar si fallan

Actualmente los hooks se ejecutan con `run_hook()` y solo se loguea si hay error. Esto **ya es el comportamiento deseado** — no hay cambios necesarios en la ejecución de hooks.

En modo detallado, cuando un hook falla, se muestra su stderr (truncado a 500 caracteres):

```
  🔧 Hook post_dev falló (exit code: 1):
  │ error[E0308]: mismatched types
  │   → src/auth/login.ts:42:15
  │   expected `Promise<Token>`, found `Promise<User>`
  ⚠️  Ejecutando rollback al commit a1b2c3d...
  ✅ Rollback completado.
```

---

## 📁 Módulos afectados

| Módulo | Cambios |
|--------|---------|
| `cli/args.rs` | Añadir `--compact` a `CommonArgs`. Añadir `--tail` a `RepoArgs`. |
| `cli/handlers.rs` | Propagar `--compact` al pipeline. Emitir header de sesión en `setup_daemon_tracing()`. `handle_logs()` propaga `--tail` a `follow()`. |
| `config.rs` | Añadir `model: Option<String>` a `AgentsConfig` y `AgentRoleConfig`. Añadir `model_for_role()`. |
| `domain/state.rs` | Añadir `TokenCount` struct y `token_usage: Arc<RwLock<HashMap<String, Vec<TokenCount>>>>` a `SharedState`. |
| `infra/agent.rs` | Streaming de stdout en `invoke_once()` controlado por `verbose: bool`. Añadir `parse_token_count()`. |
| `infra/daemon.rs` | `follow()` acepta `from_beginning: bool`. Pasar `--tail` desde CLI. |
| `infra/providers.rs` | Reutilizar `read_yaml_field()` existente para leer `model` del YAML. |
| `app/pipeline.rs` | Resolver modelo por rol con `AgentsConfig::model_for_role()` y pasarlo al log. Ejecutar `git diff --stat` post-agente. Parsear y acumular tokens. Emitir resumen final enriquecido con tokens. Pasar `verbose` a `invoke_with_retry()`. |
| `app/health.rs` | Incluir token totals en `HealthReport` si se dispara un health checkpoint. |

---

## 🧪 Estrategia de testing

| Test | Qué verifica |
|------|-------------|
| `parse_token_count_pi_format` | Reconoce `Tokens used: 1234 input, 567 output` |
| `parse_token_count_claude_format` | Reconoce `Token usage: 500 input, 200 output` |
| `parse_token_count_no_match` | Devuelve `None` para texto sin tokens |
| `parse_token_count_handles_commas` | `1,234` se parsea como `1234` |
| `model_resolution_role_overrides_global` | `AgentRoleConfig.model` pisa a `AgentsConfig.model` |
| `model_resolution_falls_back_to_yaml` | Si no hay config, lee del YAML |
| `model_resolution_unknown_when_nothing_set` | Sin config ni YAML → `"desconocido"` |
| `follow_from_beginning_does_not_seek` | Verificar que `from_beginning=true` no llama a `seek(End)` |
| `shared_state_token_accumulation` | Varias llamadas acumulan conteos en `Vec<TokenCount>` |
| `header_contains_model_per_role` | Verificar formato del header con modelos |
| `compact_flag_suppresses_streaming_and_diff` | Con `--compact`, no hay `│` ni `📁` en logs |

---

## ⚙️ Configuración resultante (`.regista/config.toml`)

```toml
[agents]
provider = "pi"
model = "qwen2.5-coder"                # ← NUEVO: modelo global por defecto

[agents.product_owner]
# provider = "claude"                  # hereda "pi" del global
# model = "claude-sonnet-4"            # ← NUEVO: opcional, pisa al global
# skill = ".pi/skills/po-custom/SKILL.md"

[agents.qa_engineer]
# Sin configuración → hereda provider="pi" y model="qwen2.5-coder"
```

---

## 🔢 Orden de implementación

1. **`config.rs`**: añadir `model` a `AgentsConfig` + `AgentRoleConfig` + `model_for_role()`.
2. **`domain/state.rs`**: añadir `TokenCount` + `token_usage` a `SharedState`.
3. **`infra/agent.rs`**: streaming en `invoke_once()` + `parse_token_count()`.
4. **`infra/daemon.rs`**: `follow()` con `from_beginning`.
5. **`cli/args.rs` + `cli/handlers.rs`**: `--compact`, `--tail`, header de sesión.
6. **`app/pipeline.rs`**: integrar todo — modelo en logs, diff post-agente, token accumulation, resumen final.
7. **`app/health.rs`**: incluir tokens en health reports.
8. **Tests**: unitarios para parseo, resolución, acumulación; integración para streaming y diffs.
