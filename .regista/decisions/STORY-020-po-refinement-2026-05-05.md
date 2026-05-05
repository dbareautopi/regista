# Decisión de Producto — STORY-020 (Refinamiento Draft → Ready)

**Fecha**: 2026-05-05
**Actor**: PO (Product Owner)
**Transición**: Draft → Ready

## Evaluación del Definition of Ready

| Criterio | Estado | Detalle |
|----------|--------|---------|
| Descripción clara y no ambigua | ✅ OK | Define exactamente qué añadir: struct `TokenCount` + campo `token_usage` en `SharedState`. Referencia explícita a `domain/state.rs`. |
| Criterios de aceptación específicos y testeables | ✅ OK | 9 CAs concreto: existencia del struct (CA1), traits (CA2), campo en SharedState (CA3), inicialización en new() (CA4), Clone (CA5), compilación (CA6), tests existentes (CA7), test concurrente (CA8), test de clone compartido (CA9). |
| Dependencias identificadas | ✅ OK | Ninguna — explícitamente marcado. |

## Verificaciones adicionales

- **Alineación con spec**: Coincide con `specs/spec-logs-transparentes.md` sección 6 («Tracking de tokens — Acumulación»). El struct `TokenCount` y el campo `token_usage` son idénticos a los definidos en el spec.
- **Scope acotado**: La historia solo añade la estructura de datos. No incluye el parseo (`parse_token_count`) ni la acumulación en el pipeline — eso será otra historia.
- **Compatibilidad**: Añadir un cuarto campo a `SharedState` no rompe la API existente. `SharedState::new()` necesitará un nuevo parámetro, pero eso es parte de la implementación. Las historias existentes (STORY-011) ya usan `Clone` + `Arc<RwLock<>>`, así que el patrón está probado.
- **No hay conflictos con otras historias**: STORY-020 solo toca `domain/state.rs`. No depende de ni bloquea a otras historias.

## Decisión

**La historia cumple el Definition of Ready. Se aprueba la transición Draft → Ready.**

El siguiente paso natural será QA para escribir tests que verifiquen cada CA antes de que Dev los implemente.
