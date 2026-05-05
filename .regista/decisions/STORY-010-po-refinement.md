# STORY-010 — Refinamiento PO (Draft → Ready)

**Fecha**: 2026-05-05  
**Actor**: Product Owner  
**Transición**: Draft → Ready

---

## Validación DoR

### 1. Descripción clara y no ambigua ✅
La historia describe con precisión la migración de busy-polling (`std::thread::sleep(250ms)`)
a `tokio::process::Command` + `tokio::time::timeout`. Menciona que el trait `AgentProvider`
ya devuelve `Vec<String>` (verificado en `src/infra/providers.rs:45`), lo que permite la
migración sin tocar providers.

### 2. Criterios de aceptación específicos y testeables ✅
8 CAs cubriendo:
- CA1-CA2: `invoke_once()` async con `tokio::process::Command` y `tokio::time::timeout`
- CA3-CA4: `invoke_with_retry()` async con `tokio::time::sleep` y backoff preservado
- CA5: `save_agent_decision()` → `tokio::fs::write` o `spawn_blocking`
- CA6: `cargo build` compila con `tokio` (hoy ausente en `Cargo.toml`)
- CA7: tests adaptados a `#[tokio::test]`
- CA8: `cargo test --lib agent` pasa

### 3. Dependencias identificadas ✅
"Ninguna". Verificado: el parseo de `## Dependencias` no extrae ningún `STORY-XXX`.

---

## Verificación contra código actual

| Afirmación en la historia | Realidad en `src/infra/agent.rs` | Coincide |
|---|---|---|
| `invoke_once()` usa `thread::sleep(250ms)` | Línea 152: `poll = Duration::from_millis(250)` + `std::thread::sleep(poll)` | ✅ |
| `invoke_with_retry()` usa `std::thread::sleep` para backoff | Línea 144: `std::thread::sleep(delay)` | ✅ |
| `save_agent_decision()` es síncrona | Usa `std::fs::write` y `std::fs::create_dir_all` | ✅ |
| `AgentProvider` devuelve `Vec<String>` | `providers.rs:45`: `fn build_args(...) -> Vec<String>` | ✅ |
| `tokio` no está en `Cargo.toml` | `Cargo.toml`: no hay entrada `tokio` | ✅ (se añadirá en CA6) |

---

## Decisión

**Transición aprobada**: STORY-010 cumple el Definition of Ready. Sin ambigüedades,
CAs testeables, sin dependencias bloqueantes. La descripción refleja fielmente el
estado actual del código.

**Nota para el Dev**: `tokio` deberá añadirse a `Cargo.toml` con features mínimos
(`process`, `time`, `rt`). El `SharedState` con `Arc<RwLock<>>` (STORY-011) ya está
en `domain/state.rs` y será necesario cuando se implemente paralelismo (#01).
