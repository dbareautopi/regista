//! Architecture tests: domain layer purity (R1).
//!
//! R1 — domain/ must only depend on std + external crates + other domain modules.
//! It must NOT:
//!   a) Use `anyhow` — domain logic should use typed errors
//!   b) Perform filesystem I/O (`std::fs`, `tokio::fs`) — I/O belongs in infra/
//!
//! These tests catch violations that the mega-test in `architecture.rs` misses:
//! - `anyhow::Result`, `anyhow::anyhow!()`, `anyhow::bail!()` without `use anyhow`
//! - `std::fs::read_to_string()`, `std::fs::write()`, etc. in domain/ source files

mod common;
use common::*;

use std::path::Path;

// ═══════════════════════════════════════════════════════════════════════════
// Test A — domain/ must not use anyhow
// ═══════════════════════════════════════════════════════════════════════════

/// R1 — domain/ must not use `anyhow`.
///
/// `anyhow` is an infrastructure crate. Domain logic should use typed errors
/// or `std::error::Error`. This test catches both `use anyhow::*` statements
/// AND fully-qualified usages like `anyhow::Result`, `anyhow::bail!()`.
///
/// Usage inside `#[cfg(test)]` blocks is exempt (tests may use anyhow freely).
#[test]
fn domain_does_not_use_anyhow() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let domain_dir = src_dir.join("domain");

    if !domain_dir.exists() {
        println!("⏭️  domain/ does not exist — skipping test");
        return;
    }

    let mut violations = Vec::new();

    for entry in std::fs::read_dir(&domain_dir).expect("cannot read domain/") {
        let path = entry.expect("cannot read dir entry").path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        // mod.rs only re-exports; exempt
        if path.file_stem().map_or(false, |s| s == "mod") {
            continue;
        }

        let source = read_file_or_panic(&path);

        // Strip #[cfg(test)] blocks and comments — test code is exempt
        let production = strip_non_production(&source);

        for (line_no, line) in production.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Catch `anyhow::` in any context (fully-qualified paths, function bodies, etc.)
            if trimmed.contains("anyhow::") {
                let relative = path.strip_prefix(&src_dir).unwrap_or(&path);
                violations.push(format!(
                    "{}:{} — uses `anyhow::`\n  → line: {}\n  → domain/ should not depend on anyhow (infrastructure crate). Use typed errors.",
                    relative.display(),
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ R1 violation: domain/ uses anyhow:\n{}\n\n\
         Fix: replace anyhow with typed errors in domain/ modules, \
         or move anyhow-dependent code to app/ or infra/.",
        violations.join("\n\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Test B — domain/ must not perform filesystem I/O
// ═══════════════════════════════════════════════════════════════════════════

/// R1 — domain/ must not do filesystem I/O.
///
/// Domain logic should be pure: receive strings/structs and return
/// strings/structs. Reading from and writing to disk belongs in `infra/`.
///
/// This test catches `std::fs::` and `tokio::fs::` usage in production code.
/// Usage inside `#[cfg(test)]` blocks is exempt.
///
/// Whitelisted: `std::fs::read_to_string(file!())` — the architecture
/// self-test pattern where a module reads its own source to verify imports.
#[test]
fn domain_does_not_do_filesystem_io() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let domain_dir = src_dir.join("domain");

    if !domain_dir.exists() {
        println!("⏭️  domain/ does not exist — skipping test");
        return;
    }

    // Filesystem operations that domain should never perform
    let forbidden_patterns: &[&str] = &[
        "std::fs::",
        "tokio::fs::",
    ];

    let mut violations = Vec::new();

    for entry in std::fs::read_dir(&domain_dir).expect("cannot read domain/") {
        let path = entry.expect("cannot read dir entry").path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        if path.file_stem().map_or(false, |s| s == "mod") {
            continue;
        }

        let source = read_file_or_panic(&path);

        // Strip #[cfg(test)] blocks and comments
        let production = strip_non_production(&source);

        for (line_no, line) in production.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            for pattern in forbidden_patterns {
                if trimmed.contains(pattern) {
                    // Whitelist: self-test pattern reading own source
                    if trimmed.contains("read_to_string(file!())") {
                        continue;
                    }

                    let relative = path.strip_prefix(&src_dir).unwrap_or(&path);
                    violations.push(format!(
                        "{}:{} — uses `{}`\n  → line: {}\n  → domain/ should not perform filesystem I/O. Move I/O to infra/.",
                        relative.display(),
                        line_no + 1,
                        pattern,
                        trimmed,
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ R1 violation: domain/ performs filesystem I/O:\n{}\n\n\
         Fix: extract I/O operations into infra/ modules. \
         Domain functions should receive and return data, not read/write files.",
        violations.join("\n\n")
    );
}
