# 14 — `regista groom --from-dir`

## 🎯 Objetivo

Extender `regista groom` para aceptar un **directorio** de documentos fuente
en lugar de un solo archivo. Esto permite organizar specs por features, módulos,
o épicas en proyectos grandes.

## ❓ Problema

Un solo `spec.md` no escala para proyectos con 10+ features. Forzar todo en
un archivo único lo hace inmanejable y ambiguo. El PO tiene más contexto si
cada feature tiene su propio documento.

## ✅ Solución propuesta

### Comando

```bash
regista groom --from-dir docs/features/ [--max-stories N]
```

### Estructura esperada

```
docs/features/
├── 01-autenticacion.md
├── 02-perfiles.md
├── 03-notificaciones.md
├── 04-admin-panel.md
└── 05-reporting.md
```

Cada archivo describe una **épica o feature independiente**. El PO procesa
cada documento y genera historias para cada uno, manteniendo la agrupación
en épicas.

### Comportamiento

1. Escanear el directorio en orden alfabético.
2. Para cada `.md`, invocar al PO con el contexto de ese documento.
3. El PO puede ver historias generadas por documentos anteriores para evitar
   duplicados y establecer dependencias cross-feature.
4. Al final, ejecutar el **bucle de validación** sobre el conjunto completo.

### Variante: `docs/epics/`

Si el directorio ya contiene **épicas** predefinidas, el PO solo descompone
cada épica en historias:

```bash
regista groom --from-epics-dir product/epics/
```

## 📝 Notas de implementación

- El PO debe recibir contexto acumulativo: al procesar el documento N, puede
  ver las historias generadas por los documentos 1..N-1.
- Si un documento ya tiene historias asociadas (por `--merge`), se salta.
- Compatible con `--max-stories`: el límite se aplica al total, no por documento.
- La prioridad entre documentos la da el orden alfabético (prefijos numéricos).

## 🔗 Relacionado con

- [`13-groom-generacion-historias.md`](./13-groom-generacion-historias.md) —
  la idea base con un solo documento.
- [`15-groom-interactive.md`](./15-groom-interactive.md) — variante interactiva.
