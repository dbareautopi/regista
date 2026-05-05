# STORY-019: Implementación de `model_for_role()` — Decisión técnica

**Fecha**: 2026-05-05  
**Actor**: Dev  
**Historia**: STORY-019

---

## Cambios realizados

Se implementó la lógica de resolución en `AgentsConfig::model_for_role()` en `src/config.rs`.  
Los campos `model` en `AgentsConfig` y `AgentRoleConfig` ya existían (el QA los añadió).

### Lógica de resolución (prioridad)

```
1. AgentRoleConfig.model  →  modelo específico del rol
2. AgentsConfig.model     →  modelo global
3. YAML frontmatter       →  campo `model` del skill .md (vía read_yaml_field)
4. "desconocido"          →  fallback último
```

### Detalles técnicos

- La función devuelve `String` (no `Option<String>`) para dar siempre un valor usable.
- Usa `crate::infra::providers::read_yaml_field()` para leer el frontmatter YAML,  
  igual que `opencode_build_args` en `providers.rs`.
- No paniquea si `skill_path` no existe: `read_yaml_field` trata `std::fs::read_to_string`  
  con `.ok()?` y retorna `None` silenciosamente.
- Para roles desconocidos, se salta el paso 1 (no hay `AgentRoleConfig`) y va  
  directamente al modelo global → YAML → "desconocido".

### Dead code

Los campos `model` y el método `model_for_role` solo se usan en tests por ahora.  
Se añadieron `#[allow(dead_code)]` siguiendo la convención del proyecto (ej: `epics_dir`).  
Se eliminarán cuando futuras historias integren `model_for_role` en el pipeline.

### Resultados

- **369 tests pasan** (0 fallos, 1 ignorado)
- **11 tests de arquitectura** pasan
- **cargo clippy -- -D warnings**: limpio
- **cargo fmt**: conforme
