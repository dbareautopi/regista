# 13 — `regista groom` — Generación automática de historias

## 🎯 Objetivo

Cerrar el círculo de automatización: que el propio PO del pipeline genere
todas las historias de usuario a partir de un documento de requisitos de alto
nivel, eliminando la fricción inicial de tener que escribirlas a mano.

## ❓ Problema actual

El pipeline asume que las historias **ya existen** en `product/stories/`.
Pero alguien tuvo que escribirlas. Hoy ese "alguien" es un humano usando `pi`
para refinar una a una. Esto rompe la promesa de automatización total y añade
**20-60 minutos de trabajo manual** antes de ver algún valor.

## ✅ Solución propuesta

### Comando

```bash
regista groom <SPEC.md> [--max-stories N] [--merge | --replace]
```

El usuario escribe **un solo archivo** con la visión general:

```markdown
# Spec: Onboarding de usuarios

## Visión
Quiero que los nuevos usuarios puedan registrarse, verificar su email,
y configurar su perfil antes de usar la app.

## Funcionalidades clave
- Registro con email/contraseña y OAuth (Google, GitHub)
- Verificación de email con link mágico
- Perfil con avatar, bio, y preferencias de notificaciones

## Restricciones
- Debe funcionar offline-first (PWA)
- Accesibilidad WCAG AA
- i18n desde el día 1 (es, en, fr)
```

### Lo que hace `regista groom`

1. **Invoca al PO** (`pi --skill product-owner`) con un prompt estructurado:
   - Lee la spec
   - Descompón en historias atómicas
   - Para cada historia: ID, título, descripción, criterios de aceptación, dependencias
   - Agrupa en épicas
   - Escribe los archivos `.md` en `product/stories/` y `product/epics/`

2. **Bucle de validación** (⚠️ crítico):
   ```
   groom → generate → validate → ¿errores de dependencias?
     ├── no → OK, terminar
     └── sí → devolver feedback al PO → generate → validate → ...
   ```
   El PO recibe los errores concretos del validador (referencias rotas,
   ciclos) y **corrige** las historias. Esto se repite hasta que el grafo
   de dependencias es correcto. Sin este bucle, el groom generaría historias
   que el pipeline normal no podría procesar.

3. **Máximo de iteraciones del bucle**: configurable (`groom_max_iterations`,
   default 5) para evitar bucles infinitos si el PO no es capaz de arreglarlo.

### Argumentos

| Flag | Default | Descripción |
|------|---------|-------------|
| `<SPEC.md>` | (requerido) | Documento de requisitos fuente |
| `--max-stories N` | `0` | Máximo de historias a generar. `0` = sin límite |
| `--merge` | ✅ default | Añade historias nuevas, no toca las existentes |
| `--replace` | | Regenera todo desde cero (borra historias existentes) |
| `--config` | `.regista.toml` | Ruta al archivo de configuración |

### Validación en bucle (detalle)

```
┌─────────────────────────────────────────┐
│ 1. PO genera historias desde spec       │
├─────────────────────────────────────────┤
│ 2. regista validate (solo dependencias) │
├─────────────────────────────────────────┤
│ 3. ¿Errores?                            │
│    ├── NO → ✅ Éxito, terminar          │
│    └── SÍ → Feedback al PO:             │
│         "Las siguientes historias tienen │
│          dependencias rotas: STORY-003   │
│          referencia STORY-999 que no     │
│          existe. Ciclo entre STORY-005   │
│          y STORY-007. Corrígelas."       │
│         → Volver a paso 1 (máx N veces) │
└─────────────────────────────────────────┘
```

## 📝 Notas de implementación

- El PO ya existe como skill. `groom` solo añade un prompt especializado y
  el bucle de validación.
- Reusa `validator.rs` para el chequeo de dependencias (ya implementado).
- Nuevo campo `groom_max_iterations` en `LimitsConfig` (default 5).
- `--max-stories 0` = sin límite. Si es > 0, el prompt del PO incluye
  "genera como máximo N historias".
- El bucle debe dar feedback **concreto** al PO: nombres de archivo, IDs,
  naturaleza del error. Nada de "hay un problema, arréglalo".
- Git snapshot antes del groom (si `git.enabled`) para poder hacer rollback
  si el PO destruye el backlog.
- `--merge` (default): solo crea historias que no existan. `--replace`:
  borra todo en `stories_dir` y `epics_dir` antes de generar.
- Posible integración futura: `regista bootstrap` = `groom` + `validate` +
  `--dry-run` para tener el pipeline listo en un solo comando.

## Consideraciones de diseño

1. **Idempotencia**: `--merge` permite iterar sobre la spec sin perder
   historias ya existentes. `--replace` es para empezar de cero.

2. **Dependencias automáticas**: el prompt debe pedir al PO que infiera
   dependencias entre historias (ej: "Registro" antes que "Verificación").

3. **Criterios de aceptación testeables**: el prompt debe enfatizar que los
   CAs deben ser específicos y verificables. Nada de "debe funcionar bien".

4. **Rollback**: compatible con git snapshots. Si el groom falla, se puede
   volver al estado anterior.

5. **Límites**: `--max-stories` evita que el PO genere 200 historias de
   golpe. Por defecto sin límite para flexibilidad.

## 🔗 Relacionado con

- [`14-groom-from-dir.md`](./14-groom-from-dir.md) — variante con múltiples
  documentos fuente.
- [`15-groom-interactive.md`](./15-groom-interactive.md) — variante
  interactiva donde el PO entrevista al usuario.
- [`05-validate.md`](./05-validate.md) — el validador que se usa en el bucle.
- [`03-dry-run.md`](./03-dry-run.md) — simular el pipeline tras el groom.
