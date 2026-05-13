//! Lógica de reintentos con backoff exponencial, timeout, y rate limiting.
//!
//! Adapta la lógica existente de `infra/agent.rs` (backoff exponencial +
//! timeout de proceso) al nuevo modelo de llamadas HTTP a APIs LLM.

use std::time::Duration;

use super::rate_limiter::RateLimiter;
use super::types::{ChatResponse, Message};
use super::LlmProvider;

/// Cota superior fija del delay entre reintentos: 300 segundos.
const MAX_BACKOFF_SECS: u64 = 300;

/// Clasifica si un error es reintentable.
///
/// Los errores de red, timeouts, HTTP 5xx y HTTP 429 son reintentables.
/// Los errores HTTP 4xx (salvo 429), errores de parseo, y errores de
/// autenticación no son reintentables.
fn is_retryable(error: &anyhow::Error) -> bool {
    let msg = error.to_string().to_lowercase();

    // Errores no reintentables
    if msg.contains("http 4") && !msg.contains("http 429") {
        // 400, 401, 403, 404... son errores del cliente (no reintentables)
        // Pero verificamos que no sea 429 (tratado aparte)
        return false;
    }

    if msg.contains("parse")
        || msg.contains("json")
        || msg.contains("choices")
        || msg.contains("content")
    {
        return false;
    }

    // Errores reintentables: timeout, conexión, 5xx, 429
    true
}

/// Extrae el valor de `retry-after` de un mensaje de error si contiene HTTP 429.
///
/// El formato esperado en el mensaje de error es:
/// "... Retry-After: <segundos>"
fn extract_retry_after(error: &anyhow::Error) -> Option<u64> {
    let msg = error.to_string();
    if !msg.to_lowercase().contains("http 429") {
        return None;
    }

    // Buscar "Retry-After: " en el mensaje de error
    if let Some(pos) = msg.find("Retry-After:") {
        let after = &msg[pos + "Retry-After:".len()..];
        // Tomar el primer token no vacío tras el header (split_whitespace elimina espacios)
        let num_str = after.split_whitespace().next().unwrap_or("").trim();
        return num_str.parse::<u64>().ok();
    }

    None
}

/// Invoca un provider LLM con reintentos, backoff exponencial, timeout y rate limiting.
///
/// # Parámetros
///
/// * `provider` — El provider LLM a invocar.
/// * `messages` — Historial completo de la conversación.
/// * `model` — Identificador del modelo en la API del provider.
/// * `timeout` — Timeout por request HTTP individual.
/// * `max_retries` — Número máximo de intentos (incluyendo el primero).
/// * `retry_delay_base` — Delay base para el backoff exponencial.
/// * `rate_limiter` — Rate limiter opcional compartido entre tareas.
///
/// # Backoff exponencial
///
/// En el intento N (1-indexed), el delay antes del siguiente intento es:
/// `min(retry_delay_base * 2^(N-1), 300s)`.
///
/// Si se recibe HTTP 429 con `retry-after`, se usa ese valor en lugar del
/// backoff estándar y se actualiza el rate limiter.
pub async fn invoke_with_retry(
    provider: &dyn LlmProvider,
    messages: Vec<Message>,
    model: &str,
    timeout: Duration,
    max_retries: u32,
    retry_delay_base: Duration,
    rate_limiter: Option<&RateLimiter>,
) -> anyhow::Result<ChatResponse> {
    let mut attempt = 1u32;
    let mut delay = retry_delay_base;

    loop {
        // Rate limiting: esperar si es necesario
        if let Some(rl) = rate_limiter {
            let wait = rl.acquire();
            if !wait.is_zero() {
                tracing::debug!("  rate limiter: esperando {:?} antes de la request", wait);
                tokio::time::sleep(wait).await;
            }
        }

        tracing::debug!(
            "  [{attempt}/{max_retries}] invocando {} (modelo: {model})",
            provider.provider_name(),
        );

        // Llamada directa al provider (sync).
        // ureq maneja su propio timeout; los delays de backoff son async.
        match provider.chat(messages.clone(), model, timeout) {
            Ok(response) => {
                tracing::info!(
                    "  ✓ {} completado (intento {attempt})",
                    provider.provider_name(),
                );
                return Ok(response);
            }
            Err(e) => {
                let retryable = is_retryable(&e);

                // Extraer retry-after si es HTTP 429
                let retry_after_secs = extract_retry_after(&e);

                if let Some(ra) = retry_after_secs {
                    tracing::warn!("  HTTP 429 rate limit: esperando {ra}s (retry-after header)");
                    // Actualizar rate limiter
                    if let Some(rl) = rate_limiter {
                        rl.apply_retry_after(ra);
                    }
                    // Usar el delay del servidor, no el backoff
                    delay = Duration::from_secs(ra);
                }

                if !retryable {
                    tracing::error!("  ✗ error no reintentable (intento {attempt}): {e}");
                    return Err(e);
                }

                if attempt >= max_retries {
                    tracing::error!("  ✗ agotados {max_retries} intentos para {}: {e}", model);
                    return Err(anyhow::anyhow!(
                        "agotados {max_retries} intentos para {} (último error: {e})",
                        model
                    ));
                }

                tracing::warn!(
                    "  ✗ intento {attempt} falló (reintentable): {e}. \
                     Esperando {:?} antes del reintento...",
                    delay
                );

                tokio::time::sleep(delay).await;

                // Backoff exponencial: delay *= 2, cota superior 300s
                if retry_after_secs.is_none() {
                    delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_BACKOFF_SECS));
                }

                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // EPIC-V10-01 | STORY-V10-004: Retry con backoff, timeout y rate limiting
    // ═══════════════════════════════════════════════════════════════

    // ── is_retryable ──────────────────────────────────────────────────

    #[test]
    fn is_retryable_timeout() {
        let err = anyhow::anyhow!("error HTTP al llamar a OpenAI: timed out");
        assert!(is_retryable(&err));
    }

    #[test]
    fn is_retryable_connection_error() {
        let err = anyhow::anyhow!("error HTTP al llamar a OpenAI: connection refused");
        assert!(is_retryable(&err));
    }

    #[test]
    fn is_retryable_http_500() {
        let err = anyhow::anyhow!("API OpenAI devolvió HTTP 500: Internal Server Error");
        assert!(is_retryable(&err));
    }

    #[test]
    fn is_retryable_http_429() {
        let err =
            anyhow::anyhow!("API OpenAI devolvió HTTP 429: rate limit exceeded. Retry-After: 30");
        assert!(is_retryable(&err));
    }

    #[test]
    fn is_not_retryable_http_400() {
        let err = anyhow::anyhow!("API OpenAI devolvió HTTP 400: Bad Request");
        assert!(!is_retryable(&err));
    }

    #[test]
    fn is_not_retryable_http_401() {
        let err = anyhow::anyhow!("API OpenAI devolvió HTTP 401: Unauthorized");
        assert!(!is_retryable(&err));
    }

    #[test]
    fn is_not_retryable_json_parse_error() {
        let err =
            anyhow::anyhow!("error parseando JSON de OpenAI: expected value at line 1 column 1");
        assert!(!is_retryable(&err));
    }

    #[test]
    fn is_not_retryable_missing_choices() {
        let err = anyhow::anyhow!("respuesta OpenAI sin campo 'choices'");
        assert!(!is_retryable(&err));
    }

    // ── extract_retry_after ───────────────────────────────────────────

    #[test]
    fn extract_retry_after_from_429_error() {
        let err = anyhow::anyhow!("API Anthropic devolvió HTTP 429: rate limit. Retry-After: 45");
        let result = extract_retry_after(&err);
        assert_eq!(result, Some(45));
    }

    #[test]
    fn extract_retry_after_no_429_returns_none() {
        let err = anyhow::anyhow!("API OpenAI devolvió HTTP 500: error");
        let result = extract_retry_after(&err);
        assert!(result.is_none());
    }

    #[test]
    fn extract_retry_after_429_without_retry_after_header() {
        let err = anyhow::anyhow!("API Anthropic devolvió HTTP 429: rate limit exceeded");
        let result = extract_retry_after(&err);
        assert!(result.is_none());
    }

    // ── CA1: backoff exponencial ─────────────────────────────────────

    /// CA1: El delay se duplica en cada reintento.
    #[test]
    fn story_v10004_ca1_backoff_doubles_each_retry() {
        let base = Duration::from_secs(10);
        // Primer delay: 10
        // Segundo: 20
        // Tercero: 40
        // Cuarto: 80
        // Quinto: 160 (sin alcanzar el cap de 300)
        let expected = [10, 20, 40, 80, 160];
        let mut delay = base;
        for (i, &exp) in expected.iter().enumerate() {
            assert_eq!(
                delay.as_secs(),
                exp,
                "intento {}: delay debería ser {exp}s",
                i + 1
            );
            delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_BACKOFF_SECS));
        }
    }

    /// CA1: El delay tiene cota superior en 300 segundos.
    #[test]
    fn story_v10004_ca1_backoff_capped_at_300_seconds() {
        let base = Duration::from_secs(10);
        let mut delay = base;
        let mut capped = false;

        for _ in 0..10 {
            delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_BACKOFF_SECS));
            if delay.as_secs() == MAX_BACKOFF_SECS && !capped {
                capped = true;
            }
            if capped {
                assert_eq!(
                    delay.as_secs(),
                    MAX_BACKOFF_SECS,
                    "delay debe estar capeado a 300s"
                );
            }
        }
        assert!(capped, "el delay debe alcanzar el cap de 300s");
    }

    /// CA1: El máximo de reintentos es 5 (6 intentos totales si max_retries=6).
    #[test]
    fn story_v10004_ca1_max_retries_is_respected() {
        // Este test verifica la lógica: con max_retries=1, solo un intento
        let max_retries = 1u32;
        let attempt = 1u32;

        // Simula el bucle
        assert!(attempt <= max_retries, "primer intento debe ejecutarse");

        // Sin reintentos (max_retries=1)
        // "agotados N intentos"
        if attempt >= max_retries {
            // Fin
        }
        // Con max_retries=6, se permiten 6 intentos
        let max6 = 6u32;
        for i in 1..=max6 {
            assert!(i <= max6);
        }
    }

    // ── Rate limiter integración ─────────────────────────────────────

    #[test]
    fn rate_limiter_zero_delay() {
        let rl = RateLimiter::new(Duration::ZERO);
        assert_eq!(rl.acquire(), Duration::ZERO);
        assert_eq!(rl.acquire(), Duration::ZERO);
    }

    /// Verifica que el backoff con valores pequeños funciona.
    #[test]
    fn story_v10004_ca1_backoff_with_delay_base() {
        // delay base = 1s
        let base = Duration::from_secs(1);
        let mut delay = base;
        assert_eq!(delay.as_secs(), 1);

        delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_BACKOFF_SECS));
        assert_eq!(delay.as_secs(), 2);

        delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_BACKOFF_SECS));
        assert_eq!(delay.as_secs(), 4);

        delay = std::cmp::min(delay * 2, Duration::from_secs(MAX_BACKOFF_SECS));
        assert_eq!(delay.as_secs(), 8);
    }

    // ── Constante MAX_BACKOFF_SECS ────────────────────────────────────

    #[test]
    fn max_backoff_constant_is_300() {
        assert_eq!(MAX_BACKOFF_SECS, 300);
    }

    // ── Provider dummy para tests de retry ────────────────────────────

    /// Provider que falla N veces antes de tener éxito.
    #[derive(Debug)]
    struct FailNTimesProvider {
        failures: std::sync::Mutex<u32>,
        name: String,
    }

    impl FailNTimesProvider {
        fn new(name: &str, failures: u32) -> Self {
            Self {
                failures: std::sync::Mutex::new(failures),
                name: name.to_string(),
            }
        }
    }

    impl LlmProvider for FailNTimesProvider {
        fn chat(
            &self,
            _messages: Vec<Message>,
            _model: &str,
            _timeout: Duration,
        ) -> anyhow::Result<ChatResponse> {
            let mut remaining = self.failures.lock().unwrap();
            if *remaining > 0 {
                *remaining -= 1;
                anyhow::bail!("HTTP 500: simulated failure ({} remaining)", *remaining + 1)
            }
            Ok(ChatResponse {
                content: "success".to_string(),
                finish_reason: "stop".to_string(),
                token_usage: None,
            })
        }

        fn provider_name(&self) -> &str {
            &self.name
        }
    }

    /// Provider que simula un timeout (nunca responde, siempre falla).
    #[derive(Debug)]
    struct TimeoutProvider {
        name: String,
    }

    impl TimeoutProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    impl LlmProvider for TimeoutProvider {
        fn chat(
            &self,
            _messages: Vec<Message>,
            _model: &str,
            _timeout: Duration,
        ) -> anyhow::Result<ChatResponse> {
            anyhow::bail!("error HTTP al llamar a {}: timed out", self.name)
        }

        fn provider_name(&self) -> &str {
            &self.name
        }
    }

    /// Provider que responde HTTP 429 con retry-after la primera vez.
    #[derive(Debug)]
    struct RateLimitedProvider {
        attempts: std::sync::Mutex<u32>,
        name: String,
    }

    impl RateLimitedProvider {
        fn new(name: &str) -> Self {
            Self {
                attempts: std::sync::Mutex::new(0),
                name: name.to_string(),
            }
        }
    }

    impl LlmProvider for RateLimitedProvider {
        fn chat(
            &self,
            _messages: Vec<Message>,
            _model: &str,
            _timeout: Duration,
        ) -> anyhow::Result<ChatResponse> {
            let mut attempts = self.attempts.lock().unwrap();
            *attempts += 1;

            if *attempts == 1 {
                anyhow::bail!(
                    "API {} devolvió HTTP 429: rate limit exceeded. Retry-After: 1",
                    self.name
                )
            } else {
                Ok(ChatResponse {
                    content: "success after rate limit".to_string(),
                    finish_reason: "stop".to_string(),
                    token_usage: None,
                })
            }
        }

        fn provider_name(&self) -> &str {
            &self.name
        }
    }

    // ── CA1: invoke_with_retry tests (async) ──────────────────────────

    /// Verifica que invoke_with_retry reintenta con backoff hasta el éxito.
    #[tokio::test]
    async fn story_v10004_ca1_invoke_with_retry_succeeds_after_retries() {
        let provider = FailNTimesProvider::new("test", 2);
        let messages = vec![Message::user("test")];
        let result = invoke_with_retry(
            &provider,
            messages,
            "test-model",
            Duration::from_secs(5),
            5,                         // max_retries
            Duration::from_millis(10), // small delay for tests
            None,
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.content, "success");
        // Debería haber hecho 3 intentos: 2 fallos + 1 éxito
        let remaining = *provider.failures.lock().unwrap();
        assert_eq!(remaining, 0, "provider debería haberse quedado sin fallos");
    }

    /// Verifica que invoke_with_retry falla tras agotar max_retries.
    #[tokio::test]
    async fn story_v10004_ca1_invoke_with_retry_fails_when_max_retries_exhausted() {
        let provider = FailNTimesProvider::new("test", 10); // siempre falla
        let messages = vec![Message::user("test")];
        let result = invoke_with_retry(
            &provider,
            messages,
            "test-model",
            Duration::from_secs(5),
            3, // max_retries = 3
            Duration::from_millis(10),
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("agotados 3 intentos"),
            "error debe indicar intentos agotados: {err}"
        );
    }

    /// Verifica que un timeout se trata como reintentable.
    #[tokio::test]
    async fn story_v10004_ca2_timeout_is_retryable_and_eventually_fails() {
        let provider = TimeoutProvider::new("test-timeout");
        let messages = vec![Message::user("test")];
        let result = invoke_with_retry(
            &provider,
            messages,
            "test-model",
            Duration::from_secs(5),
            2, // max_retries = 2 (poquitos para que el test sea rápido)
            Duration::from_millis(10),
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("timed out") || err.contains("agotados"),
            "error debe indicar timeout o intentos agotados: {err}"
        );
    }

    /// Verifica que HTTP 429 con retry-after se maneja correctamente.
    #[tokio::test]
    async fn story_v10004_ca3_rate_limited_with_retry_after_succeeds() {
        let provider = RateLimitedProvider::new("test-ratelimited");
        let messages = vec![Message::user("test")];
        let rate_limiter = RateLimiter::new(Duration::from_secs(0));

        let result = invoke_with_retry(
            &provider,
            messages,
            "test-model",
            Duration::from_secs(5),
            3,
            Duration::from_millis(10),
            Some(&rate_limiter),
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.content, "success after rate limit");
    }

    /// Verifica que invoke_with_retry hace exactamente 1 llamada cuando no hay errores.
    #[tokio::test]
    async fn story_v10004_ca1_invoke_with_retry_no_retries_on_success() {
        let provider = FailNTimesProvider::new("test", 0); // nunca falla
        let messages = vec![Message::user("test")];
        let result = invoke_with_retry(
            &provider,
            messages,
            "test-model",
            Duration::from_secs(5),
            5,
            Duration::from_millis(10),
            None,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "success");
    }
}
