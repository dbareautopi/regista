# STORY-017: Módulo `health.rs` con endpoint de métricas del pipeline

## Status
**Tests Ready**

## Epic
EPIC-06

## Descripción
No existe forma de monitorizar el pipeline en ejecución. Para las features pendientes #11 (TUI/dashboard) y #12 (cost tracking), se necesita un sistema de métricas. Implementar un módulo `health.rs` (en `src/app/`) que mantenga y exponga métricas clave: iteraciones por hora, tiempo medio por agente, tasa de rechazo, throughput de historias, y coste estimado. Las métricas se vuelcan a `.regista/health.json` cada N iteraciones.

## Criterios de aceptación
- [ ] CA1: Existe `src/app/health.rs` con un struct `HealthReport` que contiene:
  - `iterations_per_hour: f64`
  - `mean_agent_time_seconds: f64`
  - `rejection_rate: f64` (transiciones rechazadas / total)
  - `stories_per_hour: f64`
  - `estimated_cost_usd: f64`
  - `current_iteration: u32`
  - `stories_done: u32`
  - `stories_failed: u32`
  - `stories_active: u32`
  - `elapsed_wall_time_seconds: u64`
- [ ] CA2: Existe una función `pub fn generate_report(...) -> HealthReport` que calcula las métricas a partir del estado actual del orchestrator
- [ ] CA3: `HealthReport` implementa `Serialize` y se escribe a `.regista/health.json` cada 10 iteraciones (configurable)
- [ ] CA4: La escritura de `health.json` es atómica (escribir a `.tmp`, renombrar)
- [ ] CA5: Si el pipeline termina (PipelineComplete), se escribe un health report final
- [ ] CA6: `cargo test --lib health` pasa (módulo nuevo con tests)
- [ ] CA7: `cargo build` compila sin warnings

## Dependencias
- Bloqueado por: STORY-011

## Activity Log
- 2026-05-05 | QA | Tests escritos para todos los CAs (27 tests). Cobertura: CA1 (HealthReport struct y tipos, 4 tests), CA2 (generate_report con edge cases, 8 tests), CA3 (Serialize + is_health_checkpoint intervalos, 8 tests), CA4 (escritura atómica, 5 tests), CA5 (reporte final PipelineComplete, 3 tests). Módulo registrado en src/app/mod.rs. Compilación verificada con cargo check (solo warnings dead_code).