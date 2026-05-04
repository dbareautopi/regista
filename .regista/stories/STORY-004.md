# STORY-004: `StoryCache` con invalidación por `mtime`

## Status
**Draft**

## Epic
EPIC-02

## Descripción
El pipeline actual llama a `load_all_stories()` al inicio de cada iteración, re-parseando todos los archivos `.md` del directorio `stories_dir`. En un pipeline típico de 300 iteraciones con 50 historias, esto genera 15.000 lecturas de archivo, aunque solo 1 historia cambia por iteración. Implementar un `StoryCache` que mantenga las historias en memoria y solo re-parsee aquellas cuyo `mtime` (fecha de modificación del archivo) haya cambiado desde la última lectura.

## Criterios de aceptación
- [ ] CA1: Existe un struct `StoryCache` (en `src/domain/story.rs` o módulo nuevo `src/domain/story_cache.rs`) con un `HashMap<String, CachedStory>` interno
- [ ] CA2: `StoryCache::get_stories()` devuelve las historias cacheadas, re-parseando solo archivos con `mtime` modificado
- [ ] CA3: `StoryCache::invalidate(story_id)` fuerza el re-parseo de una historia específica (para usar tras `set_status`)
- [ ] CA4: El orchestrator (`pipeline.rs`) usa `StoryCache` en lugar de `load_all_stories()` en cada iteración
- [ ] CA5: `StoryCache` maneja correctamente archivos nuevos (que no estaban en cache) y archivos eliminados
- [ ] CA6: `cargo test --lib story` pasa (tests existentes + nuevos tests para StoryCache)
- [ ] CA7: En un test con 50 historias, `StoryCache::get_stories()` con 1 historia modificada solo re-parsea 1 archivo (verificable contando llamadas a `fs::metadata`)

## Dependencias
(Ninguna)

## Activity Log
- 2026-05-04 | PO | Historia generada desde roadmap/AUDITORIA-ESCALABILIDAD.md (hallazgo #2.1, recomendación #1).
