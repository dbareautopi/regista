# EPIC-06: Métricas, Benchmarks y Limpieza Técnica

## Descripción
Centralizar la función `extract_numeric` (duplicada en 4 módulos) en `domain`, implementar un módulo `health.rs` que exponga métricas del pipeline en ejecución (velocidad, throughput, rechazos, coste estimado), y añadir benchmarks con `criterion` para escenarios de gran escala.

Cubre los hallazgos #5, #11.1, #11.3 y #11.4 de la auditoría.

## Historias
- STORY-016: Centralizar `extract_numeric` + eliminar duplicación
- STORY-017: Módulo `health.rs` con endpoint de métricas
- STORY-018: Benchmarks con `criterion` para escenarios grandes
