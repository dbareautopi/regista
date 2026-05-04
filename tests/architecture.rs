//! Architecture compliance tests for regista.
//!
//! These tests verify that module dependencies follow the layered architecture
//! defined in docs/architecture.md.
//!
//! Rules:
//!   R1: domain/ → only std + external crates (no crate::infra, crate::app, crate::cli, crate::config)
//!   R2: infra/  → only config + other infra modules (no crate::domain, crate::app, crate::cli)
//!   R3: app/    → only domain, infra, config (no crate::cli)
//!   R4: cli/    → anything (outermost layer)
//!   R5: config  → nothing from crate (except std)
//!
//! The test works with both legacy flat structure and target directory structure.
//! Root-level .rs files are mapped to their target layer via ROOT_FILE_LAYER.

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
    /// Returns the set of layers this layer is allowed to import from.
    fn allowed_imports(self) -> HashSet<Layer> {
        match self {
            Layer::Cli => [Layer::App, Layer::Domain, Layer::Infra, Layer::Config]
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

/// Maps the current root-level .rs filenames (without .rs) to their target layer.
/// After refactor, these files will live in the corresponding subdirectory.
fn root_file_layer(module: &str) -> Layer {
    match module {
        // Domain
        "state" | "story" | "dependency_graph" | "deadlock" | "prompts" => Layer::Domain,
        // Infrastructure
        "providers" | "agent" | "daemon" | "checkpoint" | "git" | "hooks" => Layer::Infra,
        // Application
        "orchestrator" | "plan" | "validator" | "init" | "board" | "update" => Layer::App,
        // Config
        "config" => Layer::Config,
        // main.rs is special
        "main" => Layer::Main,
        // Unknown modules (should not happen, but be lenient)
        _ => Layer::Cli, // outermost, can import anything
    }
}

/// Determines the layer of a source file based on its path.
/// Works for both legacy flat structure and target directory structure.
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

    // Must start with "use "
    if !line.starts_with("use ") {
        return None;
    }

    // Find "crate::"
    let rest = line.strip_prefix("use ")?;
    let after_crate = rest.strip_prefix("crate::")?;

    // Take the first segment before ::, {, ;, or whitespace
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
/// Uses brace-depth tracking to detect when we exit the gated region.
fn collect_imports(source: &str) -> Vec<(usize, String)> {
    let mut imports = Vec::new();
    let mut skip_depth: i32 = -1; // -1 = not skipping; >=0 = brace depth when we entered
    let mut brace_depth: i32 = 0;
    let mut saw_cfg_test = false;

    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Detect #[cfg(test)] — start skipping on the next item
        if skip_depth < 0 && trimmed.starts_with("#[cfg(test)]") {
            saw_cfg_test = true;
            continue;
        }

        // If we just saw #[cfg(test)], the next non-empty, non-attr line starts the skip region
        if saw_cfg_test {
            if trimmed.is_empty() || trimmed.starts_with("#[") {
                // Empty or another attr — stay in saw_cfg_test state
                if !trimmed.is_empty() {
                    // another attr, still waiting for the item
                }
                continue;
            }
            // This is the item the #[cfg(test)] applies to — start skipping
            skip_depth = brace_depth;
            saw_cfg_test = false;
        }

        // Track brace depth from this line
        for ch in line.chars() {
            if ch == '{' {
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth -= 1;
            }
        }

        // If we're in a skip region, check if we've exited
        if skip_depth >= 0 {
            if brace_depth <= skip_depth {
                skip_depth = -1;
            }
            continue;
        }

        // Extract import
        if let Some(mod_name) = extract_crate_import(trimmed) {
            imports.push((i + 1, mod_name));
        }
    }

    imports
}

// ═══════════════════════════════════════════════════════════════════════════
// The test
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn architecture_layers_are_respected() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    if !src_dir.exists() {
        panic!("src/ directory not found at {}", src_dir.display());
    }

    let mut violations: Vec<String> = Vec::new();
    let mut files_checked = 0;

    // Collect all rs files recursively
    let rs_files = collect_rs_files(&src_dir);
    let module_map = build_module_layer_map(&rs_files, &src_dir);

    for file_path in &rs_files {
        files_checked += 1;

        let (layer, identifier) = file_layer(file_path);
        if layer == Layer::Main {
            continue; // main.rs has no restrictions
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
            // Determine the layer of the imported module
            let imported_layer = module_map
                .get(&imported_module)
                .copied()
                .unwrap_or(Layer::Cli); // unknown modules are treated as outermost

            let allowed = layer.allowed_imports();

            // Special case: domain can import other domain modules
            // Special case: infra can import other infra modules
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

    // Report
    if !violations.is_empty() {
        let mut msg = format!("\n❌ Architecture violations found: {}\n", violations.len());
        msg.push_str(&"=".repeat(80));
        msg.push('\n');

        // Group by rule type
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
/// This is used to resolve the layer of imported modules.
fn build_module_layer_map(
    rs_files: &[std::path::PathBuf],
    src_dir: &Path,
) -> HashMap<String, Layer> {
    let mut map = HashMap::new();

    for file in rs_files {
        let (layer, _identifier) = file_layer(file);

        // For files in target directories, use their full module path
        let relative = file.strip_prefix(src_dir).unwrap_or(file);

        // Build the module name from the path
        let module_name = path_to_module_name(relative);

        map.insert(module_name, layer);

        // Also add just the filename stem for legacy flat structure references
        if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
            map.entry(stem.to_string()).or_insert(layer);
        }
    }

    // Add known module names that might exist only after refactor
    map.entry("cli".to_string()).or_insert(Layer::Cli);
    map.entry("app".to_string()).or_insert(Layer::App);
    map.entry("domain".to_string()).or_insert(Layer::Domain);
    map.entry("infra".to_string()).or_insert(Layer::Infra);
    map.entry("config".to_string()).or_insert(Layer::Config);

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
// Tests for the test
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
        // Only the top-level import should be found
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].1, "state");
    }

    #[test]
    fn test_root_file_layer_mappings() {
        assert_eq!(root_file_layer("state"), Layer::Domain);
        assert_eq!(root_file_layer("providers"), Layer::Infra);
        assert_eq!(root_file_layer("orchestrator"), Layer::App);
        assert_eq!(root_file_layer("config"), Layer::Config);
    }

    #[test]
    fn test_layer_allowed_imports() {
        // Domain can't import anything
        assert!(Layer::Domain.allowed_imports().is_empty());

        // App can import Domain and Infra
        let app_allowed = Layer::App.allowed_imports();
        assert!(app_allowed.contains(&Layer::Domain));
        assert!(app_allowed.contains(&Layer::Infra));
        assert!(app_allowed.contains(&Layer::Config));
        assert!(!app_allowed.contains(&Layer::Cli));

        // Infra can only import Config
        let infra_allowed = Layer::Infra.allowed_imports();
        assert!(infra_allowed.contains(&Layer::Config));
        assert!(!infra_allowed.contains(&Layer::Domain));
    }

    #[test]
    fn test_path_to_module_name() {
        assert_eq!(path_to_module_name(Path::new("cli/args.rs")), "cli::args");
        assert_eq!(
            path_to_module_name(Path::new("domain/state.rs")),
            "domain::state"
        );
        assert_eq!(path_to_module_name(Path::new("state.rs")), "state");
    }
}
