# STORY-015: Robustecimiento de `set_status()` + versión del formato de historia

## Status
**Draft**

## Epic
EPIC-05

## Descripción
El método `set_status()` en `domain/story.rs` usa reemplazo posicional frágil (busca `## Status` y modifica la línea siguiente por índice). Si un agente modifica accidentalmente el formato, la escritura puede fallar o corromper el archivo. Además, `advance_status_in_memory()` usa `replacen` que puede colisionar con ocurrencias del string de estado en el Activity Log. Hay que robustecer ambos y añadir un campo de versión del formato (`## Format-Version: 1`) para permitir evolución futura.

## Criterios de aceptación
- [ ] CA1: `set_status()` usa regex o búsqueda más robusta (ej: `Regex::new(r"(?im)^## Status\s*\n\s*\*\*.*?\*\*")`) en lugar de navegación por índice de líneas
- [ ] CA2: `advance_status_in_memory()` busca específicamente la línea bajo `## Status` en lugar de usar `replacen` sobre todo `raw_content`
- [ ] CA3: El parser de historias (`Story::load`) reconoce un campo opcional `## Format-Version` (si no existe, asume versión 1)
- [ ] CA4: Las historias generadas por `init` y `plan` incluyen `## Format-Version: 1` automáticamente
- [ ] CA5: Si `## Format-Version` tiene un valor > 1, el parser emite un warning (no error) indicando que la versión puede no estar soportada
- [ ] CA6: `cargo test --lib story` pasa (tests existentes + nuevos tests para set_status robusto y format version)
- [ ] CA7: El backup atómico y verificación post-escritura en `set_status()` se mantienen

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #9.1, #9.2, #9.3).
