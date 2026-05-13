//! Architecture test: config layer (R5).
//!
//! R5 — config/ must only depend on std + serde + toml.
//! It must NOT reference any other crate layer, neither via `use crate::`
//! (caught by `architecture.rs`) nor via fully-qualified paths like
//! `crate::infra::providers::from_name()` (caught HERE).
//!
//! Self-references (`crate::config::`) are allowed because config may be
//! split into submodules in the future.

mod common;
use common::*;

use std::path::Path;

/// R5 — config must not reference any other crate layer.
///
/// Scans `src/config.rs` (and `src/config/mod.rs` if it exists) for
/// *any* reference to `crate::infra`, `crate::domain`, `crate::app`, or
/// `crate::cli` — whether via `use` statements or fully-qualified paths
/// in function bodies.
///
/// `crate::config::` self-references are permitted (for future submodule split).
#[test]
fn config_does_not_reference_other_crate_layers() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let config_path = src_dir.join("config.rs");

    let config_files: Vec<std::path::PathBuf> = if config_path.exists() {
        vec![config_path]
    } else {
        let config_mod = src_dir.join("config").join("mod.rs");
        if config_mod.exists() {
            vec![config_mod]
        } else {
            println!("⏭️  config.rs not found — skipping test");
            return;
        }
    };

    let mut violations = Vec::new();

    for config_file in &config_files {
        let source = read_file_or_panic(config_file);

        // Use classify_lines to skip #[cfg(test)] blocks and comments
        let classified = classify_lines(&source);

        for (line_no, line, kind) in &classified {
            // Only check production lines
            if *kind != LineKind::Production {
                continue;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for fully-qualified crate:: references to other layers
            for forbidden_layer in &["infra", "domain", "app", "cli"] {
                let needle = format!("crate::{}::", forbidden_layer);

                if trimmed.contains(&needle) {
                    // Allow self-references: crate::config::
                    if *forbidden_layer == "config" {
                        continue;
                    }

                    let relative = config_file
                        .strip_prefix(&src_dir)
                        .unwrap_or(config_file);

                    violations.push(format!(
                        "{}:{} — references `crate::{}::`\n  → line: {}\n  → config/ must not depend on other crate layers (R5).",
                        relative.display(),
                        line_no,
                        forbidden_layer,
                        trimmed,
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ R5 violation: config/ references other crate layers:\n{}\n\n\
         Fix: move resolution logic out of config.rs into app/ or infra/. \
         config/ should only contain data structs with serde derives.",
        violations.join("\n\n")
    );
}
