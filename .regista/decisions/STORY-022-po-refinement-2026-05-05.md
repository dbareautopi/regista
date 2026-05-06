# STORY-022 — Refinamiento PO — 2026-05-05

## Resultado
✅ **Aprobada** — Transición Draft → Ready

## Definition of Ready — evaluación

| Criterio | Estado | Comentario |
|----------|--------|------------|
| Descripción clara y no ambigua | ✅ | Especifica exactamente qué modificar (`invoke_once`), cuándo (`verbose=true/false`), y cómo (`BufReader`, prefijo `│`) |
| Criterios de aceptación específicos y testeables | ✅ | 11 CAs concretos: firma (CA1), implementación (CA2-CA6), timeout (CA7), compilación (CA8), tests (CA9), call sites (CA10), retorno (CA11) |
| Dependencias identificadas | ✅ | Marcada como "Ninguna" — correcto. `invoke_once` es función interna sin dependencias entre historias |

## Decisiones de diseño documentadas

### D1: `BufReader` asíncrono (tokio)
El CA2 menciona `BufReader::new()` + `read_line()` en bucle async. Dado que `child.stdout.take()` devuelve `tokio::process::ChildStdout` (que implementa `AsyncRead`), la implementación debe usar `tokio::io::BufReader` + `tokio::io::AsyncBufReadExt::read_line()`, **no** `std::io::BufReader` (que es bloqueante y bloquearía el runtime).

### D2: Propagación a `invoke_with_retry_blocking`
El wrapper síncrono `invoke_with_retry_blocking()` (línea 184 de `agent.rs`) debe aceptar y propagar el parámetro `verbose`, ya que internamente llama a `invoke_with_retry()`. Esto cubre el call site de `plan.rs:152`.

### D3: Call sites a actualizar (CA10)
- `src/app/pipeline.rs:774` — `invoke_with_retry(provider, instruction, &prompt, &limits, agent_opts).await` → añadir `verbose`
- `src/app/plan.rs:152` — `invoke_with_retry_blocking(provider, &skill_path, &prompt, &cfg.limits, &AgentOptions::default())` → añadir `verbose`
- `src/infra/agent.rs` (tests) — todas las llamadas a `invoke_with_retry(...)` en los tests async

### D4: Timeout en modo verbose (CA7)
En modo `verbose=true`, ya no se puede usar `child.wait_with_output()` porque los pipes stdout/stderr se han tomado con `.take()`. La estrategia correcta es:
1. `tokio::spawn` para stderr (acumulación silenciosa)
2. Leer stdout línea a línea en la tarea principal
3. `tokio::time::timeout` envolviendo `child.wait()` para obtener el exit status
4. Esperar la tarea de stderr con `.await`
5. Construir `std::process::Output` manualmente desde el exit status + buffers acumulados

### D5: Alcance limitado a sección 3 del spec
Esta historia implementa exclusivamente el streaming de stdout (sección 3 de `spec-logs-transparentes.md`). No incluye: diff de archivos (sección 4), `regista logs` con historial (sección 5), tracking de tokens (sección 6), ni resolución de modelo (sección 7). Esas secciones se implementarán en historias separadas.

## Scope verification
- Épica: EPIC-08
- Spec origen: `specs/spec-logs-transparentes.md` (sección 3: Streaming de stdout del agente)
- Archivo principal afectado: `src/infra/agent.rs`
- Sin conflictos con otras historias activas
