//! Invocación de agentes `pi` con timeout, reintentos, backoff exponencial,
//! y feedback rico (captura de stdout/stderr para trazabilidad y reintentos).

use crate::config::LimitsConfig;
use crate::infra::providers::AgentProvider;
use std::path::{Path, PathBuf};
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
    /// Intento en el que tuvo éxito (1-indexed).
    pub attempt: u32,
    /// Traza completa de cada intento (para guardar en decisions/).
    pub attempts: Vec<AttemptTrace>,
}

/// Traza de un intento individual de invocación.
#[derive(Debug, Clone)]
pub struct AttemptTrace {
    pub attempt: u32,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Opciones adicionales para la invocación de agentes (feedback + checkpoint).
#[derive(Debug, Clone, Default)]
pub struct AgentOptions {
    /// ID de la historia que se está procesando (para guardar decisiones).
    pub story_id: Option<String>,
    /// Directorio donde guardar los outputs de agente.
    pub decisions_dir: Option<PathBuf>,
    /// Si inyectar el stderr del intento fallido en el prompt del reintento.
    pub inject_feedback: bool,
}

/// Invoca un agente `pi` con reintentos con backoff exponencial.
///
/// # Feedback rico
/// Si `opts.inject_feedback` es true, en cada reintento se inyecta el stderr
/// del intento anterior en el prompt, dando contexto al agente de su fallo.
///
/// Si `opts.decisions_dir` está presente, se guarda una traza completa de
/// cada intento en `<decisions_dir>/<story_id>-<actor>-<timestamp>.md`.
pub fn invoke_with_retry(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
) -> anyhow::Result<AgentResult> {
    let mut attempt = 1u32;
    let mut delay = Duration::from_secs(limits.retry_delay_base_seconds);
    let timeout = Duration::from_secs(limits.agent_timeout_seconds);
    let max_retries = limits.max_retries_per_step;
    let mut attempts: Vec<AttemptTrace> = vec![];
    let mut current_prompt = prompt.to_string();

    loop {
        tracing::info!(
            "  [{attempt}/{max_retries}] invocando {} ({})",
            provider.display_name(),
            instruction_path.display()
        );

        match invoke_once(provider, instruction_path, &current_prompt, timeout) {
            Ok(output) if output.status.success() => {
                tracing::info!("  ✓ agente completado (intento {attempt})");

                let trace = trace_from_output(attempt, &output);
                attempts.push(trace);

                // Guardar decisión de éxito
                save_agent_decision(opts, instruction_path, &attempts, true);

                return Ok(AgentResult {
                    exit_code: output.status.code().unwrap_or(0),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    elapsed: Duration::default(),
                    attempt,
                    attempts,
                });
            }
            Ok(output) => {
                let code = output.status.code().unwrap_or(-1);
                let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();

                tracing::warn!(
                    "  ✗ agente falló (exit code {code}) — intento {attempt}/{max_retries}"
                );

                let trace = AttemptTrace {
                    attempt,
                    exit_code: code,
                    stdout: stdout_str.clone(),
                    stderr: stderr_str.clone(),
                };
                attempts.push(trace.clone());

                // Guardar decisión de fallo parcial
                save_agent_decision(opts, instruction_path, &attempts, false);

                // Inyectar feedback en el prompt para el siguiente intento
                if opts.inject_feedback && attempt < max_retries {
                    current_prompt = build_feedback_prompt(prompt, &trace);
                }
            }
            Err(e) => {
                let err_msg = format!("{e}");
                tracing::warn!(
                    "  ✗ error invocando agente: {err_msg} — intento {attempt}/{max_retries}"
                );

                let trace = AttemptTrace {
                    attempt,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: err_msg.clone(),
                };
                attempts.push(trace.clone());

                save_agent_decision(opts, instruction_path, &attempts, false);

                if opts.inject_feedback && attempt < max_retries {
                    current_prompt = build_feedback_prompt(prompt, &trace);
                }
            }
        }

        if attempt >= max_retries {
            anyhow::bail!(
                "agotados {max_retries} reintentos invocando {} ({})",
                provider.display_name(),
                instruction_path.display()
            );
        }

        tracing::info!("  ↻ reintentando en {}s...", delay.as_secs());
        std::thread::sleep(delay);
        attempt += 1;
        delay *= 2; // backoff exponencial
    }
}

/// Construye un prompt con feedback del intento fallido.
fn build_feedback_prompt(original_prompt: &str, trace: &AttemptTrace) -> String {
    let feedback = if !trace.stderr.is_empty() {
        &trace.stderr
    } else {
        &trace.stdout
    };

    // Limitar el feedback para no desbordar la ventana de contexto
    let truncated: String = if feedback.len() > 2000 {
        format!(
            "{}...\n[output truncado, {} bytes totales]",
            &feedback[..2000],
            feedback.len()
        )
    } else {
        feedback.clone()
    };

    format!(
        "⚠️  Tu intento anterior (intento {}) falló. Esto fue lo que ocurrió:\n\
         \n\
         ```\n\
         {}\n\
         ```\n\
         \n\
         Corrige el error e inténtalo de nuevo.\n\
         \n\
         ---\n\
         \n\
         {}",
        trace.attempt, truncated, original_prompt
    )
}

/// Guarda la traza de intentos en el directorio de decisiones.
fn save_agent_decision(
    opts: &AgentOptions,
    instruction_path: &Path,
    attempts: &[AttemptTrace],
    success: bool,
) {
    let Some(ref story_id) = opts.story_id else {
        return;
    };
    let Some(ref decisions_dir) = opts.decisions_dir else {
        return;
    };

    let _ = std::fs::create_dir_all(decisions_dir);

    // Derivar el nombre del actor desde el path de instrucciones:
    // .pi/skills/product-owner/SKILL.md → "product-owner"
    // .claude/agents/product_owner.md → "product_owner"
    let actor = instruction_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("agent");

    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let filename = format!("{story_id}-{actor}-{ts}.md");
    let path = decisions_dir.join(&filename);

    let status = if success {
        "✅ Éxito"
    } else {
        "❌ Fallo parcial"
    };
    let mut content = format!("# {story_id} — {actor} — {ts}\n\n## Resultado\n{status}\n\n");

    for trace in attempts {
        content.push_str(&format!(
            "\n### Intento {} (exit code: {})\n\n```\n{}\n```\n",
            trace.attempt, trace.exit_code, trace.stderr
        ));
        if !trace.stdout.is_empty() {
            content.push_str(&format!(
                "\n### stdout (intento {})\n\n```\n{}\n```\n",
                trace.attempt, trace.stdout
            ));
        }
    }

    if let Err(e) = std::fs::write(&path, &content) {
        tracing::warn!("  ⚠️ no se pudo guardar decisión del agente: {e}");
    } else {
        tracing::debug!("  📄 decisión guardada: {}", filename);
    }
}

/// Invoca un agente una sola vez, con timeout.
fn invoke_once(
    provider: &dyn AgentProvider,
    instruction: &Path,
    prompt: &str,
    timeout: Duration,
) -> anyhow::Result<Output> {
    let args = provider.build_args(instruction, prompt);
    let mut child = std::process::Command::new(provider.binary())
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(
                "no se pudo ejecutar '{}': {e}. ¿Está instalado?",
                provider.binary()
            )
        })?;

    let start = std::time::Instant::now();
    let poll = Duration::from_millis(250);
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // El proceso terminó. Leemos la salida capturada.
                let output = child.wait_with_output().map_err(|e| {
                    anyhow::anyhow!("error leyendo salida de '{}': {e}", provider.binary())
                })?;
                return Ok(output);
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    anyhow::bail!(
                        "timeout ({}s) agotado esperando a '{}'",
                        timeout.as_secs(),
                        provider.binary()
                    )
                }
                std::thread::sleep(poll);
            }
            Err(e) => {
                anyhow::bail!("error esperando a '{}': {e}", provider.binary())
            }
        }
    }
}

fn trace_from_output(attempt: u32, output: &Output) -> AttemptTrace {
    AttemptTrace {
        attempt,
        exit_code: output.status.code().unwrap_or(0),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::providers::PiProvider;

    #[test]
    fn build_feedback_prompt_includes_error() {
        let trace = AttemptTrace {
            attempt: 1,
            exit_code: 1,
            stdout: String::new(),
            stderr: "error: no se encontró el archivo src/lib.rs".into(),
        };
        let prompt = build_feedback_prompt("prompt original", &trace);
        assert!(prompt.contains("Tu intento anterior"));
        assert!(prompt.contains("src/lib.rs"));
        assert!(prompt.contains("prompt original"));
    }

    #[test]
    fn build_feedback_prompt_truncates_long_output() {
        let long_err = "x".repeat(3000);
        let trace = AttemptTrace {
            attempt: 2,
            exit_code: 1,
            stdout: String::new(),
            stderr: long_err,
        };
        let prompt = build_feedback_prompt("test", &trace);
        assert!(prompt.contains("truncado"));
        assert!(prompt.len() < 4000); // No debe ser enorme
    }

    #[test]
    fn agent_options_default() {
        let opts = AgentOptions::default();
        assert!(opts.story_id.is_none());
        assert!(opts.decisions_dir.is_none());
        assert!(!opts.inject_feedback);
    }

    #[test]
    #[ignore = "requiere pi instalado"]
    fn invoke_with_retry_fails_when_agent_not_installed() {
        let limits = LimitsConfig {
            max_retries_per_step: 1,
            retry_delay_base_seconds: 0,
            agent_timeout_seconds: 5,
            ..Default::default()
        };
        let opts = AgentOptions::default();
        let provider = PiProvider;
        let result = invoke_with_retry(
            &provider,
            Path::new("/nonexistent/skill.md"),
            "test",
            &limits,
            &opts,
        );
        assert!(result.is_err());
    }
}
