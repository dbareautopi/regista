# STORY-022: Streaming de stdout del agente en `invoke_once()` + parámetro `verbose`

## Status
**Ready**

## Epic
EPIC-08

## Descripción
Modificar `invoke_once()` en `infra/agent.rs` para que, cuando `verbose = true`, lea stdout del proceso hijo línea a línea usando `BufReader` sobre el pipe y emita cada línea al log con prefijo `  │ `. El stderr se sigue capturando en silencio (sin streaming). Cuando `verbose = false`, se usa el comportamiento actual (`wait_with_output()`, más eficiente). Añadir `verbose: bool` como parámetro a `invoke_with_retry()` y propagarlo a `invoke_once()`.

## Criterios de aceptación
- [ ] CA1: `invoke_with_retry()` acepta un nuevo parámetro `verbose: bool` como último argumento
- [ ] CA2: Cuando `verbose = true`, `invoke_once()` usa `child.stdout.take()` + `BufReader::new()` + `read_line()` en un bucle async
- [ ] CA3: Cada línea no vacía de stdout se loguea con `tracing::info!("  │ {}", trimmed)` 
- [ ] CA4: El stdout completo se acumula en un `Vec<u8>` y se devuelve como parte del resultado (igual que antes)
- [ ] CA5: stderr se lee en una tarea `tokio::spawn` separada, sin streaming al log, acumulado en `Vec<u8>`
- [ ] CA6: Cuando `verbose = false`, `invoke_once()` usa `wait_with_output()` (comportamiento actual, sin cambios)
- [ ] CA7: El timeout sigue funcionando correctamente en ambos modos (mata el proceso y devuelve error)
- [ ] CA8: `cargo check --lib` compila todo el crate sin errores
- [ ] CA9: `cargo test --lib infra::agent` pasa todos los tests existentes
- [ ] CA10: Todos los call sites existentes de `invoke_with_retry()` se actualizan con el nuevo parámetro `verbose`
- [ ] CA11: `AgentResult` (o el tipo de retorno) sigue conteniendo `stdout`, `stderr`, y `exit_code`

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 3: Streaming de stdout del agente).
- 2026-05-05 | PO | Refinamiento: validada contra DoR. Supera los 3 criterios (descripción clara, CAs testeables, dependencias identificadas). Documentadas 5 decisiones de diseño en .regista/decisions/STORY-022-po-refinement-2026-05-05.md. Transición Draft → Ready.
