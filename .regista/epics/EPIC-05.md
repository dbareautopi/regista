# EPIC-05: Estado Persistente, Logs y Daemon Robustos

## Descripción
Compactar el checkpoint eliminando entradas de historias terminales, añadir rotación de logs con `tracing-appender` para evitar que `daemon.log` crezca sin límite, eliminar la condición de carrera en `kill()` del daemon, y robustecer el parseo/escritura del formato de historia (versionado, `set_status` más robusto).

Cubre los hallazgos #3, #6 (parcial), y #9 de la auditoría.

## Historias
- STORY-013: Compactación de checkpoint (filtrar historias Done/Failed)
- STORY-014: Rotación de logs + mejorar `kill()` del daemon
- STORY-015: Robustecimiento de `set_status()` + versión del formato de historia
