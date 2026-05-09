# language: es
@config @models @STORY-V10-005

Feature: Configuración de modelos LLM en TOML
  Como usuario que quiere usar diferentes modelos LLM,
  quiero declarar modelos en config.toml con provider, api_key y base_url,
  para que las API keys nunca se commiteen y los modelos se referencien por nombre lógico.

  Background:
    Given el archivo .regista/config.toml contiene:
      """
      [models.gpt4o]
      provider = "openai"
      model_id = "gpt-4o"
      api_key = "${OPENAI_API_KEY}"

      [models.claude]
      provider = "anthropic"
      model_id = "claude-sonnet-4-20250514"
      api_key = "${ANTHROPIC_API_KEY}"
      """

  Scenario: Deserializar sección [models] en HashMap<String, ModelConfig>
    When se carga Config::load() desde el archivo TOML
    Then config.models contiene la clave "gpt4o" con ModelConfig {provider: "openai", model_id: "gpt-4o"}
    And config.models contiene la clave "claude" con ModelConfig {provider: "anthropic"}

  Scenario: Expandir ${ENV_VAR} en api_key
    Given la variable de entorno OPENAI_API_KEY está definida como "sk-real-key"
    When se invoca Config::resolve_model("gpt4o")
    Then el ModelConfig resultante tiene api_key = "sk-real-key"
    Given la variable ANTHROPIC_API_KEY no está definida
    When se invoca Config::resolve_model("claude")
    Then devuelve Err con mensaje "variable de entorno ANTHROPIC_API_KEY no definida"

  Scenario: Validate detecta modelos referenciados pero no definidos
    Given el workflow referencia model="gemini" en una fase
    And [models] no contiene la clave "gemini"
    When se ejecuta regista validate
    Then se reporta un Error: "modelo 'gemini' referenciado en fase 'X' no está definido en [models]"
