# 02 — Salida JSON + integración CI/CD

> ✅ **IMPLEMENTADO** — 2026-04-30

## 🎯 Objetivo

Proveer una salida estructurada (JSON) del resultado del pipeline, con exit
codes significativos, para integración en sistemas de CI/CD (GitHub Actions,
GitLab CI, Jenkins).

## ❓ Problema actual

Todo el output del orquestador va a `tracing` (stderr). No hay forma de:

- Obtener un resumen machine-readable del resultado.
- Diferenciar entre "pipeline completado con éxito" y "pipeline terminó con
  historias en Failed".
- Generar anotaciones en el PR/MR automáticamente.

Un CI/CD necesita saber **si falló algo** sin parsear logs de texto.

## ✅ Solución propuesta

### Flag `--json`

```bash
regista --json                  # stdout = JSON, stderr = logs
regista --json --quiet          # stdout = JSON, stderr = nada (solo errores)
```

### Formato de salida

```json
{
  "regista_version": "0.1.0",
  "project_dir": "/root/repos/purist",
  "result": "completed",
  "exit_code": 0,
  "summary": {
    "total": 21,
    "done": 7,
    "failed": 2,
    "blocked": 8,
    "draft": 4,
    "iterations": 12,
    "elapsed_seconds": 1847
  },
  "stories": [
    {
      "id": "STORY-001",
      "status": "Done",
      "epic": "EPIC-001",
      "iterations": 4,
      "reject_cycles": 0,
      "error": null
    },
    {
      "id": "STORY-013",
      "status": "Failed",
      "epic": "EPIC-003",
      "iterations": 6,
      "reject_cycles": 3,
      "error": "max_reject_cycles alcanzado tras revisión técnica"
    }
  ]
}
```

### Exit codes

| Código | Significado |
|--------|-------------|
| 0 | Pipeline completo, 0 historias `Failed` |
| 1 | Error de configuración / entorno |
| 2 | Pipeline completo, ≥1 historia `Failed` |
| 3 | Timeout (`max_wall_time` o `max_iterations`) |

### Integración GitHub Actions

```yaml
- name: Run regista pipeline
  run: |
    regista . --json --once > regista-report.json
    cat regista-report.json | jq '.stories[] | select(.status == "Failed")'
```

## 📝 Notas de implementación

- Nuevo campo `json: bool` en `RunOptions`.
- `RunReport` debe implementar `Serialize` y exponer el estado final de
  cada historia.
- El flag `--json` redirige el reporte a stdout (en vez del box-drawing de
  tracing).
- Compatible con `--quiet` para suprimir logs de progreso.
- Posible extensión: `--format json | markdown | github-annotations`.

## 🔗 Relacionado con

- [`03-dry-run.md`](./03-dry-run.md) — el dry-run también debe generar JSON.
- [`05-validate.md`](./05-validate.md) — validación pre-vuelo con salida JSON.
