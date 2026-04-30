# 05 — `regista validate`

> ✅ **IMPLEMENTADO** — 2026-04-30

## 🎯 Objetivo

Comando que verifica la integridad y consistencia de todos los artefactos del
proyecto (historias, skills, configuración) sin ejecutar agentes.

## ❓ Problema actual

Los errores de configuración o formato solo se detectan en runtime, cuando el
orquestador ya está corriendo. Si una historia tiene un `Bloqueado por:
STORY-999` que no existe, el pipeline falla en la iteración 7 después de 30
minutos de ejecución.

## ✅ Solución propuesta

### Comando

```bash
regista validate [PROJECT_DIR]
regista validate --json          # salida estructurada para CI
```

### Qué valida

| Categoría | Checks |
|-----------|--------|
| **Config** | `.regista.toml` parseable, `stories_dir` existe, skills referenciadas existen en disco |
| **Historias** | Cada `.md` parseable, status válido, formato de ID correcto (`STORY-\d+`), tiene `Activity Log` |
| **Dependencias** | Todos los IDs en `Bloqueado por:` existen como archivos, sin ciclos (usando `dependency_graph`) |
| **Épicas** | Si se usan, IDs de épica coherentes, historias sin épica advertir |
| **Skills** | Archivos existen, son legibles |
| **Git** | Si `git.enabled`, repo existe o se puede inicializar |

### Salida esperada

```
✅ Config: .regista.toml OK
✅ Stories dir: product/stories/ (21 archivos)
✅ Skills: 4/4 encontrados
✅ Story STORY-001: OK (Draft)
✅ Story STORY-002: OK (Done)
...
⚠ Story STORY-013: no tiene Activity Log
❌ Story STORY-015: referencia STORY-999 que no existe
❌ Dependencias: ciclo detectado entre STORY-020 ↔ STORY-021

Resultado: 19 OK, 1 warning, 2 errores
```

### Exit codes

| Código | Significado |
|--------|-------------|
| 0 | Todo OK |
| 1 | Errores encontrados (no ejecutar pipeline) |
| 2 | Solo warnings (pipeline puede correr) |

## 📝 Notas de implementación

- Nuevo módulo `src/validator.rs` o función `validate()` en `config.rs`.
- Reusa `dependency_graph.rs` para detección de ciclos.
- Reusa `story.rs` para parseo.
- Con `--json`, emite array de hallazgos con severidad `error | warning`.
- Ideal como paso previo en CI: `regista validate && regista --once`.
- Debe ser rápido (sin llamadas a LLM, solo I/O de archivos).

## 🔗 Relacionado con

- [`03-dry-run.md`](./03-dry-run.md) — validate = chequeo estático, dry-run =
  chequeo dinámico.
- [`02-salida-json-ci-cd.md`](./02-salida-json-ci-cd.md) — validate también
  debe tener modo `--json`.
