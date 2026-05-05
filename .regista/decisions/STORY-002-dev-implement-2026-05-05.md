# STORY-002 Dev Implementation — 2026-05-05

## Resumen

Migradas las funciones libres `provider_for_role()` y `skill_for_role()` desde `src/infra/providers.rs` a métodos de `AgentsConfig` en `src/config.rs`, eliminando el acoplamiento incorrecto infra → config.

## Cambios realizados

### 1. `src/config.rs` — `AgentsConfig::provider_for_role()` y `skill_for_role()`

Se reemplazaron los stubs con `unimplemented!()` por la implementación real:

- **`provider_for_role(&self, role: &str) -> String`**: Busca el `AgentRoleConfig` del rol; si tiene `provider` explícito lo usa, si no hereda `self.provider` (global). Rol desconocido → devuelve el provider global.
- **`skill_for_role(&self, role: &str) -> String`**: Busca el `AgentRoleConfig` del rol; si tiene `skill` explícito lo usa, si no llama a `provider.instruction_dir(role)` usando `crate::infra::providers::from_name()` para resolver el provider. Rol desconocido → `String::new()`.

La implementación es una copia directa de la lógica que estaba en `providers.rs`, sin cambios de comportamiento.

### 2. `src/infra/providers.rs` — Eliminación de funciones libres

Se eliminaron las funciones `pub fn provider_for_role(...)` y `pub fn skill_for_role(...)` junto con el bloque de comentarios "Resolver provider e instrucciones desde AgentsConfig".

### 3. Actualización de callers

| Archivo | Cambio |
|---------|--------|
| `src/app/pipeline.rs` | `providers::provider_for_role(&cfg.agents, role)` → `cfg.agents.provider_for_role(role)` y equivalente para `skill_for_role` |
| `src/app/plan.rs` | Ídem (plan usa solo el PO) |
| `src/app/validate.rs` | Ídem (validate_skills y validate_providers) |

### 4. Actualización de tests antiguos

Los tests en `config.rs` que usaban `providers::provider_for_role(&cfg.agents, ...)` y `providers::skill_for_role(&cfg.agents, ...)` se actualizaron a la sintaxis de método. Se eliminó el `use crate::infra::providers;` del módulo de tests de config.rs.

Los tests de `providers.rs` ya usaban la sintaxis de método (los escribió el QA con visión de la migración), no requirieron cambios.

### 5. Tests STORY-002 del QA

Los 22 tests escritos por QA (etiquetados `story002_ca*`) pasan correctamente:
- CA1: 6 tests (`provider_for_role`)
- CA2: 8 tests (`skill_for_role`)
- CA3: 2 tests (`all_roles`)
- CA4: 3 tests (verifican que no hay funciones libres)
- CA5: 3 tests (patrón de caller)

## Verificación

| Comando | Resultado |
|---------|-----------|
| `cargo build` | ✅ Compila sin warnings |
| `cargo test` | ✅ 346 tests pasan, 0 fallos, 1 ignorado |
| `cargo clippy -- -D warnings` | ✅ Sin warnings |
| `cargo fmt -- --check` | ✅ Formato correcto |
| Tests de arquitectura | ✅ 11/11 pasan |

## Consecuencias de diseño

- La capa `config` ahora tiene una dependencia en `crate::infra::providers::from_name()` (usada en `skill_for_role`). Esto es aceptable porque `config` es la capa de dominio/configuración y `providers` es infraestructura: la dependencia es config → infra, no infra → config (que era el antipatrón original).
- Las funciones libres ya no existen: los callers usan `cfg.agents.provider_for_role(role)`, forzando el paso por la configuración centralizada.
- No hay cambios de comportamiento: la lógica de resolución de provider y skill es idéntica a la anterior.
