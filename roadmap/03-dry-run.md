# 03 — Dry-run

> ✅ **IMPLEMENTADO** — 2026-04-30

## 🎯 Objetivo

Simular la ejecución del pipeline sin invocar agentes reales, mostrando qué
decisiones tomaría el orquestador y qué transiciones aplicaría.

## ❓ Problema actual

No hay forma de previsualizar el comportamiento del orquestador antes de
ejecutarlo. Como modifica archivos `.md` y hace commits git, un developer
necesita confianza en que la herramienta hará lo correcto.

El dry-run es el equivalente a `terraform plan` — esencial para adopción.

## ✅ Solución propuesta

### Flag `--dry-run`

```bash
regista --dry-run
regista --dry-run --json       # salida estructurada
regista --dry-run --once       # simular solo una iteración
```

### Salida esperada

```
🧪 DRY-RUN — No se ejecutarán agentes ni se modificarán archivos.

═══ Iteración 1 ═══
  → STORY-007 (Draft) sería procesada por PO (groom) → Ready
  → Desbloquearía: STORY-008, STORY-016

═══ Iteración 2 ═══
  → STORY-008 (Blocked) desbloqueada automáticamente → Ready
  → STORY-007 (Ready) sería procesada por QA (tests) → Tests Ready

...

Resumen:
  Total historias: 21
  Done:           7
  Failed:         0
  Pendientes:     14
  Iteraciones estimadas: 18
  Tiempo estimado: ~75-120 min
```

### Modo de implementación

El dry-run **no invoca `pi`**, pero sí ejecuta toda la lógica del orquestador:

1. Carga historias normalmente.
2. Aplica transiciones automáticas (Blocked→Ready, Failed) en un **clone en
   memoria** de las historias (sin escribir a disco).
3. Evalúa deadlock.
4. Simula `process_story()` asumiendo que el agente **siempre tiene éxito**
   (transición feliz) y muestra qué pasaría.

### Opcional: modo "pesimista"

```bash
regista --dry-run --pessimistic
```
Simula también rechazos aleatorios para ver cómo se comporta el pipeline bajo
estrés.

## 📝 Notas de implementación

- Nuevo campo `dry_run: bool` en `RunOptions`.
- `Story` necesita un método `clone_in_memory()` que no escriba a disco.
- `process_story()` debe aceptar un flag `simulate: bool` que saltee
  `agent::invoke_with_retry` y `set_status()`.
- El dry-run puede exponer bugs en el grafo de dependencias antes de gastar
  créditos de LLM.

## 🔗 Relacionado con

- [`02-salida-json-ci-cd.md`](./02-salida-json-ci-cd.md) — dry-run con `--json`
  para validación en CI.
- [`05-validate.md`](./05-validate.md) — complementario: validate es chequeo
  estático, dry-run es chequeo dinámico.
