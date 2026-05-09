//! Architecture compliance tests for regista v1.0.
//!
//! These tests verify that module dependencies follow the layered architecture.
//!
//! Rules:
//!   R1: domain/ → only std + external crates + other domain modules
//!                  (no crate::infra, crate::app, crate::cli, crate::config)
//!   R2: infra/  → only config + other infra modules
//!                  (no crate::domain, crate::app, crate::cli)
//!   R3: app/    → only domain, infra, config
//!                  (no crate::cli)
//!   R4: cli/    → anything (outermost layer, no restrictions)
//!   R5: config  → only std + serde + toml
//!                  (no crate::* imports from any layer)
//!
//! The mega-test `architecture_layers_are_respected` covers R1–R5 automatically
//! by scanning all source files. Five additional targeted tests verify
//! specific policies that the mega-test alone cannot catch (e.g. cycles,
//! infra/llm/ importing domain, app/presets/ importing infra/llm/).
//!
//! v1.0 transition notes:
//!   - `root_file_layer()` contains both legacy (v0.x) and new (v1.0) names.
//!     Legacy entries marked with `—v0.x` are removed in Phase 5 cleanup.
//!   - `infra/llm/` and `app/presets/` are detected by directory path and
//!     mapped to Infra and App respectively.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

// ═══════════════════════════════════════════════════════════════════════════
// Layer definitions
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Layer {
    Cli,
    App,
    Domain,
    Infra,
    Config,
    /// main.rs — can import anything
    Main,
}

impl Layer {
    /// Returns the set of layers this layer is allowed to import from
    /// (excluding its own layer — same-layer imports are always allowed).
    fn allowed_imports(self) -> HashSet<Layer> {
        match self {
            Layer::Cli => [
                Layer::App,
                Layer::Domain,
                Layer::Infra,
                Layer::Config,
            ]
            .into_iter()
            .collect(),
            Layer::App => [Layer::Domain, Layer::Infra, Layer::Config]
                .into_iter()
                .collect(),
            Layer::Domain => {
                // Domain must not import anything from the crate except other domain modules
                HashSet::new()
            }
            Layer::Infra => {
                // Infra can import config and other infra modules
                [Layer::Config].into_iter().collect()
            }
            Layer::Config => HashSet::new(),
            Layer::Main => panic!("Layer::Main has no import restrictions"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Layer::Cli => "cli",
            Layer::App => "app",
            Layer::Domain => "domain",
            Layer::Infra => "infra",
            Layer::Config => "config",
            Layer::Main => "main",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Mapping: root-level modules → their target layer
// ═══════════════════════════════════════════════════════════════════════════

/// Maps root-level .rs filenames (without .rs) to their target layer.
///
/// This is a fallback for files not under a recognized subdirectory.
/// Files inside cli/, app/, domain/, infra/ are detected by path prefix.
///
/// Legacy entries marked `—v0.x` exist for transition compatibility
/// and are removed in Phase 5 cleanup.
fn root_file_layer(module: &str) -> Layer {
    match module {
        // ── Domain ──────────────────────────────────────────────────
        // v1.0
        "state" | "deadlock" | "graph" | "templates" | "task"
        | "workflow" => Layer::Domain,
        // —v0.x (remove in Phase 5)
        "story" | "dependency_graph" | "prompts" => Layer::Domain,

        // ── Infrastructure ─────────────────────────────────────────
        // v1.0
        "daemon" | "checkpoint" | "git" | "hooks" => Layer::Infra,
        // —v0.x (remove in Phase 5)
        "providers" | "agent" => Layer::Infra,

        // ── Application ────────────────────────────────────────────
        // v1.0
        "pipeline" | "plan" | "board" | "init" | "validate" | "health"
        | "update" => Layer::App,
        // —v0.x (remove in Phase 5)
        "orchestrator" | "validator" => Layer::App,

        // ── Root ───────────────────────────────────────────────────
        "config" => Layer::Config,
        "main" => Layer::Main,

        // Unknown modules → treat as outermost (can import anything)
        _ => Layer::Cli,
    }
}

/// Determines the layer of a source file based on its path.
fn file_layer(path: &Path) -> (Layer, String) {
    let path_str = path.to_string_lossy();

    // Target structure: check directory prefix
    if path_str.contains("/cli/") {
        return (Layer::Cli, "cli".to_string());
    }
    if path_str.contains("/app/") {
        return (Layer::App, "app".to_string());
    }
    if path_str.contains("/domain/") {
        return (Layer::Domain, "domain".to_string());
    }
    if path_str.contains("/infra/") {
        return (Layer::Infra, "infra".to_string());
    }

    // Legacy flat structure: determine from filename
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    (root_file_layer(stem), format!("{stem}.rs"))
}

// ═══════════════════════════════════════════════════════════════════════════
// Import extraction
// ═══════════════════════════════════════════════════════════════════════════

/// Extracts the first path segment after `crate::` from an import line.
/// Handles:
///   use crate::foo::bar;
///   use crate::foo::{bar, baz};
///   use crate::foo;
fn extract_crate_import(use_line: &str) -> Option<String> {
    let line = use_line.trim();

    if !line.starts_with("use ") {
        return None;
    }

    let rest = line.strip_prefix("use ")?;
    let after_crate = rest.strip_prefix("crate::")?;

    let first_segment = after_crate
        .split(|c: char| c == ':' || c == '{' || c == ';' || c == ' ' || c == '\n')
        .next()?;

    if first_segment.is_empty() {
        return None;
    }

    Some(first_segment.to_string())
}

/// Collects all `use crate::X` imports from a source file.
/// Skips lines inside #[cfg(test)]-gated blocks (test-only deps are exempt).
fn collect_imports(source: &str) -> Vec<(usize, String)> {
    let mut imports = Vec::new();
    let mut skip_depth: i32 = -1;
    let mut brace_depth: i32 = 0;
    let mut saw_cfg_test = false;

    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if skip_depth < 0 && trimmed.starts_with("#[cfg(test)]") {
            saw_cfg_test = true;
            continue;
        }

        if saw_cfg_test {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                continue;
            }
            skip_depth = brace_depth;
            saw_cfg_test = false;
        }

        for ch in line.chars() {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
            }
        }

        if skip_depth >= 0 {
            if brace_depth <= skip_depth {
                skip_depth = -1;
            }
            continue;
        }

        if let Some(mod_name) = extract_crate_import(trimmed) {
            imports.push((i + 1, mod_name));
        }
    }

    imports
}

// ═══════════════════════════════════════════════════════════════════════════
// Mega-test: all layers (R1–R5)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn architecture_layers_are_respected() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    if !src_dir.exists() {
        panic!("src/ directory not found at {}", src_dir.display());
    }

    let mut violations: Vec<String> = Vec::new();
    let mut files_checked = 0;

    let rs_files = collect_rs_files(&src_dir);
    let module_map = build_module_layer_map(&rs_files, &src_dir);

    for file_path in &rs_files {
        files_checked += 1;

        let (layer, identifier) = file_layer(file_path);
        if layer == Layer::Main {
            continue;
        }

        let source = match fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(e) => {
                violations.push(format!("Cannot read {}: {e}", file_path.display()));
                continue;
            }
        };

        let imports = collect_imports(&source);

        for (line_no, imported_module) in imports {
            let imported_layer = module_map
                .get(&imported_module)
                .copied()
                .unwrap_or(Layer::Cli);

            let allowed = layer.allowed_imports();
            let is_same_layer = layer == imported_layer;

            if !is_same_layer && !allowed.contains(&imported_layer) {
                violations.push(format!(
                    "{}:{} — layer `{}` ({}) imports `{}` (layer `{}`) — NOT ALLOWED\n  → line: {}",
                    file_path
                        .strip_prefix(&src_dir)
                        .unwrap_or(file_path)
                        .display(),
                    line_no,
                    layer.name(),
                    identifier,
                    imported_module,
                    imported_layer.name(),
                    source.lines().nth(line_no - 1).unwrap_or("").trim(),
                ));
            }
        }
    }

    if !violations.is_empty() {
        let mut msg = format!(
            "\n❌ Architecture violations found: {}\n",
            violations.len()
        );
        msg.push_str(&"=".repeat(80));
        msg.push('\n');

        let domain_violations: Vec<_> = violations
            .iter()
            .filter(|v| v.contains("domain") || v.contains("Domain"))
            .collect();
        let infra_violations: Vec<_> = violations
            .iter()
            .filter(|v| v.contains("infra") || v.contains("Infra"))
            .collect();
        let app_violations: Vec<_> = violations
            .iter()
            .filter(|v| v.contains("app") || v.contains("App"))
            .collect();

        if !domain_violations.is_empty() {
            msg.push_str("\n── R1 violations: domain/ imports forbidden modules ──\n");
            for v in domain_violations {
                msg.push_str(&format!("{v}\n\n"));
            }
        }
        if !infra_violations.is_empty() {
            msg.push_str("\n── R2 violations: infra/ imports forbidden modules ──\n");
            for v in infra_violations {
                msg.push_str(&format!("{v}\n\n"));
            }
        }
        if !app_violations.is_empty() {
            msg.push_str("\n── R3 violations: app/ imports forbidden modules ──\n");
            for v in app_violations {
                msg.push_str(&format!("{v}\n\n"));
            }
        }

        msg.push_str(&format!("\nFiles checked: {}\n", files_checked));
        msg.push_str("Fix: move modules to their target directories and update imports.\n");
        msg.push_str("See docs/architecture.md for the target structure.\n");

        panic!("{msg}");
    }

    println!("✅ Architecture OK — {files_checked} files checked, 0 violations");
}

// ═══════════════════════════════════════════════════════════════════════════
// Targeted tests — specific policies that the mega-test cannot catch
// ═══════════════════════════════════════════════════════════════════════════

/// Test A — infra/llm/ must not import domain.
///
/// `infra/llm/types.rs` defines `Message`, `ChatResponse`. It would be
/// tempting to import `domain::task::Task` for serialization — that breaks R2.
/// This test is forward-compatible: it passes if infra/llm/ does not exist yet.
#[test]
fn infra_llm_does_not_import_domain() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let llm_dir = src_dir.join("infra").join("llm");

    if !llm_dir.exists() {
        println!("⏭️  infra/llm/ does not exist yet — skipping test");
        return;
    }

    let mut violations = Vec::new();

    for entry in fs::read_dir(&llm_dir).expect("cannot read infra/llm/") {
        let path = entry.expect("cannot read dir entry").path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }

        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));

        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use crate::domain") {
                violations.push(format!(
                    "{}:{} — {}\n  → infra/llm/ imports domain, violates R2",
                    path.file_name().unwrap().to_string_lossy(),
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ R2 violation: infra/llm/ imports domain:\n{}",
        violations.join("\n")
    );
}

/// Test B — domain/ must not depend on `anyhow`.
///
/// `anyhow` is an infrastructure crate. Domain logic should use typed errors
/// or `std::error::Error` to stay testable without infrastructure deps.
/// If the team decides to keep `anyhow` in domain, this test should be removed.
#[test]
fn domain_does_not_import_anyhow() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let domain_dir = src_dir.join("domain");

    if !domain_dir.exists() {
        return;
    }

    let mut violations = Vec::new();

    for entry in fs::read_dir(&domain_dir).expect("cannot read domain/") {
        let path = entry.expect("cannot read dir entry").path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }
        // mod.rs only re-exports; it's exempt
        if path.file_stem().map_or(false, |s| s == "mod") {
            continue;
        }

        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
        let imports = collect_imports(&source);

        for (line_no, imported_module) in imports {
            if imported_module == "anyhow" {
                violations.push(format!(
                    "{}:{} — use crate::anyhow detected\n  → domain/ should use typed errors, not anyhow.\n  → If intentional, remove this test.",
                    path.strip_prefix(&src_dir).unwrap_or(&path).display(),
                    line_no,
                ));
            }
        }

        // Also check for direct `use anyhow::...` (not via crate::)
        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use anyhow::") {
                violations.push(format!(
                    "{}:{} — {}\n  → domain/ should not depend on anyhow.",
                    path.strip_prefix(&src_dir).unwrap_or(&path).display(),
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    // Also check domain/ mod.rs
    let mod_path = domain_dir.join("mod.rs");
    if mod_path.exists() {
        let source = fs::read_to_string(&mod_path)
            .unwrap_or_else(|e| panic!("cannot read {mod_path:?}: {e}"));
        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use anyhow::") {
                violations.push(format!(
                    "domain/mod.rs:{} — {}\n  → domain/ should not depend on anyhow.",
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ domain/ depends on anyhow (infrastructure crate):\n{}\n\nFix: use typed errors in domain/ or remove this test if anyhow is accepted.",
        violations.join("\n")
    );
}

/// Test C — config must not import any crate module (R5 strict).
///
/// `config.rs` is the data layer. It must only depend on std, serde, toml.
/// Any `use crate::` is a violation.
#[test]
fn config_does_not_import_crate_modules() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let config_path = src_dir.join("config.rs");

    if !config_path.exists() {
        // config might have moved to config/mod.rs in the future
        let config_mod = src_dir.join("config").join("mod.rs");
        if !config_mod.exists() {
            println!("⏭️  config.rs not found — skipping test");
            return;
        }
        check_config_file(&config_mod, &src_dir);
        return;
    }

    check_config_file(&config_path, &src_dir);
}

fn check_config_file(path: &Path, src_dir: &Path) {
    let source = fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
    let mut violations = Vec::new();

    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("use crate::") {
            violations.push(format!(
                "{}:{} — {}\n  → config/ must not import crate modules (R5). Use std + serde + toml only.",
                path.strip_prefix(src_dir).unwrap_or(path).display(),
                line_no + 1,
                trimmed,
            ));
        }
    }

    // config can use `use crate::config::` if it's split into submodules — that's allowed
    let violations: Vec<_> = violations
        .into_iter()
        .filter(|v| !v.contains("use crate::config::"))
        .collect();

    assert!(
        violations.is_empty(),
        "❌ R5 violation: config imports crate modules:\n{}",
        violations.join("\n")
    );
}

/// Test D — app/presets/ must not import infra/llm/.
///
/// Presets are pure data (WorkflowConfig). They should not instantiate
/// LLM providers or depend on infrastructure.
#[test]
fn app_presets_do_not_import_infra() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let presets_dir = src_dir.join("app").join("presets");

    if !presets_dir.exists() {
        println!("⏭️  app/presets/ does not exist yet — skipping test");
        return;
    }

    let mut violations = Vec::new();

    for entry in fs::read_dir(&presets_dir).expect("cannot read app/presets/") {
        let path = entry.expect("cannot read dir entry").path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }

        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));

        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use crate::infra::llm") {
                violations.push(format!(
                    "{}:{} — {}\n  → app/presets/ imports infra/llm/. Presets are data, not infrastructure.",
                    path.strip_prefix(&src_dir).unwrap_or(&path).display(),
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ app/presets/ imports infra/ (presets should be pure data):\n{}",
        violations.join("\n")
    );
}

/// Test E — no circular dependencies between layers.
///
/// The mega-test catches individual violations but not cycles.
/// A cycle means layer A imports layer B and B imports A (transitively).
/// This test builds a directed graph of inter-layer imports and runs DFS.
#[test]
fn layers_are_not_circular() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    if !src_dir.exists() {
        return;
    }

    let rs_files = collect_rs_files(&src_dir);
    let module_map = build_module_layer_map(&rs_files, &src_dir);

    // Build adjacency: layer A → set of layers that A imports
    let mut edges: HashMap<Layer, HashSet<Layer>> = HashMap::new();

    for file_path in &rs_files {
        let (file_layer, _) = file_layer(file_path);
        if file_layer == Layer::Main {
            continue;
        }

        let source = match fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let imports = collect_imports(&source);
        let targets = edges.entry(file_layer).or_default();

        for (_, imported_module) in imports {
            let imported_layer = module_map
                .get(&imported_module)
                .copied()
                .unwrap_or(Layer::Cli);
            // Only record cross-layer imports
            if imported_layer != file_layer {
                targets.insert(imported_layer);
            }
        }
    }

    // Detect cycles with DFS
    let all_layers: Vec<Layer> = vec![
        Layer::Cli,
        Layer::App,
        Layer::Domain,
        Layer::Infra,
        Layer::Config,
    ];

    let mut cycles: Vec<String> = Vec::new();

    for start in &all_layers {
        let mut visited: HashSet<Layer> = HashSet::new();
        let mut stack: Vec<Layer> = Vec::new();
        if dfs_cycle(*start, &edges, &mut visited, &mut stack) {
            cycles.push(format!(
                "Cycle detected: {}",
                stack
                    .iter()
                    .map(|l| l.name())
                    .collect::<Vec<_>>()
                    .join(" → ")
            ));
        }
    }

    assert!(
        cycles.is_empty(),
        "❌ Circular dependencies between layers:\n{}\n\nFix: break the cycle by moving shared types to config/ or introducing a shared-types crate.",
        cycles.join("\n")
    );
}

/// DFS helper for cycle detection.
fn dfs_cycle(
    current: Layer,
    edges: &HashMap<Layer, HashSet<Layer>>,
    visited: &mut HashSet<Layer>,
    stack: &mut Vec<Layer>,
) -> bool {
    if stack.contains(&current) {
        // Found a cycle — but only report if the current node starts the cycle
        return stack.first() == Some(&current);
    }
    if visited.contains(&current) {
        return false;
    }

    visited.insert(current);
    stack.push(current);

    if let Some(targets) = edges.get(&current) {
        for target in targets {
            if dfs_cycle(*target, edges, visited, stack) {
                return true;
            }
        }
    }

    stack.pop();
    false
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Recursively collects all .rs files from a directory.
fn collect_rs_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    walk_dir(dir, &mut files);
    files.sort();
    files
}

fn walk_dir(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
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

/// Builds a map from module name → target layer for all known modules.
fn build_module_layer_map(
    rs_files: &[std::path::PathBuf],
    src_dir: &Path,
) -> HashMap<String, Layer> {
    let mut map = HashMap::new();

    for file in rs_files {
        let (layer, _identifier) = file_layer(file);

        let relative = file.strip_prefix(src_dir).unwrap_or(file);
        let module_name = path_to_module_name(relative);

        map.insert(module_name, layer);

        // Also add just the filename stem for legacy flat structure references
        if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
            map.entry(stem.to_string()).or_insert(layer);
        }
    }

    // Known module prefixes — ensure they are mapped even before they exist
    map.entry("cli".to_string()).or_insert(Layer::Cli);
    map.entry("app".to_string()).or_insert(Layer::App);
    map.entry("domain".to_string()).or_insert(Layer::Domain);
    map.entry("infra".to_string()).or_insert(Layer::Infra);
    map.entry("config".to_string()).or_insert(Layer::Config);

    // v1.0 submodules — map them proactively so forward references resolve
    map.entry("infra::llm".to_string())
        .or_insert(Layer::Infra);
    map.entry("app::presets".to_string())
        .or_insert(Layer::App);

    map
}

/// Converts a relative path like "cli/args.rs" or "domain/state.rs"
/// to a module name like "cli::args" or "domain::state".
fn path_to_module_name(path: &Path) -> String {
    let components: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();

    let mut parts: Vec<String> = Vec::new();
    for comp in &components {
        let stripped = comp.strip_suffix(".rs").unwrap_or(comp);
        if stripped == "mod" {
            continue; // skip mod.rs
        }
        parts.push(stripped.to_string());
    }

    parts.join("::")
}

// ═══════════════════════════════════════════════════════════════════════════
// Unit tests for the test helpers
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_crate_import_simple() {
        assert_eq!(
            extract_crate_import("use crate::state::Status;"),
            Some("state".into())
        );
    }

    #[test]
    fn test_extract_crate_import_braced() {
        assert_eq!(
            extract_crate_import("use crate::state::{Status, Actor};"),
            Some("state".into())
        );
    }

    #[test]
    fn test_extract_crate_import_module_only() {
        assert_eq!(
            extract_crate_import("use crate::providers;"),
            Some("providers".into())
        );
    }

    #[test]
    fn test_extract_crate_import_not_crate() {
        assert_eq!(extract_crate_import("use std::collections::HashMap;"), None);
    }

    #[test]
    fn test_extract_crate_import_no_use() {
        assert_eq!(extract_crate_import("let x = 5;"), None);
    }

    #[test]
    fn test_extract_crate_import_multi_segment() {
        assert_eq!(
            extract_crate_import("use crate::domain::state::Status;"),
            Some("domain".into())
        );
    }

    #[test]
    fn test_collect_imports_skips_test_module() {
        let source = r#"
use crate::state::Status;

#[cfg(test)]
mod tests {
    use crate::infra::daemon;
    use crate::config::Config;
}
"#;
        let imports = collect_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].1, "state");
    }

    #[test]
    fn test_collect_imports_skips_cfg_test_function() {
        let source = r#"
use crate::domain::state::Status;

#[cfg(test)]
fn test_helper() {
    use crate::infra::git::snapshot;
}
"#;
        let imports = collect_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].1, "domain");
    }

    #[test]
    fn test_root_file_layer_mappings() {
        // v1.0 names
        assert_eq!(root_file_layer("state"), Layer::Domain);
        assert_eq!(root_file_layer("task"), Layer::Domain);
        assert_eq!(root_file_layer("workflow"), Layer::Domain);
        assert_eq!(root_file_layer("templates"), Layer::Domain);
        assert_eq!(root_file_layer("daemon"), Layer::Infra);
        assert_eq!(root_file_layer("git"), Layer::Infra);
        assert_eq!(root_file_layer("pipeline"), Layer::App);
        assert_eq!(root_file_layer("validate"), Layer::App);
        assert_eq!(root_file_layer("board"), Layer::App);
        assert_eq!(root_file_layer("config"), Layer::Config);
        assert_eq!(root_file_layer("main"), Layer::Main);

        // Legacy v0.x names (still mapped during transition)
        assert_eq!(root_file_layer("story"), Layer::Domain);
        assert_eq!(root_file_layer("prompts"), Layer::Domain);
        assert_eq!(root_file_layer("providers"), Layer::Infra);
        assert_eq!(root_file_layer("agent"), Layer::Infra);
        assert_eq!(root_file_layer("orchestrator"), Layer::App);
        assert_eq!(root_file_layer("validator"), Layer::App);
    }

    #[test]
    fn test_layer_allowed_imports() {
        // R1: Domain can't import anything
        assert!(Layer::Domain.allowed_imports().is_empty());

        // R2: Infra can only import Config
        let infra_allowed = Layer::Infra.allowed_imports();
        assert!(infra_allowed.contains(&Layer::Config));
        assert!(!infra_allowed.contains(&Layer::Domain));
        assert!(!infra_allowed.contains(&Layer::App));
        assert!(!infra_allowed.contains(&Layer::Cli));

        // R3: App can import Domain, Infra, Config — not Cli
        let app_allowed = Layer::App.allowed_imports();
        assert!(app_allowed.contains(&Layer::Domain));
        assert!(app_allowed.contains(&Layer::Infra));
        assert!(app_allowed.contains(&Layer::Config));
        assert!(!app_allowed.contains(&Layer::Cli));

        // R4: Cli can import anything
        let cli_allowed = Layer::Cli.allowed_imports();
        assert!(cli_allowed.contains(&Layer::App));
        assert!(cli_allowed.contains(&Layer::Domain));
        assert!(cli_allowed.contains(&Layer::Infra));
        assert!(cli_allowed.contains(&Layer::Config));

        // R5: Config can't import anything
        assert!(Layer::Config.allowed_imports().is_empty());
    }

    #[test]
    fn test_path_to_module_name() {
        assert_eq!(path_to_module_name(Path::new("cli/args.rs")), "cli::args");
        assert_eq!(
            path_to_module_name(Path::new("domain/state.rs")),
            "domain::state"
        );
        assert_eq!(path_to_module_name(Path::new("state.rs")), "state");
        // v1.0: nested paths
        assert_eq!(
            path_to_module_name(Path::new("infra/llm/openai.rs")),
            "infra::llm::openai"
        );
        assert_eq!(
            path_to_module_name(Path::new("app/presets/software_dev.rs")),
            "app::presets::software_dev"
        );
    }

    #[test]
    fn test_file_layer_detects_by_directory() {
        assert_eq!(
            file_layer(Path::new("src/cli/args.rs")).0,
            Layer::Cli
        );
        assert_eq!(
            file_layer(Path::new("src/app/pipeline.rs")).0,
            Layer::App
        );
        assert_eq!(
            file_layer(Path::new("src/domain/task.rs")).0,
            Layer::Domain
        );
        assert_eq!(
            file_layer(Path::new("src/infra/llm/openai.rs")).0,
            Layer::Infra
        );
        // v1.0 subdirectories
        assert_eq!(
            file_layer(Path::new("src/infra/llm/mod.rs")).0,
            Layer::Infra
        );
        assert_eq!(
            file_layer(Path::new("src/app/presets/mod.rs")).0,
            Layer::App
        );
    }
}
