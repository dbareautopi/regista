# EPIC-09: CLI y Visibilidad de Sesión

## Descripción
Flags de usuario (`--compact`, `--tail`) y header de sesión con metadatos. Conecta la infraestructura de EPIC-07 y EPIC-08 con la experiencia del desarrollador: qué flag usar para ver más o menos detalle, y qué información se muestra al iniciar una sesión de regista.

## Historias
- STORY-024: Flag `--compact` en `CommonArgs` y propagación al pipeline
- STORY-025: Flag `--tail` en `RepoArgs` + integración en `handle_logs()`
- STORY-026: Header de sesión con metadatos (versión, modelos, límites, git, hooks)
