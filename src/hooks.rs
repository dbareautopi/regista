//! Ejecución de comandos de verificación post-fase (hooks).
//!
//! Los hooks son comandos shell opcionales que se ejecutan tras cada
//! fase del pipeline. Si fallan, se notifica para que el orquestador
//! pueda hacer rollback.

/// Ejecuta un hook de verificación.
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
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .status()
        .map_err(|e| anyhow::anyhow!("no se pudo ejecutar hook '{cmd}': {e}"))?;

    if status.success() {
        tracing::info!("  ✓ hook {label} OK");
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        anyhow::bail!("hook '{label}' falló con exit code {code}: {cmd}")
    }
}
