# STORY-018: Benchmarks con `criterion` para escenarios de gran escala

## Status
**Draft**

## Epic
EPIC-06

## Descripción
No existen tests de rendimiento para escenarios grandes (100+ historias, cadenas de dependencias profundas, pipelines largos). Añadir benchmarks con [`criterion`](https://crates.io/crates/criterion) para: construcción de `DependencyGraph` con 100/500/1000 historias, `load_all_stories` con 100/500/1000 archivos, y `deadlock::analyze` con grafos complejos. Esto permite evaluar el impacto de las optimizaciones de EPIC-02 y detectar regresiones de rendimiento.

## Criterios de aceptación
- [ ] CA1: `Cargo.toml` tiene `criterion` como dev-dependency (con `[[bench]]` o `harness = false`)
- [ ] CA2: Existe `benches/graph_benchmark.rs` con benchmarks para `DependencyGraph::from_stories` con 100, 500 y 1000 historias sintéticas
- [ ] CA3: Existe `benches/story_benchmark.rs` con benchmarks para `load_all_stories` (usando `tempfile::tempdir` con N archivos .md)
- [ ] CA4: Existe `benches/deadlock_benchmark.rs` con benchmarks para `deadlock::analyze` con cadenas de dependencias de profundidad 10, 50, 100
- [ ] CA5: `cargo bench` ejecuta todos los benchmarks sin errores
- [ ] CA6: Los benchmarks usan `criterion_group!` y `criterion_main!` correctamente
- [ ] CA7: Los tests unitarios existentes no se ven afectados (`cargo test` pasa)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #11.4, recomendación #13).
