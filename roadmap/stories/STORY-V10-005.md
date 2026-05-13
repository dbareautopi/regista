# STORY-V10-005: Configuración de modelos LLM en TOML

## Status
**Done**

## Epic
EPIC-V10-01

## Descripción
Añadir la sección `[models]` a `.regista/config.toml` y los tipos correspondientes en `config.rs` para que el usuario pueda declarar qué modelos LLM quiere usar, con qué provider, API key y base URL.

Cada modelo se referencia por un nombre lógico (ej. `gpt4o`, `claude`) que luego se usa en las fases del workflow (`model = "gpt4o"`). Las API keys nunca se escriben en claro en el archivo: se usa el patrón `${ENV_VAR}` para referenciar variables de entorno.

**Valor de negocio**: Separación limpia entre configuración de modelos (qué APIs están disponibles) y configuración de workflow (qué modelo usa cada fase). Las API keys nunca se commitean al repositorio.

## Criterios de aceptación
- [ ] CA1: La sección `[models.<name>]` se deserializa en `HashMap<String, ModelConfig>` con campos `provider`, `model_id`, `api_key`, `base_url` (opcional). Los structs `ModelConfig` y la lógica de expansión `${ENV_VAR}` están en `config.rs`
- [ ] CA2: `Config::resolve_model(name) -> Result<ModelConfig>` busca el modelo por nombre lógico y expande `${ENV_VAR}` en `api_key`. Si el modelo no existe o la variable de entorno no está definida, devuelve error descriptivo
- [ ] CA3: `regista validate` verifica que todos los modelos referenciados en `workflow.phases[].model` existen en `[models]` y que sus variables de entorno están definidas (con warning si `api_key` está vacío para Ollama)

## Dependencias
- Bloqueado por: STORY-V10-001

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 1
- 2026-05-09 | Dev | STORY-V10-005 completada: [models] en TOML, ModelConfig, resolve_model() con ${ENV_VAR}, validate_models() en app/validate.rs, 14 tests
