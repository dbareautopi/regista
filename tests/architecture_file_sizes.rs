//! Architecture test: file size limits.
//!
//! Enforces a maximum number of production lines per source file.
//! Files that exceed the limit should be split into smaller, focused modules.
//!
//! Only production code is counted — lines inside `#[cfg(test)]` blocks
//! and comment lines are excluded. This allows files with extensive test
//! suites to stay under the limit even if their total line count is high.

mod common;
use common::*;

use std::path::Path;

/// Maximum number of production lines allowed per source file.
///
/// Files exceeding this limit are considered monolithic and should be
/// decomposed into smaller, single-responsibility modules.
const MAX_PRODUCTION_LINES: usize = 1000;

/// Files that are allowed to exceed the limit during the v1.0 transition.
///
/// Each entry should have a STORY reference and planned fix. Remove entries
/// as files are decomposed.
const GRANDFATHERED: &[(&str, &str)] = &[
    // Format: ("relative/path.rs", "STORY-XXX: reason for exemption")
];

/// R0 — no source file shall exceed MAX_PRODUCTION_LINES.
///
/// Scans all `.rs` files under `src/`, counts production lines (excluding
/// `#[cfg(test)]` blocks and comments), and reports violations.
///
/// Grandfathered files (listed in `GRANDFATHERED`) are reported as warnings
/// in the assertion message but do not fail the test — they are acknowledged
/// debt with a migration plan.
#[test]
fn no_source_file_exceeds_max_production_lines() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    if !src_dir.exists() {
        panic!("src/ directory not found at {}", src_dir.display());
    }

    let rs_files = collect_rs_files(&src_dir);
    let mut violations = Vec::new();
    let mut grandfathered_found = Vec::new();

    for file_path in &rs_files {
        let source = read_file_or_panic(file_path);
        let prod_lines = count_production_lines(&source);

        if prod_lines <= MAX_PRODUCTION_LINES {
            continue;
        }

        let relative = file_path.strip_prefix(&src_dir).unwrap_or(file_path);
        let relative_str = relative.to_string_lossy().to_string();

        // Check if this file is grandfathered
        let is_grandfathered = GRANDFATHERED
            .iter()
            .any(|(path, _)| relative_str.contains(path));

        let total_lines = source.lines().count();
        let entry = format!(
            "  {} — {} production lines ({} total)",
            relative_str,
            prod_lines,
            total_lines,
        );

        if is_grandfathered {
            let (_, reason) = GRANDFATHERED
                .iter()
                .find(|(path, _)| relative_str.contains(path))
                .unwrap();
            grandfathered_found.push(format!("{entry}\n    → Grandfathered: {reason}"));
        } else {
            violations.push(entry);
        }
    }

    let mut message = String::new();

    if !violations.is_empty() {
        message.push_str(&format!(
            "❌ Files exceeding {} production lines (limit: {}):\n\n",
            violations.len(),
            MAX_PRODUCTION_LINES,
        ));
        for v in &violations {
            message.push_str(&format!("{v}\n\n"));
        }
        message.push_str(&format!(
            "Fix: decompose these files into smaller, single-responsibility modules.\n\
             If a file cannot be split yet, add it to GRANDFATHERED with a STORY reference.\n\
             Current limit: {} production lines.\n",
            MAX_PRODUCTION_LINES,
        ));
    }

    if !grandfathered_found.is_empty() {
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(&format!(
            "⚠️  Grandfathered files (acknowledged debt, will not fail the test):\n\n"
        ));
        for g in &grandfathered_found {
            message.push_str(&format!("{g}\n\n"));
        }
        message.push_str(
            "Remove grandfathered entries after the referenced STORY is completed.\n",
        );
    }

    if message.is_empty() {
        println!(
            "✅ All {} files under {} production lines",
            rs_files.len(),
            MAX_PRODUCTION_LINES,
        );
        return;
    }

    // Grandfathered files don't fail the test — they're acknowledged debt
    if violations.is_empty() {
        println!("{message}");
        return;
    }

    panic!("{message}");
}
