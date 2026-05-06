# STORY-028: Incluir conteo de tokens en `HealthReport`

## Status
**Ready**

## Epic
EPIC-10

## Descripción
Extender `HealthReport` en `app/health.rs` para incluir el total acumulado de tokens de entrada y salida. El reporte se genera periódicamente (cada N iteraciones, vía health checkpoint) y al final del pipeline. Los datos de tokens se leen de `SharedState.token_usage`, sumando todos los `TokenCount` de todas las historias.

## Criterios de aceptación
- [ ] CA1: `HealthReport` tiene campos `pub total_input_tokens: u64` y `pub total_output_tokens: u64`
- [ ] CA2: `HealthReport` tiene campo `pub total_tokens: u64` (calculado como `input + output`)
- [ ] CA3: `generate_report()` (o la función equivalente) acepta una referencia a `SharedState` y lee `token_usage` para calcular los totales
- [ ] CA4: Los campos nuevos se incluyen en la serialización JSON de `HealthReport`
- [ ] CA5: `write_health_json()` escribe el JSON incluyendo los campos de tokens
- [ ] CA6: Si no hay datos de tokens (pipeline recién iniciado), los campos son `0`
- [ ] CA7: `cargo build` compila sin errores
- [ ] CA8: `cargo test --lib app::health` pasa todos los tests existentes y nuevos
- [ ] CA9: Test unitario verifica que `HealthReport` serializa correctamente los campos de tokens a JSON

## Dependencias
- Bloqueado por: STORY-027

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 6: Tracking de tokens — Health report).