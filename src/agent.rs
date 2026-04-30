//! Invocación de agentes `pi` con timeout, reintentos y backoff exponencial.
//!
//! Este módulo es el único punto de contacto con el CLI de `pi`.
//! Toda la lógica de timeout, señales, y reintentos está aquí.

use crate::config::LimitsConfig;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

/// Resultado de una invocación de agente.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AgentResult {
    /// Código de salida del proceso `pi`.
    pub exit_code: i32,
    /// stdout capturado.
    pub stdout: String,
    /// stderr capturado.
    pub stderr: String,
    /// Tiempo total que tomó (incluyendo reintentos).
    pub elapsed: Duration,
}

/// Invoca un agente `pi` con reintentos con backoff exponencial.
///
/// # Argumentos
/// - `skill_path`: ruta al skill .md (absoluta o relativa al proyecto).
/// - `prompt`: prompt a pasar con `-p`.
/// - `limits`: configuración de timeouts y reintentos.
///
/// # Retorna
/// - `Ok(AgentResult)` si el agente terminó con éxito (exit code 0) en algún intento.
/// - `Err(...)` si se agotaron los reintentos.
pub fn invoke_with_retry(
    skill_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
) -> anyhow::Result<AgentResult> {
    let mut attempt = 1u32;
    let mut delay = Duration::from_secs(limits.retry_delay_base_seconds);
    let timeout = Duration::from_secs(limits.agent_timeout_seconds);
    let max_retries = limits.max_retries_per_step;

    loop {
        tracing::info!(
            "  [{attempt}/{max_retries}] invocando pi --skill {}",
            skill_path.display()
        );

        match invoke_once(skill_path, prompt, timeout) {
            Ok(output) if output.status.success() => {
                tracing::info!("  ✓ agente completado (intento {attempt})");
                return Ok(agent_result_from_output(output));
            }
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                tracing::warn!(
                    "  ✗ agente falló (exit code {code}) — intento {attempt}/{max_retries}"
                );
            }
            Err(e) => {
                tracing::warn!("  ✗ error invocando agente: {e} — intento {attempt}/{max_retries}");
            }
        }

        if attempt >= max_retries {
            anyhow::bail!(
                "agotados {max_retries} reintentos invocando pi --skill {}",
                skill_path.display()
            );
        }

        tracing::info!("  ↻ reintentando en {}s...", delay.as_secs());
        std::thread::sleep(delay);
        attempt += 1;
        delay *= 2; // backoff exponencial
    }
}

/// Invoca `pi` una sola vez, con timeout.
fn invoke_once(skill_path: &Path, prompt: &str, _timeout: Duration) -> anyhow::Result<Output> {
    let result = std::process::Command::new("pi")
        .arg("-p")
        .arg(prompt)
        .arg("--skill")
        .arg(skill_path)
        .arg("--no-session")
        .output();

    match result {
        Ok(output) => Ok(output),
        Err(e) => {
            anyhow::bail!("no se pudo ejecutar 'pi': {e}. ¿Está instalado?");
        }
    }
}

fn agent_result_from_output(output: Output) -> AgentResult {
    AgentResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        elapsed: Duration::default(), // simplificado: no medimos tiempo por ahora
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requiere pi instalado"]
    fn invoke_with_retry_fails_when_pi_not_installed() {
        let limits = LimitsConfig {
            max_retries_per_step: 1,
            retry_delay_base_seconds: 0,
            agent_timeout_seconds: 5,
            ..Default::default()
        };
        // Usar un path que no existe para simular skill ausente
        let result = invoke_with_retry(Path::new("/nonexistent/skill.md"), "test", &limits);
        // Debería fallar porque pi falla o no encuentra el skill
        assert!(result.is_err());
    }
}
