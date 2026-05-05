//! Ejecución de comandos de verificación post-fase (hooks).
//!
//! Los hooks son comandos shell opcionales que se ejecutan tras cada
//! fase del pipeline. Si fallan, se notifica para que el orquestador
//! pueda hacer rollback.

/// Ejecuta un hook de verificación.
///
/// Migrado a tokio: usa `tokio::process::Command` en lugar de
/// `std::process::Command` para no bloquear el runtime async.
/// La firma externa se mantiene síncrona: internamente usa
/// `spawn_blocking` o el runtime global de tokio.
///
/// Retorna `Ok(())` si el comando no está definido o si termina con éxito.
/// Retorna `Err(...)` si el comando falla.
pub fn run_hook(hook: Option<&str>, label: &str) -> anyhow::Result<()> {
    let cmd = match hook {
        Some(c) => c,
        None => {
            tracing::debug!("  sin hook para {label}");
            return Ok(());
        }
    };

    tracing::info!("  🔍 {label}: ejecutando hook...");

    // Usar tokio::process::Command con el runtime global para no bloquear
    // el event loop cuando se llama desde contextos async.
    let status = crate::infra::agent::RUNTIME
        .block_on(async {
            if cfg!(windows) {
                tokio::process::Command::new("cmd")
                    .arg("/c")
                    .arg(cmd)
                    .status()
                    .await
            } else {
                tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .status()
                    .await
            }
        })
        .map_err(|e| anyhow::anyhow!("no se pudo ejecutar hook '{cmd}': {e}"))?;

    if status.success() {
        tracing::info!("  ✓ hook {label} OK");
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        anyhow::bail!("hook '{label}' falló con exit code {code}: {cmd}")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // STORY-012 CA4: run_hook() usa tokio::process::Command
    // ═══════════════════════════════════════════════════════════════

    /// CA4: run_hook con `None` retorna Ok sin ejecutar nada.
    /// Este caso base debe funcionar igual antes y después de la migración.
    #[test]
    fn run_hook_with_none_returns_ok() {
        let result = run_hook(None, "test_hook");
        assert!(result.is_ok(), "None hook siempre retorna Ok");
    }

    /// CA4: run_hook con un comando válido ejecuta correctamente.
    /// Verifica que la migración a tokio no rompe la funcionalidad básica.
    #[test]
    fn run_hook_executes_valid_command() {
        let result = run_hook(Some("true"), "test_valid");
        assert!(result.is_ok(), "hook con comando válido debe retornar Ok");
    }

    /// CA4: run_hook con un comando que falla retorna Err.
    /// El error debe contener el exit code y el label del hook.
    #[test]
    fn run_hook_fails_with_invalid_command() {
        let result = run_hook(Some("exit 42"), "test_fail");
        assert!(
            result.is_err(),
            "hook con comando fallido debe retornar Err"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("test_fail"),
            "error debe contener el label del hook, obtuve: {err}"
        );
        assert!(
            err.contains("42") || err.contains("exit code"),
            "error debe contener el exit code, obtuve: {err}"
        );
    }

    /// CA4: run_hook con un comando de shell (echo) que produce output
    /// no afecta al comportamiento — sigue retornando Ok si exit code es 0.
    #[test]
    fn run_hook_executes_shell_commands() {
        let result = run_hook(Some("echo 'hello from hook'"), "test_echo");
        assert!(
            result.is_ok(),
            "hook con echo debe retornar Ok (exit code 0)"
        );
    }

    /// CA4: run_hook() se puede llamar desde un contexto no-async
    /// (test sin #[tokio::test]). Si la migración usa spawn_blocking
    /// internamente, la firma externa se mantiene síncrona.
    #[test]
    fn run_hook_callable_from_sync_context() {
        // Este test simplemente existe en un contexto no-async.
        // Si run_hook se volviera async sin wrapper síncrono,
        // este test no compilaría.
        let r1 = run_hook(None, "sync_1");
        let r2 = run_hook(Some("true"), "sync_2");
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }

    /// CA4: run_hook no bloquea el event loop cuando se usa desde async.
    /// Si internamente usa tokio::process::Command con spawn_blocking
    /// o el runtime global, la invocación desde async debe ser segura.
    #[tokio::test]
    async fn run_hook_safe_from_async_context() {
        // run_hook es síncrona externamente pero usa RUNTIME.block_on()
        // internamente. Para invocarla desde un contexto async sin causar
        // "Cannot start a runtime from within a runtime", se debe ejecutar
        // dentro de spawn_blocking (hilo separado del worker pool).
        let handle = tokio::task::spawn_blocking(|| run_hook(Some("true"), "async_hook"));
        let result = handle.await.unwrap();
        assert!(
            result.is_ok(),
            "hook debe completar desde async sin bloquear"
        );
    }
}
