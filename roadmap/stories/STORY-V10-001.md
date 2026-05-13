# STORY-V10-001: Definir trait LlmProvider y tipos base

## Status
**Done**

## Epic
EPIC-V10-01

## Descripción
Crear el nuevo módulo `infra/llm/` que define el contrato para todos los proveedores LLM nativos. Incluye el trait `LlmProvider`, los tipos de mensaje (`Message`, `ChatResponse`, `TokenUsage`) y una factory que instancia el provider correcto según la configuración TOML.

Este módulo reemplaza la lógica de `infra/providers.rs` + `infra/agent.rs` de v0.x, eliminando la dependencia de binarios CLI externos.

**Valor de negocio**: Fundación del nuevo sistema de invocación LLM. Sin esto, ningún otro módulo del rework puede funcionar.

## Criterios de aceptación
- [ ] CA1: El trait `LlmProvider` está definido en `infra/llm/mod.rs` con el método `fn chat(&self, messages: Vec<Message>, model: &str, timeout: Duration) -> Result<ChatResponse>` y es `Send + Sync + Debug`
- [ ] CA2: Los structs `Message { role, content }`, `ChatResponse { content, finish_reason, token_usage }` y `TokenUsage { input, output }` están en `infra/llm/types.rs` con `Serialize`/`Deserialize`
- [ ] CA3: La factory `LlmProvider::from_config(model_name, model_config) -> Result<Box<dyn LlmProvider>>` instancia `OpenAiProvider` o `AnthropicProvider` según `model_config.provider`, con error descriptivo si el provider es desconocido

## Dependencias
- Ninguna (es el primer módulo del rework)

## Activity Log
- 2026-05-08 | PO | historia creada desde DESIGN.md fase 1
- 2026-05-09 | Dev | STORY-V10-001 completada: trait LlmProvider, tipos Message/ChatResponse/TokenUsage, factory from_config(), 12 tests
