# STORY-V10-004: Retry con backoff, timeout y rate limiting en cliente LLM

## Status
**Draft**

## Epic
EPIC-V10-01

## Descripción
Implementar la lógica de reintentos, timeout y rate limiting en el cliente LLM, adaptando la lógica existente de `infra/agent.rs` (backoff exponencial + timeout de proceso) al nuevo modelo de llamadas HTTP.

A diferencia de v0.x donde se reintentaba matando y relanzando procesos, aquí se reintenta la llamada HTTP con delay creciente. El timeout ahora es sobre la request HTTP (no sobre un proceso), y se añade rate limiting para respetar los límites de las APIs.

**Valor de negocio**: Resiliencia ante fallos transitorios de red, respeto de rate limits de las APIs, y control de tiempo máximo de ejecución por fase.

## Criterios de aceptación
- [ ] CA1: `invoke_with_retry()` implementa backoff exponencial: delay inicial configurable (`retry_delay_base_seconds`), se duplica en cada reintento (`delay *= 2`), con cota superior fija en 300s, y máximo de reintentos = 5
- [ ] CA2: Timeout por request configurable por fase (`timeout_seconds` en `PhaseConfig`). Si se agota, se aborta la request HTTP y se trata como fallo reintentable (salvo si es el último intento)
- [ ] CA3: Rate limiting: delay mínimo configurable entre requests consecutivas al mismo provider. Si se recibe HTTP 429 con `retry-after`, se respeta ese valor en lugar del backoff estándar

## Dependencias
- Bloqueado por: STORY-V10-001

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 1
