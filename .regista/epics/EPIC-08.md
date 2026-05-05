# EPIC-08: Streaming de Agente y Mejora de Daemon Logs

## Descripción
Cambios de infraestructura con I/O real: modificar `invoke_once()` para leer stdout del agente línea a línea y emitirlo al log en tiempo real, controlado por un flag `verbose`. Modificar `follow()` del daemon para que pueda volcar el historial completo antes de entrar en modo tail.

## Historias
- STORY-022: Streaming de stdout del agente en `invoke_once()` + parámetro `verbose`
- STORY-023: `follow()` con parámetro `from_beginning` para volcado de historial completo
