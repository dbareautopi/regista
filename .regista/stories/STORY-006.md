# STORY-006: `git add` selectivo para snapshots

## Status
**Draft**

## Epic
EPIC-02

## Descripción
Actualmente `git.rs::snapshot()` ejecuta `git add -A`, que stajea todo el working tree del repositorio. En repositorios grandes (50.000+ archivos), cada `git add -A` puede tardar 0.5-2 segundos. En 300 iteraciones, eso suma 2.5-10 minutos solo en `git add`. El snapshot debe limitarse a los paths que los agentes modifican: `.regista/stories/`, `.regista/decisions/`, y los directorios de código fuente configurados en `StackConfig`.

## Criterios de aceptación
- [ ] CA1: `git.rs::snapshot()` añade paths específicos en lugar de `git add -A`:
  - `.regista/stories/`
  - `.regista/decisions/`
  - `.regista/epics/`
  - `src/` (directorio de código fuente del proyecto anfitrión)
- [ ] CA2: Si `StackConfig.src_dir` está configurado, ese directorio también se incluye en el `git add`
- [ ] CA3: El mensaje del commit sigue incluyendo el label descriptivo (ej: `"snapshot: after STORY-042 (PO→Ready)"`)
- [ ] CA4: Si alguno de los paths no existe, `git add` no falla (git ignora paths inexistentes con un warning, no error)
- [ ] CA5: `cargo test --lib git` pasa (adaptar tests existentes si verificaban `git add -A`)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #2.3, recomendación #5).
