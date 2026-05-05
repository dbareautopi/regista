# STORY-014: Rotación de logs con `tracing-appender` + mejorar `kill()` del daemon

## Status
**Draft**

## Epic
EPIC-05

## Descripción
Dos mejoras independientes en el daemon: (1) El archivo `daemon.log` crece sin límite (50-100MB en pipelines de 8 horas). Hay que configurar `tracing-appender` con rotación diaria y cleanup de archivos viejos. (2) La función `kill()` usa `sleep(2s)` fijo entre SIGTERM y SIGKILL, creando una condición de carrera donde SIGKILL podría enviarse a un PID reutilizado. Hay que reemplazar el sleep fijo por un loop de verificación con timeout máximo.

## Criterios de aceptación
- [ ] CA1: `daemon.log` se configura con `tracing_appender::rolling::daily` y retención de N archivos (máx 7 días)
- [ ] CA2: Los logs antiguos se nombran `daemon.log.YYYY-MM-DD` y se eliminan los de más de 7 días
- [ ] CA3: La configuración de tracing del daemon se separa de la del usuario (stderr para usuario, archivo con rotación para daemon)
- [ ] CA4: `kill()` reemplaza `thread::sleep(2s)` por un loop que verifica `is_process_alive()` cada 500ms hasta un máximo de 30 segundos
- [ ] CA5: Si el proceso muere durante el loop de verificación, no se envía SIGKILL (se detecta que ya no existe)
- [ ] CA6: El mensaje de error si el proceso no muere tras 30s es descriptivo: "el proceso PID no respondió a SIGTERM tras 30s"
- [ ] CA7: `cargo test --lib daemon` pasa

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgos #6.3, #6.5, recomendación #11).
