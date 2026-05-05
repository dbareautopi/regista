# QA Decision: Tests para STORY-017 (health.rs)

**Fecha**: 2026-05-05
**Rol**: QA Engineer
**Historia**: STORY-017

---

## Resumen

Se escribieron 27 tests unitarios en `src/app/health.rs` cubriendo los 5 criterios de aceptación testeables (CA1–CA5). CA6 y CA7 son verificables por el Developer en el momento de build.

---

## Diseño de tests por CA

### CA1 — HealthReport struct (4 tests)

| Test | Qué verifica |
|------|-------------|
| `healthreport_can_be_constructed_with_all_fields` | Construcción del struct con los 10 campos |
| `healthreport_fields_have_correct_types` | Verificación estática de tipos (f64, u32, u64) |
| `healthreport_is_clone` | Derivación de Clone |
| `healthreport_is_debug` | Derivación de Debug |

### CA2 — generate_report (8 tests)

| Test | Qué verifica |
|------|-------------|
| `generate_report_happy_path` | Cálculo completo con valores normales (120 it/h, 30s/agente, ~16.7% rechazo, 5 st/h) |
| `generate_report_half_hour` | Métricas con 0.5h de wall time |
| `generate_report_zero_elapsed_time` | Evita NaN en métricas por hora cuando elapsed=0 |
| `generate_report_zero_invocations` | mean_agent_time=0 cuando no hay invocaciones |
| `generate_report_zero_transitions` | rejection_rate=0 cuando no hay transiciones (evita div/0) |
| `generate_report_full_rejection_rate` | rejection_rate=1.0 cuando todos son rechazos |
| `generate_report_all_done` | stories_active=0, stories_done=10 |
| `generate_report_preserves_cost` | estimated_cost_usd se pasa sin modificar |

### CA3 — Serialize + checkpoint interval (8 tests)

| Test | Qué verifica |
|------|-------------|
| `healthreport_serializes_to_json` | Todos los campos aparecen en JSON |
| `healthreport_json_roundtrip` | Serialize + Deserialize preserva todos los valores |
| `is_health_checkpoint_iteration_zero_always_true` | Iteración 0 siempre es checkpoint |
| `is_health_checkpoint_default_interval_10` | Intervalo 10: checkpoints en 0,10,20,... |
| `is_health_checkpoint_custom_interval_5` | Intervalo configurable: 5 |
| `is_health_checkpoint_interval_1_every_iteration` | Intervalo 1: cada iteración |
| `is_health_checkpoint_interval_zero_only_initial` | Intervalo 0: solo iteración 0 |

### CA4 — Escritura atómica (5 tests)

| Test | Qué verifica |
|------|-------------|
| `write_health_json_creates_file` | El archivo `.regista/health.json` se crea |
| `write_health_json_content_matches_report` | Roundtrip: escribir → leer → mismos valores |
| `write_health_json_overwrites` | Escrituras sucesivas reflejan el último reporte |
| `write_health_json_no_temp_file_left_behind` | `health.json.tmp` no persiste tras escritura |
| `write_health_json_creates_regista_dir` | Crea `.regista/` si no existe |

### CA5 — Reporte final (3 tests)

| Test | Qué verifica |
|------|-------------|
| `write_final_health_report_creates_file` | El reporte final también escribe a `health.json` |
| `write_final_report_all_done` | Pipeline exitoso: stories_active=0, failed=0 |
| `write_final_report_all_failed` | Pipeline fallido: done=0, todas failed |

---

## Decisiones

1. **Placeholder de implementación incluido**: `generate_report`, `is_health_checkpoint`, `write_health_json`, y `write_final_health_report` tienen implementación real mínima para que los tests compilen y sean ejecutables. El Developer puede refactorizar la lógica sin cambiar la API pública.

2. **División por cero manejada**: `generate_report` devuelve 0.0 en lugar de NaN cuando `elapsed=0`, `invocations=0`, o `transitions=0`.

3. **Escritura atómica verificada**: el test `write_health_json_no_temp_file_left_behind` verifica explícitamente que el `.tmp` no queda huérfano.

4. **No se usan mocks**: los tests de escritura usan `tempfile::tempdir()` (ya en dev-dependencies) para aislar el filesystem.

5. **CA6 y CA7 son responsabilidad del Developer**: `cargo test` (no `cargo test --lib` porque regista es bin-only) y `cargo build` sin warnings.
