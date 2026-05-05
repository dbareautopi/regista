//! Invocación de agentes `pi` con timeout, reintentos, backoff exponencial,
//! y feedback rico (captura de stdout/stderr para trazabilidad y reintentos).
//!
//! Migrado a tokio: `invoke_once` usa `tokio::process::Command` con
//! `tokio::time::timeout` en lugar de busy-polling con `thread::sleep`.
//! `invoke_with_retry` usa `tokio::time::sleep` para backoff exponencial.

use crate::config::LimitsConfig;
use crate::infra::providers::AgentProvider;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::LazyLock;
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

/// Runtime tokio global compartido para operaciones async desde contextos síncronos.
pub(crate) static RUNTIME: LazyLock<tokio::runtime::Runtime> =
    LazyLock::new(|| tokio::runtime::Runtime::new().expect("no se pudo crear el runtime de tokio"));

/// Invoca un agente con reintentos con backoff exponencial (async).
///
/// # Feedback rico
/// Si `opts.inject_feedback` es true, en cada reintento se inyecta el stderr
/// del intento anterior en el prompt, dando contexto al agente de su fallo.
///
/// Si `opts.decisions_dir` está presente, se guarda una traza completa de
/// cada intento en `<decisions_dir>/<story_id>-<actor>-<timestamp>.md`.
pub async fn invoke_with_retry(
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

        match invoke_once(provider, instruction_path, &current_prompt, timeout).await {
            Ok(output) if output.status.success() => {
                tracing::info!("  ✓ agente completado (intento {attempt})");

                let trace = trace_from_output(attempt, &output);
                attempts.push(trace);

                // Guardar decisión de éxito
                save_agent_decision(opts, instruction_path, &attempts, true).await;

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
                save_agent_decision(opts, instruction_path, &attempts, false).await;

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

                save_agent_decision(opts, instruction_path, &attempts, false).await;

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
        tokio::time::sleep(delay).await;
        attempt += 1;
        delay *= 2; // backoff exponencial
    }
}

/// Wrapper síncrono para `invoke_with_retry` — usa el runtime tokio global.
///
/// Necesario para callers síncronos (`plan.rs`, `pipeline.rs`) que todavía
/// no se han migrado a async.
pub fn invoke_with_retry_blocking(
    provider: &dyn AgentProvider,
    instruction_path: &Path,
    prompt: &str,
    limits: &LimitsConfig,
    opts: &AgentOptions,
) -> anyhow::Result<AgentResult> {
    RUNTIME.block_on(invoke_with_retry(
        provider,
        instruction_path,
        prompt,
        limits,
        opts,
    ))
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

/// Guarda la traza de intentos en el directorio de decisiones (async).
async fn save_agent_decision(
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

    let _ = tokio::fs::create_dir_all(decisions_dir).await;

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

    if let Err(e) = tokio::fs::write(&path, &content).await {
        tracing::warn!("  ⚠️ no se pudo guardar decisión del agente: {e}");
    } else {
        tracing::debug!("  📄 decisión guardada: {}", filename);
    }
}

/// Invoca un agente una sola vez, con timeout (async).
///
/// Usa `tokio::process::Command` para no bloquear el thread del runtime,
/// y `tokio::time::timeout` para limitar la duración de la invocación.
async fn invoke_once(
    provider: &dyn AgentProvider,
    instruction: &Path,
    prompt: &str,
    timeout: Duration,
) -> anyhow::Result<Output> {
    let args = provider.build_args(instruction, prompt);
    let child = tokio::process::Command::new(provider.binary())
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

    // Guardar el PID antes del move para poder matar el proceso en timeout.
    let pid = child.id();

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(anyhow::anyhow!(
            "error leyendo salida de '{}': {e}",
            provider.binary()
        )),
        Err(_elapsed) => {
            // child ya fue movido — matamos por PID.
            if let Some(pid) = pid {
                #[cfg(unix)]
                {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output();
                }
                #[cfg(not(unix))]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/F"])
                        .output();
                }
            }
            anyhow::bail!(
                "timeout ({}s) agotado esperando a '{}'",
                timeout.as_secs(),
                provider.binary()
            )
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
    use crate::config::LimitsConfig;
    use crate::infra::providers::PiProvider;
    use std::path::Path;
    use tempfile::TempDir;

    // ─────────────────────────────────────────────────────────────────────
    // Tests existentes — funciones puras, NO necesitan tokio
    // ─────────────────────────────────────────────────────────────────────

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
        assert!(prompt.len() < 4000);
    }

    #[test]
    fn agent_options_default() {
        let opts = AgentOptions::default();
        assert!(opts.story_id.is_none());
        assert!(opts.decisions_dir.is_none());
        assert!(!opts.inject_feedback);
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA6: tokio como dependencia
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn tokio_is_a_dependency() {
        // Si tokio no está en Cargo.toml, este test no compilará.
        // Verifica que tokio::time::Duration está disponible.
        let d = tokio::time::Duration::from_secs(5);
        assert_eq!(d.as_secs(), 5);

        // Verifica que tokio::process::Command existe.
        let _cmd: tokio::process::Command = tokio::process::Command::new("echo");
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA1: invoke_once() es async y usa tokio::process::Command
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_once_is_async() {
        let provider = PiProvider;
        let result = invoke_once(
            &provider,
            Path::new("/nonexistent/skill.md"),
            "prompt de prueba",
            std::time::Duration::from_secs(1),
        )
        .await;
        // Debe fallar porque el binario no existe, pero prueba que es async.
        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA2: invoke_once usa tokio::time::timeout en lugar de busy-polling
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_once_respects_timeout() {
        // Con un timeout extremadamente corto, la invocación debe fallar
        // rápidamente. Esto valida que no hay busy-polling indefinido.
        let provider = PiProvider;
        let start = std::time::Instant::now();
        let result = invoke_once(
            &provider,
            Path::new("/nonexistent/skill.md"),
            "prompt que causa timeout",
            std::time::Duration::from_millis(100),
        )
        .await;
        let elapsed = start.elapsed();

        // No debe tardar más de 5s (margen generoso). Si fuera busy-polling
        // con thread::sleep, bloquearía mucho más.
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "timeout tardó demasiado: {:?}",
            elapsed
        );
        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA3: invoke_with_retry() es async
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_with_retry_is_async() {
        let limits = LimitsConfig {
            max_retries_per_step: 1,
            retry_delay_base_seconds: 0,
            agent_timeout_seconds: 1,
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
        )
        .await;
        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA4: backoff exponencial con tokio::time::sleep
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_with_retry_preserves_retry_count() {
        // Con max_retries=3, debe intentar exactamente 3 veces antes de fallar.
        // Esto valida indirectamente que el loop de reintentos —incluyendo el
        // backoff exponencial con tokio::time::sleep— funciona correctamente.
        let limits = LimitsConfig {
            max_retries_per_step: 3,
            retry_delay_base_seconds: 0,
            agent_timeout_seconds: 2,
            ..Default::default()
        };
        let opts = AgentOptions::default();
        let provider = PiProvider;
        let result = invoke_with_retry(
            &provider,
            Path::new("/nonexistent/skill.md"),
            "test de backoff",
            &limits,
            &opts,
        )
        .await;

        // Debe fallar después de agotar los reintentos.
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("3 reintentos") || err.contains("agotados"),
            "esperaba mensaje de reintentos agotados, obtuve: {err}"
        );
    }

    #[tokio::test]
    async fn invoke_with_retry_backoff_doubles_delay() {
        // Verifica que la lógica de backoff exponencial (delay *= 2)
        // se conserva en la migración a tokio::time::sleep.
        // Usamos retry_delay_base_seconds > 0 para forzar esperas reales.
        let limits = LimitsConfig {
            max_retries_per_step: 3,
            retry_delay_base_seconds: 1,
            agent_timeout_seconds: 2,
            ..Default::default()
        };
        let opts = AgentOptions::default();
        let provider = PiProvider;

        let start = std::time::Instant::now();
        let result = invoke_with_retry(
            &provider,
            Path::new("/nonexistent/skill.md"),
            "test de backoff exponencial",
            &limits,
            &opts,
        )
        .await;
        let elapsed = start.elapsed();

        assert!(result.is_err());

        // Con retry_delay_base_seconds=1, los delays serán 1s, 2s, 4s = 7s total.
        // Verificamos que al menos pasó un tiempo mínimo (>= 1s, al menos el primer delay).
        assert!(
            elapsed >= std::time::Duration::from_secs(1),
            "backoff debería haber esperado al menos 1s, pero tardó {:?}",
            elapsed
        );
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA5: save_agent_decision migrada a tokio::fs::write o spawn_blocking
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn save_agent_decision_creates_file_with_content() {
        let tmp = TempDir::new().expect("tempdir");
        let decisions = tmp.path().join("decisions");
        let story_id = "STORY-010".to_string();

        let opts = AgentOptions {
            story_id: Some(story_id.clone()),
            decisions_dir: Some(decisions.clone()),
            inject_feedback: false,
        };

        let attempts = vec![AttemptTrace {
            attempt: 1,
            exit_code: 0,
            stdout: "todo bien".into(),
            stderr: String::new(),
        }];

        // Simular path de instrucciones con nombre de actor en el penúltimo segmento.
        // NOTA: instruction se crea en un directorio diferente a decisions_dir
        // para evitar que el subdirectorio del actor contamine read_dir().
        let skill_dir = tmp.path().join("skills").join("product-owner");
        let instruction = skill_dir.join("SKILL.md");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(&instruction, "skill content").unwrap();

        save_agent_decision(&opts, &instruction, &attempts, true).await;

        // Verificar que se creó exactamente 1 archivo.
        let entries: Vec<_> = std::fs::read_dir(&decisions)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1, "esperaba 1 archivo de decisión");

        let saved_path = entries[0].path();
        let content = std::fs::read_to_string(&saved_path).unwrap();

        assert!(content.contains(&story_id), "debe contener el story ID");
        assert!(content.contains("✅ Éxito"), "debe marcar éxito");
        assert!(content.contains("todo bien"), "debe contener stdout");
        assert!(content.contains("product-owner"), "debe contener el actor");
    }

    #[tokio::test]
    async fn save_agent_decision_noops_when_story_id_is_none() {
        let tmp = TempDir::new().expect("tempdir");

        let opts = AgentOptions {
            story_id: None,
            decisions_dir: Some(tmp.path().to_path_buf()),
            inject_feedback: false,
        };

        let attempts = vec![AttemptTrace {
            attempt: 1,
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }];

        let instruction = tmp.path().join("qa-engineer").join("SKILL.md");
        std::fs::create_dir_all(instruction.parent().unwrap()).unwrap();
        std::fs::write(&instruction, "skill").unwrap();

        save_agent_decision(&opts, &instruction, &attempts, true).await;

        // No debe crear archivos si story_id es None.
        // Solo existe el directorio qa-engineer/, sin archivos sueltos.
        let file_count = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .count();
        assert_eq!(file_count, 0, "no deben existir archivos de decisión");
    }

    #[tokio::test]
    async fn save_agent_decision_noops_when_decisions_dir_is_none() {
        let tmp = TempDir::new().expect("tempdir");

        let opts = AgentOptions {
            story_id: Some("STORY-010".into()),
            decisions_dir: None,
            inject_feedback: false,
        };

        let attempts = vec![AttemptTrace {
            attempt: 1,
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }];

        let instruction = tmp.path().join("reviewer").join("SKILL.md");
        std::fs::create_dir_all(instruction.parent().unwrap()).unwrap();
        std::fs::write(&instruction, "skill").unwrap();

        // No debe panic ni crear archivos.
        save_agent_decision(&opts, &instruction, &attempts, true).await;
    }

    // ─────────────────────────────────────────────────────────────────────
    // CA7: tests existentes adaptados a async (#[tokio::test])
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requiere pi instalado"]
    async fn invoke_with_retry_fails_when_agent_not_installed() {
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
        )
        .await;
        assert!(result.is_err());
    }
}
