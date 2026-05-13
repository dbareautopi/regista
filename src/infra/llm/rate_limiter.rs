//! Rate limiter para providers LLM.
//!
//! Garantiza un delay mínimo entre requests consecutivas al mismo provider
//! para respetar los rate limits de las APIs. Thread-safe mediante `Mutex`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Controla la tasa de requests a un provider LLM.
///
/// Garantiza que entre requests consecutivas transcurra al menos `min_delay`.
/// Si se usa desde múltiples tareas, se debe compartir mediante `Arc`.
#[derive(Debug)]
pub struct RateLimiter {
    last_request: Mutex<Option<Instant>>,
    min_delay: Duration,
}

impl RateLimiter {
    /// Crea un nuevo rate limiter con el delay mínimo especificado.
    pub fn new(min_delay: Duration) -> Self {
        Self {
            last_request: Mutex::new(None),
            min_delay,
        }
    }

    /// Calcula cuánto tiempo esperar antes de la próxima request.
    ///
    /// Si no ha habido requests previas, devuelve `Duration::ZERO`.
    /// Si ha pasado más de `min_delay` desde la última request, devuelve `Duration::ZERO`.
    /// En otro caso, devuelve el tiempo restante para cumplir `min_delay`.
    ///
    /// Al llamar a este método, se registra la marca de tiempo actual como
    /// la base de la próxima request (incluso si esta request aún no se ha hecho).
    /// Esto evita que múltiples tareas calculen el mismo delay simultáneamente.
    pub fn acquire(&self) -> Duration {
        let mut last = self.last_request.lock().unwrap();
        let now = Instant::now();

        match *last {
            None => {
                *last = Some(now);
                Duration::ZERO
            }
            Some(prev) => {
                // Si prev está en el futuro (por apply_retry_after), esperar hasta entonces
                if prev > now {
                    let wait = prev - now;
                    *last = Some(now + wait);
                    return wait;
                }
                let elapsed = now - prev;
                if elapsed >= self.min_delay {
                    *last = Some(now);
                    Duration::ZERO
                } else {
                    let wait = self.min_delay - elapsed;
                    *last = Some(now + wait);
                    wait
                }
            }
        }
    }

    /// Retraso desde una respuesta HTTP 429 con `retry-after`.
    ///
    /// Parsea el header `retry-after` (segundos) y aplica un delay forzoso
    /// registrándolo en el rate limiter para que futuras llamadas a `acquire()`
    /// también lo respeten.
    pub fn apply_retry_after(&self, retry_after_seconds: u64) {
        let mut last = self.last_request.lock().unwrap();
        *last = Some(Instant::now() + Duration::from_secs(retry_after_seconds));
    }

    /// Devuelve el delay mínimo configurado.
    #[cfg(test)]
    pub(crate) fn min_delay(&self) -> Duration {
        self.min_delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // EPIC-V10-01 | STORY-V10-004: Rate limiter
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn rate_limiter_first_call_returns_zero() {
        let rl = RateLimiter::new(Duration::from_secs(1));
        let wait = rl.acquire();
        assert_eq!(wait, Duration::ZERO);
    }

    #[test]
    fn rate_limiter_second_call_within_delay_returns_remaining() {
        let rl = RateLimiter::new(Duration::from_secs(60));
        let first = rl.acquire();
        assert_eq!(first, Duration::ZERO);

        let second = rl.acquire();
        // Segunda llamada inmediata debe pedir esperar ~60 segundos
        assert!(second > Duration::ZERO);
        assert!(second <= Duration::from_secs(60));
    }

    #[test]
    fn rate_limiter_respects_min_delay() {
        let rl = RateLimiter::new(Duration::from_millis(100));
        let _ = rl.acquire();

        // Esperamos más del min_delay
        std::thread::sleep(Duration::from_millis(150));

        let wait = rl.acquire();
        assert_eq!(
            wait,
            Duration::ZERO,
            "después de esperar > min_delay, acquire debe devolver 0"
        );
    }

    #[test]
    fn rate_limiter_zero_delay_always_returns_zero() {
        let rl = RateLimiter::new(Duration::ZERO);
        assert_eq!(rl.acquire(), Duration::ZERO);
        assert_eq!(rl.acquire(), Duration::ZERO);
        assert_eq!(rl.acquire(), Duration::ZERO);
    }

    /// CA3: apply_retry_after fuerza un delay basado en el header retry-after.
    #[test]
    fn story_v10004_ca3_apply_retry_after_forces_delay() {
        let rl = RateLimiter::new(Duration::from_secs(1));
        let _ = rl.acquire();

        // Simular un 429 con retry-after: 5 segundos
        rl.apply_retry_after(5);

        // La siguiente llamada debe requerir esperar ~5 segundos
        let wait = rl.acquire();
        assert!(
            wait >= Duration::from_secs(4),
            "wait debería ser ~5s, fue {:?}",
            wait
        );
        assert!(
            wait <= Duration::from_secs(6),
            "wait debería ser ~5s, fue {:?}",
            wait
        );
    }

    #[test]
    fn rate_limiter_min_delay_stored_correctly() {
        let rl = RateLimiter::new(Duration::from_secs(2));
        assert_eq!(rl.min_delay(), Duration::from_secs(2));
    }

    #[test]
    fn rate_limiter_multiple_sequential_acquires() {
        let rl = RateLimiter::new(Duration::from_millis(50));

        let first = rl.acquire();
        assert_eq!(first, Duration::ZERO);

        // Esperar suficiente
        std::thread::sleep(Duration::from_millis(100));

        let second = rl.acquire();
        assert_eq!(second, Duration::ZERO);

        // Inmediatamente después debe pedir espera
        let third = rl.acquire();
        assert!(third > Duration::ZERO);
    }
}
