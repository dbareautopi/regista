# STORY-023: `follow()` con parámetro `from_beginning` para volcado de historial completo

## Status
**Draft**

## Epic
EPIC-08

## Descripción
Modificar `daemon::follow()` en `infra/daemon.rs` para que acepte un parámetro `from_beginning: bool`. Cuando es `true`, no se hace `seek(SeekFrom::End(0))` — en su lugar, se lee el archivo de log desde el byte 0, volcando todo el contenido existente, y luego se sigue en modo tail. Cuando es `false`, comportamiento actual (solo contenido nuevo). El mensaje de finalización debe incluir el PID del daemon.

## Criterios de aceptación
- [ ] CA1: `daemon::follow()` tiene firma `pub fn follow(project_dir: &Path, from_beginning: bool) -> anyhow::Result<()>` 
- [ ] CA2: Cuando `from_beginning = true`, se abre el log y se lee desde offset 0 (sin `seek(SeekFrom::End(0))`)
- [ ] CA3: Cuando `from_beginning = false`, se hace `seek(SeekFrom::End(0))` (comportamiento actual)
- [ ] CA4: En ambos modos, tras volcar el contenido inicial, se sigue leyendo líneas nuevas según llegan (modo `tail -f`)
- [ ] CA5: Cuando el daemon termina, se muestra `── Daemon terminado (PID: X) ──` y la función retorna
- [ ] CA6: `cargo check --lib` compila sin errores en `infra::daemon`
- [ ] CA7: `cargo test --lib infra::daemon` pasa todos los tests existentes
- [ ] CA8: Todos los call sites existentes de `follow()` se actualizan pasando `false` como `from_beginning`

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-05 | PO | Historia generada desde specs/spec-logs-transparentes.md (sección 5: regista logs — historial completo).
