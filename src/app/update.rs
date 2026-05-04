//! update.rs — Actualización automática de regista desde crates.io.
//!
//! Comprueba si hay una nueva versión disponible y la instala con `cargo install`.

use anyhow::{bail, Context, Result};

/// Devuelve la versión actual del binario en ejecución.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Resultado de la comprobación de actualizaciones.
#[derive(Debug)]
pub struct UpdateStatus {
    pub current: String,
    pub latest: Option<String>,
    pub up_to_date: bool,
}

/// Compara dos versiones semánticas (X.Y.Z) y devuelve true si `a < b`.
fn version_less_than(a: &str, b: &str) -> bool {
    let parse =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };
    let va = parse(a);
    let vb = parse(b);
    let n = va.len().max(vb.len());
    for i in 0..n {
        let na = va.get(i).copied().unwrap_or(0);
        let nb = vb.get(i).copied().unwrap_or(0);
        match na.cmp(&nb) {
            std::cmp::Ordering::Less => return true,
            std::cmp::Ordering::Greater => return false,
            std::cmp::Ordering::Equal => continue,
        }
    }
    false // iguales
}

/// Consulta crates.io para obtener la última versión estable de regista.
fn fetch_latest_version() -> Result<Option<String>> {
    let resp = ureq::get("https://crates.io/api/v1/crates/regista")
        .set("User-Agent", "regista-update-check/1.0")
        .call()
        .context("No se pudo contactar con crates.io. ¿Hay conexión a internet?")?;

    let json: serde_json::Value = resp
        .into_json()
        .context("No se pudo interpretar la respuesta de crates.io")?;

    // max_stable_version es la versión estable más reciente (no yanked)
    let version = json["crate"]["max_stable_version"]
        .as_str()
        .or_else(|| json["crate"]["max_version"].as_str())
        .map(|s| s.to_string());

    Ok(version)
}

/// Comprueba si hay una versión más reciente en crates.io.
pub fn check() -> Result<UpdateStatus> {
    let current = current_version().to_string();
    let latest = fetch_latest_version()?;

    let up_to_date = latest
        .as_ref()
        .is_none_or(|l| !version_less_than(&current, l));

    Ok(UpdateStatus {
        current,
        latest,
        up_to_date,
    })
}

/// Ejecuta la actualización: comprueba, pregunta (si no --yes) e instala.
pub fn run(auto_yes: bool) -> Result<()> {
    let status = check()?;

    println!("regista v{}", status.current);

    match &status.latest {
        None => {
            println!("No se pudo determinar la última versión disponible.");
        }
        Some(latest) if status.up_to_date => {
            println!("✅ Ya tienes la última versión (v{latest}).");
        }
        Some(latest) => {
            println!(
                "🔔 Nueva versión disponible: v{latest} (instalada: v{})",
                status.current
            );

            if !auto_yes {
                use std::io::{self, Write};
                print!("¿Instalar ahora? [y/N]: ");
                io::stdout().flush().ok();
                let mut input = String::new();
                io::stdin().read_line(&mut input).ok();
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelado.");
                    return Ok(());
                }
            }

            println!("📦 Instalando regista v{latest}...");

            let install_status = std::process::Command::new("cargo")
                .args(["install", "regista", "--version", latest])
                .status()
                .context("No se pudo ejecutar 'cargo install'. ¿Está cargo instalado?")?;

            if install_status.success() {
                println!("✅ regista actualizado a v{latest}");
            } else {
                bail!(
                    "'cargo install' falló con código: {}",
                    install_status.code().unwrap_or(-1)
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_parses() {
        let v = current_version();
        assert!(!v.is_empty());
        // Debe tener formato X.Y.Z
        assert!(v.split('.').count() >= 2);
    }

    #[test]
    fn version_comparison() {
        assert!(version_less_than("0.1.0", "0.2.0"));
        assert!(version_less_than("0.5.0", "0.10.0"));
        assert!(version_less_than("1.0.0", "2.0.0"));
        assert!(!version_less_than("0.5.0", "0.5.0"));
        assert!(!version_less_than("1.0.0", "0.9.0"));
        assert!(version_less_than("0.5.0", "0.5.1"));
        assert!(!version_less_than("0.5.1", "0.5.0"));
    }
}
