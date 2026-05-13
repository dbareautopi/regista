//! Shared helpers for architecture compliance tests.
//!
//! This module is not compiled as a test binary itself (it lives in a
//! subdirectory). Other test files import it via `mod common; use common::*;`.

use std::fs;
use std::path::{Path, PathBuf};

// ═══════════════════════════════════════════════════════════════════════════
// File collection
// ═══════════════════════════════════════════════════════════════════════════

/// Recursively collects all .rs files from a directory, sorted.
pub fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir(dir, &mut files);
    files.sort();
    files
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, files);
            } else if path.extension().map_or(false, |e| e == "rs") {
                files.push(path);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// #[cfg(test)] block stripping
// ═══════════════════════════════════════════════════════════════════════════

/// Classification of a source line for architecture enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    /// Production code — subject to architecture rules.
    Production,
    /// Inside a `#[cfg(test)]` block — exempt from architecture rules.
    Test,
    /// Comment line (`//` or inside `/* */`) — exempt.
    Comment,
}

/// Processes source lines, marking each as Production, Test, or Comment.
///
/// Returns `Vec<(1-based line number, line content, LineKind)>`.
///
/// Handles:
/// - `#[cfg(test)] mod tests { ... }` — entire module body is Test
/// - `#[cfg(test)] fn test_foo() { ... }` — entire function body is Test
/// - Nested `#[cfg(test)]` blocks — correctly tracked via brace depth
/// - Line comments (`//`) — marked as Comment
/// - Block comments (`/* */`) — lines inside are marked as Comment
pub fn classify_lines(source: &str) -> Vec<(usize, String, LineKind)> {
    let mut result = Vec::new();
    let mut brace_depth: i32 = 0;
    let mut skip_depth: i32 = -1;
    let mut saw_cfg_test = false;
    let mut in_block_comment = false;

    for (i, line) in source.lines().enumerate() {
        let line_no = i + 1;
        let trimmed = line.trim();

        // ── Block comment tracking ──────────────────────────────
        if in_block_comment {
            let kind = LineKind::Comment;
            result.push((line_no, line.to_string(), kind));
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            in_block_comment = true;
            if trimmed.contains("*/") && !trimmed.ends_with("*/") {
                // Comment starts and ends on same line — but we still mark as Comment
            } else if trimmed.contains("*/") {
                // Single-line block comment
                result.push((line_no, line.to_string(), LineKind::Comment));
                in_block_comment = false;
                continue;
            }
            result.push((line_no, line.to_string(), LineKind::Comment));
            continue;
        }

        // ── Line comment ────────────────────────────────────────
        if trimmed.starts_with("//") || trimmed.starts_with("//!") {
            result.push((line_no, line.to_string(), LineKind::Comment));
            continue;
        }

        // ── Empty lines in test blocks ──────────────────────────
        if trimmed.is_empty() && skip_depth >= 0 {
            result.push((line_no, line.to_string(), LineKind::Test));
            continue;
        }

        // ── #[cfg(test)] detection ──────────────────────────────
        if skip_depth < 0 && trimmed.starts_with("#[cfg(test)]") {
            saw_cfg_test = true;
            result.push((line_no, line.to_string(), LineKind::Test));
            continue;
        }

        if saw_cfg_test {
            // Allow chained attributes (#[something]) before the actual block
            if trimmed.starts_with("#[") {
                result.push((line_no, line.to_string(), LineKind::Test));
                continue;
            }
            if trimmed.is_empty() {
                result.push((line_no, line.to_string(), LineKind::Test));
                continue;
            }
            // This is the start of the test item (mod, fn, etc.)
            skip_depth = brace_depth;
            saw_cfg_test = false;
        }

        // ── Brace depth tracking ────────────────────────────────
        for ch in line.chars() {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
            }
        }

        // ── Determine kind ──────────────────────────────────────
        let kind = if skip_depth >= 0 {
            if brace_depth <= skip_depth {
                skip_depth = -1;
                LineKind::Test
            } else {
                LineKind::Test
            }
        } else {
            LineKind::Production
        };

        result.push((line_no, line.to_string(), kind));
    }

    result
}

/// Counts production lines (lines outside `#[cfg(test)]` blocks and comments).
pub fn count_production_lines(source: &str) -> usize {
    classify_lines(source)
        .iter()
        .filter(|(_, _, kind)| *kind == LineKind::Production)
        .count()
}

/// Returns only the production lines as a single string (for regex scanning).
/// Test and comment lines are replaced with empty lines to preserve line numbers.
pub fn production_lines_only(source: &str) -> String {
    classify_lines(source)
        .iter()
        .map(|(_, line, kind)| match kind {
            LineKind::Production => line.clone(),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the content of a source file with `#[cfg(test)]` blocks and comments
/// stripped out. Test regions are replaced with blank lines to preserve
/// line numbers for error reporting.
pub fn strip_non_production(source: &str) -> String {
    production_lines_only(source)
}

// ═══════════════════════════════════════════════════════════════════════════
// Simple helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Returns the filename portion of a path (e.g. "pipeline.rs" from "src/app/pipeline.rs").
pub fn filename(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Reads a file and returns its content, panicking with a descriptive message on error.
pub fn read_file_or_panic(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}
