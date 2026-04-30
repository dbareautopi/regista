# 15 — `regista groom --interactive`

## 🎯 Objetivo

Permitir que el PO **entreviste al usuario** para extraer requisitos, en
lugar de leer un documento estático. Esto es útil cuando el usuario no tiene
una spec formal escrita o prefiere una conversación para refinar ideas.

## ❓ Problema

Escribir una spec formal requiere disciplina y tiempo. Muchos desarrolladores
(y especialmente no-técnicos) prefieren **explicar lo que necesitan en una
conversación**. Un documento estático no captura matices que una entrevista sí.

## ✅ Solución propuesta

### Comando

```bash
regista groom --interactive [--max-stories N]
```

### Flujo

```
Usuario: regista groom --interactive

PO: ¿Qué problema resuelve este producto?
Usuario: Los developers pierden tiempo configurando CI/CD.

PO: ¿Quiénes son los usuarios objetivo?
Usuario: Developers y DevOps engineers.

PO: Dame 3 funcionalidades clave, ordenadas por importancia.
Usuario: 1. Detección automática del stack.
         2. Generación de pipeline YAML.
         3. Integración con GitHub Actions.

PO: ¿Hay restricciones técnicas que deba conocer?
Usuario: Debe funcionar en Linux y macOS. No puede depender de Docker.

PO: Vale. He capturado suficiente contexto. Voy a generar el backlog ahora...

[PO genera historias, épicas, dependencias]

PO: He generado 12 historias en 3 épicas:
     EPIC-001: Detección de stack (4 historias)
     EPIC-002: Generación de pipeline (5 historias)
     EPIC-003: Integración GitHub (3 historias)
     ¿Quieres revisarlas antes de continuar? [sí/no]

Usuario: sí

[PO muestra resumen o el usuario puede leer los archivos generados]

Usuario: ok, adelante
```

### Características

- El PO hace **máximo N preguntas** (configurable, default 5-7) antes de
  generar. Esto evita entrevistas infinitas.
- Las preguntas deben ser **abiertas pero dirigidas**: problema, usuarios,
  funcionalidades, restricciones, stack.
- Al terminar la entrevista, el PO genera las historias y ejecuta el
  **bucle de validación** de dependencias igual que en el modo normal.

## 📝 Notas de implementación

- Nuevo campo `groom_max_questions` en `LimitsConfig` (default 6).
- El prompt del PO debe incluir instrucciones para la entrevista:
  "Haz preguntas abiertas. No más de N preguntas. Cuando tengas suficiente
  contexto, genera las historias sin pedir confirmación."
- El usuario puede interrumpir la entrevista con `/done` para forzar la
  generación inmediata.
- Al generar, el PO documenta la "entrevista" en `product/decisions/` como
  contexto para futuros agentes.
- Compatible con `--max-stories`.

## ⚠️ Riesgos

- **Alucinaciones**: el PO puede malinterpretar o asumir requisitos que el
  usuario no dio. Mitigación: el resumen final y la opción de revisar.
- **Entrevistas vagas**: si el usuario da respuestas ambiguas, el backlog
  será pobre. El PO debe pedir concreción.
- **Costo**: una entrevista consume más tokens que procesar un documento.

## 🔗 Relacionado con

- [`13-groom-generacion-historias.md`](./13-groom-generacion-historias.md) —
  la idea base con documento estático.
- [`14-groom-from-dir.md`](./14-groom-from-dir.md) — variante con múltiples
  documentos.
