# 06 — `regista init`

> ✅ **IMPLEMENTADO** — 2026-04-30

## 🎯 Objetivo

Comando que genera un proyecto nuevo con toda la estructura necesaria para
usar `regista`: configuración, skills de ejemplo, y una historia de plantilla.

## ❓ Problema actual

Empezar con `regista` requiere:

1. Crear `.regista.toml` a mano (conociendo todas las claves y defaults).
2. Crear 4 skills de `pi` desde cero (sin saber qué formato o contenido
   esperan).
3. Escribir la primera historia en el formato exacto que `story.rs` espera.
4. Crear la estructura de directorios `product/stories/`, `product/decisions/`,
   `product/logs/`.

Esto son **20-30 minutos de fricción inicial** antes de ver algún valor.
Muchos developers abandonan en este punto.

## ✅ Solución propuesta

### Comando

```bash
regista init [PROJECT_DIR]
regista init --light              # solo .regista.toml, sin skills
regista init --with-example       # incluye historia de ejemplo
```

### Lo que genera

```
mi-proyecto/
├── .regista.toml                    ← Config con defaults y paths relativos
├── .pi/
│   └── skills/
│       ├── product-owner/
│       │   └── SKILL.md             ← Skill genérico de PO
│       ├── qa-engineer/
│       │   └── SKILL.md             ← Skill genérico de QA
│       ├── developer/
│       │   └── SKILL.md             ← Skill genérico de Dev
│       └── reviewer/
│           └── SKILL.md             ← Skill genérico de Reviewer
└── product/
    ├── stories/
    │   └── STORY-001.md             ← Historia de ejemplo (opcional)
    ├── epics/
    │   └── EPIC-001.md              ← Épica de ejemplo (opcional)
    ├── decisions/                   ← Vacío
    └── logs/                        ← Vacío
```

### Skills generados

Cada `SKILL.md` sería un skill mínimo pero funcional, con:

```markdown
# Product Owner Skill

Eres un Product Owner. Tu trabajo es refinar y validar historias de usuario.

## Responsabilidades
- Leer historias desde {{stories_dir}}
- Refinar (Draft → Ready): verificar criterios DoR
- Validar (Business Review → Done): verificar valor de negocio
- Documentar decisiones en {{decisions_dir}}
- Actualizar el Activity Log en la historia
- NO preguntar nada al usuario
```

Con placeholders que `regista` reemplaza en runtime (o el propio `pi` con el
contexto del prompt).

### Interactivo (opcional)

```bash
regista init --interactive
```

Pregunta:
- ¿Nombre del proyecto?
- ¿Lenguaje/stack? (Rust, Python, TypeScript, genérico)
- ¿Usarás QA automatizado? (sí/no)
- ¿Git? (sí/no)

Y ajusta `.regista.toml` + skills según respuestas.

## 📝 Notas de implementación

- Nuevo módulo `src/init.rs`.
- Templates de skills como constantes de string o archivos embebidos con
  `include_str!`.
- `--with-example` genera una historia `STORY-001.md` parseable y funcional.
- El init **no pisa archivos existentes** (pregunta antes de sobrescribir).
- Puede integrarse con `regista validate` al final para confirmar que
  todo lo generado es válido.

## 🔗 Relacionado con

- [`05-validate.md`](./05-validate.md) — correr validate tras init para
  verificar.
- [`04-workflow-configurable.md`](./04-workflow-configurable.md) — si hay
  workflow custom, init debería soportarlo.
