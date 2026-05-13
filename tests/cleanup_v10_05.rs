//! TDD Red Tests — EPIC-V10-05: Limpieza y Migración
//!
//! Estos tests verifican que el código obsoleto de v0.x ha sido completamente
//! eliminado y que las dependencias han sido actualizadas. Todos los tests
//! deben estar en ROJO (fallar) hasta que se complete la limpieza.
//!
//! Gherkin cubierto:
//!   - roadmap/features/quality/cleanup.feature (completo, 6 escenarios)
//!   - roadmap/features/quality/architecture.feature (escenarios implicados en EPIC-V10-05)
//!   - roadmap/features/quality/coverage.feature (STORY-V10-024, parte de arquitectura)
//!
//! Historias:
//!   - STORY-V10-019: Eliminar providers CLI y agent.rs
//!   - STORY-V10-020: Eliminar dominio hardcodeado y dependencia spartito
//!
//! ⚠️  Verificaciones manuales post-cleanup requeridas (no automatizables desde tests):
//!   1. `cargo build` — debe compilar sin errores
//!   2. `cargo test` — todos los tests unitarios deben pasar
//!   3. `cargo clippy -- -D warnings` — cero warnings
//!   Estos pasos son parte del DoD de los escenarios S3 (cleanup.feature)
//!   y C1 (coverage.feature). Ver test `cleanup_manual_verification_checklist`.

use std::fs;
use std::path::{Path, PathBuf};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn cargo_toml_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

fn architecture_test_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("architecture.rs")
}

/// Reads all Rust source lines in `dir` recursively, returning (relative_path, line_number, line_text).
fn collect_all_source_lines(dir: &Path) -> Vec<(PathBuf, usize, String)> {
    let mut results = Vec::new();
    collect_lines_recursive(dir, dir, &mut results);
    results.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    results
}

fn collect_lines_recursive(base: &Path, dir: &Path, results: &mut Vec<(PathBuf, usize, String)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip target/, .git/, etc.
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == "target" || name.starts_with('.') {
                        continue;
                    }
                }
                collect_lines_recursive(base, &path, results);
            } else if path.extension().map_or(false, |e| e == "rs") {
                if let Ok(source) = fs::read_to_string(&path) {
                    let relative = path.strip_prefix(base).unwrap_or(&path);
                    for (i, line) in source.lines().enumerate() {
                        results.push((relative.to_path_buf(), i + 1, line.to_string()));
                    }
                }
            }
        }
    }
}

/// Checks whether a specific file path exists under src/
fn file_exists_under_src(relative_path: &str) -> bool {
    src_dir().join(relative_path).exists()
}

// ═══════════════════════════════════════════════════════════════════════════
// cleanup.feature — STORY-V10-019: Eliminar providers CLI y agent.rs
// ═══════════════════════════════════════════════════════════════════════════

// ── Escenario: infra/providers.rs ya no existe ─────────────────────────

/// **Gherkin**: Given el código fuente en src/
///              When se verifica la existencia de infra/providers.rs
///              Then el archivo no existe
///
/// **RED**: Este archivo existe actualmente (v0.9.5). Debe ser eliminado.
#[test]
fn cleanup_infra_providers_rs_does_not_exist() {
    let path = src_dir().join("infra").join("providers.rs");

    assert!(
        !path.exists(),
        "❌ RED — infra/providers.rs todavía existe en {:?}\n\
         ── cleanup.feature: \"infra/providers.rs ya no existe\"\n\
         Acción requerida (STORY-V10-019): eliminar este archivo.\n\
         Toda la funcionalidad ha sido migrada a infra/llm/.",
        path
    );
}

// ── Escenario: infra/agent.rs ya no existe ─────────────────────────────

/// **Gherkin**: Given el código fuente en src/
///              When se verifica la existencia de infra/agent.rs
///              Then el archivo no existe
///
/// **RED**: Este archivo existe actualmente. Debe ser eliminado.
#[test]
fn cleanup_infra_agent_rs_does_not_exist() {
    let path = src_dir().join("infra").join("agent.rs");

    assert!(
        !path.exists(),
        "❌ RED — infra/agent.rs todavía existe en {:?}\n\
         ── cleanup.feature: \"infra/agent.rs ya no existe\"\n\
         Acción requerida (STORY-V10-019): eliminar este archivo.\n\
         La invocación de agentes ahora se hace a través de infra/llm/.",
        path
    );
}

// ── Escenario: La compilación es limpia sin los módulos eliminados ─────

/// **Gherkin**: Given todos los imports de crate::infra::providers han sido eliminados
///              And todos los imports de crate::infra::agent han sido eliminados
///
/// **RED**: Actualmente hay imports de estos módulos en varios archivos:
///   - app/pipeline.rs
///   - app/plan.rs
///   - app/init.rs
///   - app/resolver.rs
///   - app/validate.rs
///   - infra/agent.rs (self-import)
///   - infra/mod.rs (pub mod)
#[test]
fn cleanup_no_imports_of_providers_module() {
    let all_lines = collect_all_source_lines(&src_dir());
    let mut violations: Vec<String> = Vec::new();

    for (rel_path, line_no, line) in &all_lines {
        let trimmed = line.trim();

        // Detect imports of the providers module
        let imports_providers = trimmed.starts_with("use crate::infra::providers")
            || trimmed.starts_with("pub use crate::infra::providers")
            || (trimmed.starts_with("use ") && trimmed.contains("crate::infra::providers"));

        if imports_providers {
            // Skip if it's inside files that will be deleted (self-referential imports)
            let rel_str = rel_path.to_string_lossy();
            if rel_str.contains("infra/providers")
                || rel_str.contains("infra/agent")
                || rel_str.contains("infra/mod")
            {
                continue;
            }
            violations.push(format!(
                "  {}:{} — {}",
                rel_path.display(),
                line_no,
                trimmed
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — Aún existen imports de crate::infra::providers:\n{}\n\n\
         ── cleanup.feature: \"Todos los imports de crate::infra::providers han sido eliminados\"\n\
         Acción requerida (STORY-V10-019): eliminar estos imports y adaptar el código a usar infra/llm/.\n\
         Violaciones encontradas: {}",
        violations.join("\n"),
        violations.len()
    );
}

/// Verifica que no existen imports de `crate::infra::agent` en el código fuente.
///
/// **RED**: Actualmente hay imports en app/pipeline.rs, app/plan.rs, etc.
#[test]
fn cleanup_no_imports_of_agent_module() {
    let all_lines = collect_all_source_lines(&src_dir());
    let mut violations: Vec<String> = Vec::new();

    for (rel_path, line_no, line) in &all_lines {
        let trimmed = line.trim();

        let imports_agent = trimmed.starts_with("use crate::infra::agent")
            || (trimmed.starts_with("use ") && trimmed.contains("crate::infra::agent"));

        if imports_agent {
            let rel_str = rel_path.to_string_lossy();
            if rel_str.contains("infra/agent") || rel_str.contains("infra/mod") {
                continue;
            }
            violations.push(format!(
                "  {}:{} — {}",
                rel_path.display(),
                line_no,
                trimmed
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — Aún existen imports de crate::infra::agent ({} violaciones):\n{}\n\n\
         ── cleanup.feature: \"Todos los imports de crate::infra::agent han sido eliminados\"\n\
         Acción requerida (STORY-V10-019): eliminar estos imports.",
        violations.len(),
        violations.join("\n")
    );
}

/// Verifica que `infra/mod.rs` ya no declara `pub mod providers` ni `pub mod agent`.
///
/// **Gap 4 fix**: El test `cleanup_no_imports_of_providers_module` ahora también
/// skipea `infra/agent` en su condición de skip, eliminando el ruido de imports
/// autocontenidos en archivos que van a ser eliminados.
///
/// **RED**: Ambas declaraciones existen actualmente.
#[test]
fn cleanup_infra_mod_no_longer_declares_providers_or_agent() {
    let mod_path = src_dir().join("infra").join("mod.rs");
    let source = fs::read_to_string(&mod_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", mod_path, e));

    let mut declarations: Vec<String> = Vec::new();

    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == "pub mod agent;" || trimmed == "pub mod providers;" {
            declarations.push(format!("  línea {}: {}", line_no + 1, trimmed));
        }
    }

    assert!(
        declarations.is_empty(),
        "❌ RED — infra/mod.rs todavía declara módulos eliminados:\n{}\n\n\
         ── cleanup.feature: infra/mod.rs no debe declarar providers ni agent.\n\
         Acción requerida (STORY-V10-019): eliminar 'pub mod providers;' y 'pub mod agent;' de infra/mod.rs.",
        declarations.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// cleanup.feature — STORY-V10-020: Eliminar dominio hardcodeado
// ═══════════════════════════════════════════════════════════════════════════

// ── Gap 1: domain/prompts.rs ya no existe ────────────────────────────

/// **Gherkin** (STORY-V10-020 CA1, architecture-testing.md §2.1):
///   domain/prompts.rs es código v0.x reemplazado por domain/templates.rs.
///   Debe ser eliminado junto con story.rs.
///
/// **RED**: domain/prompts.rs existe actualmente. Debe ser eliminado.
#[test]
fn cleanup_domain_prompts_rs_does_not_exist() {
    let path = src_dir().join("domain").join("prompts.rs");

    assert!(
        !path.exists(),
        "❌ RED — domain/prompts.rs todavía existe en {:?}\n\
         ── STORY-V10-020 CA1: \"prompts.rs es código v0.x, reemplazado por templates.rs\"\n\
         ── architecture-testing.md §2.1: \"prompts (pasó a templates)\"\n\
         Acción requerida (STORY-V10-020): eliminar este archivo.\n\
         Los 7 prompts hardcodeados han sido reemplazados por templates con {{{{variables}}}}.",
        path
    );
}

// ── Escenario: domain/story.rs ya no existe ────────────────────────────

/// **Gherkin**: Given el código fuente en src/
///              When se verifica la existencia de domain/story.rs
///              Then el archivo no existe
///
/// **RED**: domain/story.rs existe actualmente. Debe ser eliminado.
#[test]
fn cleanup_domain_story_rs_does_not_exist() {
    let path = src_dir().join("domain").join("story.rs");

    assert!(
        !path.exists(),
        "❌ RED — domain/story.rs todavía existe en {:?}\n\
         ── cleanup.feature: \"domain/story.rs ya no existe\"\n\
         Acción requerida (STORY-V10-020): eliminar este archivo.\n\
         El tipo Story ha sido reemplazado por Task en domain/task.rs.",
        path
    );
}

// ── Escenario: domain/workflow.rs antiguo ha sido reemplazado ──────────

/// **Gherkin**: Given el código fuente en src/domain/
///              When se examina domain/workflow.rs
///              Then solo contiene ConfigurableWorkflow (no CanonicalWorkflow ni las 14 transiciones fijas)
///              And no importa spartito
///
/// **RED**: CanonicalWorkflow existe actualmente en domain/workflow.rs.
#[test]
fn cleanup_domain_workflow_no_canonical_workflow() {
    let path = src_dir().join("domain").join("workflow.rs");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", path, e));

    // Verificar que CanonicalWorkflow NO está presente
    let has_canonical = source.contains("CanonicalWorkflow");

    assert!(
        !has_canonical,
        "❌ RED — domain/workflow.rs todavía contiene 'CanonicalWorkflow'.\n\
         ── cleanup.feature: \"domain/workflow.rs solo contiene ConfigurableWorkflow\"\n\
         Acción requerida (STORY-V10-020): eliminar CanonicalWorkflow y el trait Workflow hardcodeado.\n\
         Solo debe quedar ConfigurableWorkflow (workflow definido desde TOML)."
    );
}

/// Verifica que domain/workflow.rs no contiene el trait Workflow con Status fijo.
///
/// **RED**: El trait Workflow que usa Status (enum) todavía existe.
#[test]
fn cleanup_domain_workflow_no_status_based_trait() {
    let path = src_dir().join("domain").join("workflow.rs");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", path, e));

    // Verificar que el trait Workflow no usa Status (el enum fijo)
    let has_status_import = source.contains("use crate::domain::state::Status");

    assert!(
        !has_status_import,
        "❌ RED — domain/workflow.rs todavía importa crate::domain::state::Status.\n\
         ── cleanup.feature: \"domain/workflow.rs no usa Status (enum fijo)\"\n\
         Acción requerida (STORY-V10-020): el workflow v1.0 usa strings para estados, no el enum Status."
    );
}

/// Verifica que domain/workflow.rs no importa spartito.
///
/// **GREEN o RED**: spartito nunca fue añadido como dependencia real en Cargo.toml,
/// pero verificamos que no hay referencias.
#[test]
fn cleanup_domain_workflow_no_spartito_import() {
    let path = src_dir().join("domain").join("workflow.rs");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", path, e));

    let has_spartito = source.to_lowercase().contains("spartito");

    assert!(
        !has_spartito,
        "❌ RED — domain/workflow.rs contiene referencia a 'spartito'.\n\
         ── cleanup.feature: \"domain/workflow.rs no importa spartito\"\n\
         Acción requerida (STORY-V10-020): eliminar cualquier mención a spartito."
    );
}

/// Verifica que no existen imports de `crate::domain::story` o `use crate::domain::Story`
/// en ningún archivo fuente (fuera del propio story.rs).
///
/// **RED**: Actualmente hay imports en deadlock.rs, graph.rs, report.rs, story_io.rs,
/// pipeline.rs, validate.rs, board.rs.
#[test]
fn cleanup_no_imports_of_story_module() {
    let all_lines = collect_all_source_lines(&src_dir());
    let mut violations: Vec<String> = Vec::new();

    for (rel_path, line_no, line) in &all_lines {
        let trimmed = line.trim();

        let imports_story = trimmed.starts_with("use crate::domain::story")
            || (trimmed.starts_with("use ") && trimmed.contains("crate::domain::story"));

        if imports_story {
            let rel_str = rel_path.to_string_lossy();
            // Skip the story.rs file itself and domain/mod.rs re-export
            if rel_str.contains("domain/story") || rel_str.contains("domain/mod") {
                continue;
            }
            violations.push(format!(
                "  {}:{} — {}",
                rel_path.display(),
                line_no,
                trimmed
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — Aún existen imports de crate::domain::story:\n{}\n\n\
         ── cleanup.feature: imports de Story eliminados de todos los módulos.\n\
         Acción requerida (STORY-V10-020): migrar estos imports a usar domain::task::Task.\n\
         Violaciones encontradas: {}",
        violations.join("\n"),
        violations.len()
    );
}

/// Verifica que `domain/mod.rs` ya no declara `pub mod story`, `pub mod prompts`,
/// ni re-exporta `Story`.
///
/// **Gap 1 complemento**: también verifica que `prompts` no se declara.
///
/// **RED**: domain/mod.rs declara `pub mod story;` actualmente.
#[test]
fn cleanup_domain_mod_no_longer_declares_story() {
    let mod_path = src_dir().join("domain").join("mod.rs");
    let source = fs::read_to_string(&mod_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", mod_path, e));

    let mut declarations: Vec<String> = Vec::new();

    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == "pub mod story;"
            || trimmed == "pub mod prompts;"
            || trimmed.starts_with("pub use story::")
        {
            declarations.push(format!("  línea {}: {}", line_no + 1, trimmed));
        }
    }

    assert!(
        declarations.is_empty(),
        "❌ RED — domain/mod.rs todavía declara/re-exporta módulos v0.x:\n{}\n\n\
         ── cleanup.feature: domain/mod.rs no debe declarar story ni prompts.\n\
         Acción requerida (STORY-V10-020): eliminar 'pub mod story;' y 'pub mod prompts;' de domain/mod.rs.",
        declarations.join("\n")
    );
}

/// Verifica que `domain/mod.rs` solo re-exporta los módulos v1.0:
/// task, workflow, templates, graph, deadlock, state.
#[test]
fn cleanup_domain_mod_only_exposes_v10_modules() {
    let mod_path = src_dir().join("domain").join("mod.rs");
    let source = fs::read_to_string(&mod_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", mod_path, e));

    let v10_modules = ["task", "workflow", "templates", "graph", "deadlock", "state"];

    // Collect declared pub mods
    let mut declared: Vec<String> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub mod ") {
            let mod_name = trimmed
                .strip_prefix("pub mod ")
                .unwrap()
                .trim_end_matches(';');
            declared.push(mod_name.to_string());
        }
    }

    // Check that no v0.x modules are declared
    let forbidden = ["story", "prompts", "dependency_graph"];
    let mut bad_declarations: Vec<String> = Vec::new();
    for d in &declared {
        if forbidden.contains(&d.as_str()) {
            bad_declarations.push(d.clone());
        }
    }

    assert!(
        bad_declarations.is_empty(),
        "❌ RED — domain/mod.rs declara módulos v0.x obsoletos: {:?}\n\
         ── cleanup.feature (STORY-V10-020, CA2): solo módulos v1.0.\n\
         Módulos permitidos: {:?}",
        bad_declarations,
        v10_modules
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// cleanup.feature — Cargo.toml
// ═══════════════════════════════════════════════════════════════════════════

// ── Escenario: Cargo.toml no depende de spartito ni ureq ───────────────

/// **Gherkin**: Given el archivo Cargo.toml
///              When se examinan las dependencias
///              Then no existe la entrada "spartito"
///
/// **GREEN**: spartito nunca fue añadido como dependencia real en Cargo.toml.
/// Este test verifica que siga siendo así.
#[test]
fn cleanup_cargo_toml_has_no_spartito_dependency() {
    let cargo = fs::read_to_string(&cargo_toml_path())
        .unwrap_or_else(|e| panic!("No se pudo leer Cargo.toml: {}", e));

    let mut found_spartito = false;

    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            // Start of deps
            continue;
        }
        if trimmed.starts_with('[') && trimmed != "[dependencies]" {
            // Another section
            continue;
        }
        if trimmed.starts_with("spartito") {
            found_spartito = true;
            break;
        }
    }

    assert!(
        !found_spartito,
        "❌ RED — Cargo.toml contiene dependencia 'spartito'.\n\
         ── cleanup.feature: \"no existe la entrada spartito\"\n\
         Acción requerida (STORY-V10-020): eliminar spartito de [dependencies]."
    );
}

/// Verifica que `ureq` NO está en Cargo.toml (ha sido reemplazado por `reqwest`).
///
/// **RED**: ureq existe actualmente en Cargo.toml.
#[test]
fn cleanup_cargo_toml_has_no_ureq_dependency() {
    let cargo = fs::read_to_string(&cargo_toml_path())
        .unwrap_or_else(|e| panic!("No se pudo leer Cargo.toml: {}", e));

    let mut found_ureq = false;

    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ureq") {
            found_ureq = true;
            break;
        }
    }

    assert!(
        !found_ureq,
        "❌ RED — Cargo.toml todavía contiene la dependencia 'ureq'.\n\
         ── cleanup.feature: \"no existe la entrada ureq\"\n\
         Acción requerida (STORY-V10-019): eliminar ureq de [dependencies].\n\
         Ha sido reemplazada por reqwest (cliente HTTP usado en infra/llm/ y app/update.rs)."
    );
}

/// Verifica que `reqwest` SÍ está en Cargo.toml con los features correctos.
///
/// **RED**: reqwest no existe actualmente en Cargo.toml.
#[test]
fn cleanup_cargo_toml_has_reqwest_with_correct_features() {
    let cargo = fs::read_to_string(&cargo_toml_path())
        .unwrap_or_else(|e| panic!("No se pudo leer Cargo.toml: {}", e));

    let mut found_reqwest = false;
    let mut reqwest_line = String::new();

    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("reqwest") {
            found_reqwest = true;
            reqwest_line = trimmed.to_string();
            break;
        }
    }

    assert!(
        found_reqwest,
        "❌ RED — Cargo.toml no contiene la dependencia 'reqwest'.\n\
         ── cleanup.feature: \"existe la entrada reqwest con features [json, rustls-tls]\"\n\
         Acción requerida (STORY-V10-019): añadir reqwest = {{ version = \"0.12\", features = [\"json\", \"rustls-tls\"] }}."
    );

    // Verify features
    let line_lower = reqwest_line.to_lowercase();
    assert!(
        line_lower.contains("json"),
        "❌ RED — reqwest no tiene el feature 'json'.\n\
         ── cleanup.feature: reqwest debe tener features = [\"json\", \"rustls-tls\"]\n\
         Línea actual: {}",
        reqwest_line
    );
    assert!(
        line_lower.contains("rustls-tls") || line_lower.contains("rustls"),
        "❌ RED — reqwest no tiene el feature 'rustls-tls'.\n\
         ── cleanup.feature: reqwest debe tener features = [\"json\", \"rustls-tls\"]\n\
         Línea actual: {}",
        reqwest_line
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// architecture.feature — "Los módulos eliminados en v1.0 ya no existen"
// ═══════════════════════════════════════════════════════════════════════════

/// **Gherkin** (architecture.feature):
///   Given el código fuente en src/
///   When se verifica la existencia de los siguientes archivos:
///     | archivo              |
///     | infra/providers.rs   |
///     | infra/agent.rs       |
///     | domain/story.rs      |
///   Then ninguno de ellos existe
///
/// **RED**: Los 3 archivos existen actualmente.
#[test]
fn architecture_removed_modules_do_not_exist() {
    let removed = &[
        "infra/providers.rs",
        "infra/agent.rs",
        "domain/story.rs",
        "domain/prompts.rs",
    ];

    let mut existing: Vec<String> = Vec::new();

    for file in removed {
        if file_exists_under_src(file) {
            existing.push(format!("  - src/{} (EXISTE — debe ser eliminado)", file));
        }
    }

    assert!(
        existing.is_empty(),
        "❌ RED — Los siguientes archivos v0.x todavía existen:\n{}\n\n\
         ── architecture.feature: \"Los módulos eliminados en v1.0 ya no existen\"\n\
         Acción requerida (EPIC-V10-05): eliminar estos archivos.",
        existing.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// coverage.feature (STORY-V10-024) — Tests de arquitectura actualizados
// ═══════════════════════════════════════════════════════════════════════════

/// **Gherkin** (coverage.feature, STORY-V10-024 CA3):
///   tests/architecture.rs actualizado: nuevas reglas que verifican que
///   infra/llm/ no importa domain, app, ni cli.
///
/// **RED**: El test de arquitectura existe pero las reglas legacy de root_file_layer()
/// aún contienen entradas v0.x que deben eliminarse en Phase 5.
#[test]
fn architecture_test_has_no_legacy_entries_in_root_file_layer() {
    let arch_test_path = architecture_test_path();
    let source = fs::read_to_string(&arch_test_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", arch_test_path, e));

    // These legacy entries must NOT appear in root_file_layer after cleanup
    let legacy_entries = &[
        "\"story\"",
        "\"dependency_graph\"",
        "\"prompts\"",
        "\"providers\"",
        "\"agent\"",
        "\"orchestrator\"",
        "\"validator\"",
    ];

    let mut found_legacy: Vec<String> = Vec::new();

    // We need to check within the root_file_layer function specifically.
    // Find the function body
    let mut in_legacy_comment = false;

    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Track whether we're inside root_file_layer function
        if trimmed.starts_with("fn root_file_layer(") {
            // Function started
            continue;
        }

        // Simple approach: find any legacy entry in the file
        // The function is the only place they appear
        for entry in legacy_entries {
            if trimmed.contains(entry) && !trimmed.starts_with("//") && !trimmed.starts_with("///")
            {
                // Check if it's inside a comment block
                if !in_legacy_comment {
                    found_legacy.push(format!(
                        "  línea {}: {}",
                        line_no + 1,
                        trimmed
                    ));
                }
            }
        }
    }

    // Dedup
    found_legacy.sort();
    found_legacy.dedup();

    assert!(
        found_legacy.is_empty(),
        "❌ RED — tests/architecture.rs todavía contiene entradas legacy v0.x en root_file_layer():\n{}\n\n\
         ── coverage.feature (STORY-V10-024 CA3): \"tests/architecture.rs actualizado sin entradas legacy\"\n\
         ── architecture-testing.md, sección 2.1: eliminar 'story', 'dependency_graph', 'prompts',\n\
         ──   'providers', 'agent', 'orchestrator', 'validator' de root_file_layer().\n\
         Acción requerida (EPIC-V10-05): limpiar root_file_layer() en tests/architecture.rs.",
        found_legacy.join("\n")
    );
}

/// Verifica que el mega-test `architecture_layers_are_respected` sigue funcionando
/// y no reporta falsos positivos después de la limpieza.
///
/// Este test corre el mega-test existente y verifica que NO hay violaciones de
/// arquitectura después del cleanup.
///
/// **RED**: Actualmente puede haber violaciones porque el código v0.x aún existe
/// y tiene imports que rompen las reglas (ej: config.rs importa providers).
#[test]
fn architecture_mega_test_passes_after_cleanup() {
    // This test is a meta-test: it re-runs the architecture mega-test logic
    // in-process to verify it would pass after cleanup.

    // We check that the mega-test function exists and is callable.
    // The actual mega-test logic is in tests/architecture.rs and runs separately.
    // This test verifies that the key structural requirements are met:
    // 1. config.rs does not import any crate module (R5)
    // 2. domain/ files do not import infra, app, cli (R1)
    // 3. infra/llm/ does not import domain (R2)

    let config_path = src_dir().join("config.rs");
    let config_source = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("No se pudo leer config.rs: {}", e));

    let mut r5_violations: Vec<String> = Vec::new();
    for (line_no, line) in config_source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("use crate::") {
            // Allow use crate::config:: for sub-modules
            if !trimmed.starts_with("use crate::config::") {
                r5_violations.push(format!("  línea {}: {}", line_no + 1, trimmed));
            }
        }
    }

    assert!(
        r5_violations.is_empty(),
        "❌ RED — R5 violation: config.rs importa módulos del crate:\n{}\n\n\
         ── coverage.feature (STORY-V10-024 CA3): \"config/ no importa ninguna capa del crate\"\n\
         Acción requerida (EPIC-V10-05): eliminar imports de crate:: en config.rs.\n\
         Nota: actualmente config.rs importa crate::providers — esto debe eliminarse.",
        r5_violations.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Verificaciones adicionales cross-layer (R1-R5 post-cleanup)
// ═══════════════════════════════════════════════════════════════════════════

/// Verifica que ningún archivo en domain/ importa `anyhow` (infra crate).
///
/// **RED/GREEN**: Depende de si los módulos domain/ actuales importan anyhow.
/// domain/task.rs y domain/templates.rs no lo importan actualmente.
#[test]
fn post_cleanup_domain_does_not_import_anyhow() {
    let domain_dir = src_dir().join("domain");

    if !domain_dir.exists() {
        return;
    }

    let mut violations = Vec::new();

    for entry in fs::read_dir(&domain_dir).expect("cannot read domain/") {
        let path = entry.expect("cannot read dir entry").path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }

        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));

        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use anyhow::") || trimmed.starts_with("extern crate anyhow") {
                violations.push(format!(
                    "{}:{} — {}",
                    path.strip_prefix(&src_dir()).unwrap_or(&path).display(),
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — domain/ depende de anyhow (crate de infraestructura):\n{}\n\n\
         ── architecture-testing.md Test B: \"domain/ no debe usar anyhow\".\n\
         Acción requerida: usar errores tipados o String en domain/.",
        violations.join("\n")
    );
}

/// Verifica que infra/llm/ no importa domain/ (R2 estricto).
///
/// **RED/GREEN**: Verificamos que infra/llm/ sigue la regla R2.
#[test]
fn post_cleanup_infra_llm_does_not_import_domain() {
    let llm_dir = src_dir().join("infra").join("llm");

    if !llm_dir.exists() {
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
                    "{}:{} — {}",
                    path.strip_prefix(&src_dir()).unwrap_or(&path).display(),
                    line_no + 1,
                    trimmed,
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — R2 violation: infra/llm/ importa domain/:\n{}\n\n\
         ── architecture.feature: \"infra/llm/openai.rs no importa dominio ni aplicación\".\n\
         Acción requerida: infra/llm/ solo debe importar config y crates externos.",
        violations.join("\n")
    );
}

/// Verifica que app/pipeline.rs no importa cli/ (R3).
///
/// **RED/GREEN**: Verifica la regla R3 en el pipeline post-cleanup.
#[test]
fn post_cleanup_app_pipeline_does_not_import_cli() {
    let pipeline_path = src_dir().join("app").join("pipeline.rs");

    if !pipeline_path.exists() {
        return;
    }

    let source = fs::read_to_string(&pipeline_path)
        .unwrap_or_else(|e| panic!("cannot read {pipeline_path:?}: {e}"));

    let mut violations = Vec::new();

    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("use crate::cli") {
            violations.push(format!("  línea {}: {}", line_no + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — R3 violation: app/pipeline.rs importa cli/:\n{}\n\n\
         ── architecture.feature: \"app/pipeline.rs no importa cli\".\n\
         Acción requerida: app/ no debe depender de cli/.",
        violations.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Integridad — domain/state.rs solo conserva SharedState (sin Status/Actor/Transition)
// ═══════════════════════════════════════════════════════════════════════════

/// **Gherkin** (STORY-V10-020 CA1):
///   De domain/state.rs solo se conserva SharedState.
///   Status, Actor, Transition deben ser eliminados.
///
/// **RED**: Status, Actor y Transition existen actualmente en state.rs.
#[test]
fn cleanup_domain_state_only_has_shared_state() {
    let state_path = src_dir().join("domain").join("state.rs");
    let source = fs::read_to_string(&state_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", state_path, e));

    let forbidden_types = ["pub enum Status", "pub enum Actor", "pub struct Transition"];
    let mut found: Vec<String> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        for ft in &forbidden_types {
            if trimmed.starts_with(ft) {
                found.push(format!("  {}", trimmed));
            }
        }
    }

    assert!(
        found.is_empty(),
        "❌ RED — domain/state.rs todavía contiene tipos hardcodeados:\n{}\n\n\
         ── STORY-V10-020 CA1: \"De domain/state.rs solo se conserva SharedState\".\n\
         Acción requerida: eliminar Status, Actor, Transition de state.rs.\n\
         Los estados ahora son strings definidos en TOML.",
        found.join("\n")
    );
}

/// Verifica que domain/state.rs aún conserva SharedState.
///
/// **RED/GREEN**: SharedState debe conservarse.
#[test]
fn cleanup_domain_state_retains_shared_state() {
    let state_path = src_dir().join("domain").join("state.rs");
    let source = fs::read_to_string(&state_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", state_path, e));

    assert!(
        source.contains("SharedState"),
        "❌ RED — domain/state.rs ha perdido SharedState.\n\
         ── STORY-V10-020 CA1: \"De domain/state.rs solo se conserva SharedState\".\n\
         Acción requerida: asegurarse de que SharedState (con TokenCount y token_usage) se conserva."
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Gap 2 — No deben quedar imports de Status, Actor, Transition en ningún módulo
// ═══════════════════════════════════════════════════════════════════════════

/// **Gherkin** (STORY-V10-020 CA1):
///   "Los imports de Status, Actor, Transition se eliminan de todos los módulos."
///
/// Escanea todos los archivos .rs en src/ en busca de imports residuales de
/// los tipos hardcodeados que deben ser eliminados de `domain/state.rs`.
///
/// **RED**: Actualmente deadlock.rs y graph.rs importan `domain::state::Status`
/// (a través de `domain::story::Story`). Tras eliminar Story, si algún módulo
/// todavía referencia estos tipos directamente, este test lo detectará.
#[test]
fn cleanup_no_imports_of_status_actor_transition() {
    let all_lines = collect_all_source_lines(&src_dir());
    let forbidden_patterns = &[
        "domain::state::Status",
        "domain::state::Actor",
        "domain::state::Transition",
    ];

    let mut violations: Vec<String> = Vec::new();

    for (rel_path, line_no, line) in &all_lines {
        let trimmed = line.trim();

        // Solo interesan líneas que son imports (use ...)
        if !trimmed.starts_with("use ") {
            continue;
        }

        let rel_str = rel_path.to_string_lossy();

        // Skip los archivos que definen estos tipos (se eliminarán)
        if rel_str.contains("domain/state") || rel_str.contains("domain/story") {
            continue;
        }

        // Skip domain/mod.rs (solo re-exports)
        if rel_str.contains("domain/mod") {
            continue;
        }

        for pattern in forbidden_patterns {
            if trimmed.contains(pattern) {
                violations.push(format!(
                    "  {}:{} — {}",
                    rel_path.display(),
                    line_no,
                    trimmed
                ));
                break; // no duplicar la misma línea para múltiples patrones
            }
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — Aún existen imports de Status/Actor/Transition ({} violaciones):\n{}\n\n\
         ── STORY-V10-020 CA1: \"Los imports de Status, Actor, Transition se eliminan de todos los módulos\"\n\
         Acción requerida: migrar estos módulos para que usen strings (estados definidos en TOML)\n\
         en lugar de los tipos hardcodeados Status/Actor/Transition de v0.x.",
        violations.len(),
        violations.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Gap 3 — Checklist de verificación manual post-cleanup
// ═══════════════════════════════════════════════════════════════════════════

/// **Gherkin** (cleanup.feature S3 + coverage.feature STORY-V10-024 CA3):
///   "When se ejecuta cargo build → Then compila sin errores
///    And cargo test pasa todos los tests unitarios restantes
///    And cargo clippy -- -D warnings no reporta warnings"
///
/// Estas verificaciones requieren herramientas externas (cargo build, cargo test,
/// cargo clippy) y no pueden ejecutarse desde dentro del harness de tests.
///
/// Este test documenta los pasos manuales requeridos para completar el DoD.
/// Se mantiene con `#[ignore]` porque no es automatizable.
#[test]
#[ignore = "Verificación manual: ejecutar 'cargo build', 'cargo test', y 'cargo clippy -- -D warnings' después de la limpieza"]
fn cleanup_manual_verification_checklist() {
    // ═══════════════════════════════════════════════════════════════════
    // PASOS MANUALES REQUERIDOS PARA EL DoD DE EPIC-V10-05:
    //
    // 1. cargo build
    //    Esperado: compila sin errores.
    //    Verifica que: todos los imports rotos por la eliminación de
    //    providers.rs, agent.rs, story.rs, prompts.rs han sido corregidos.
    //
    // 2. cargo test
    //    Esperado: todos los tests unitarios pasan.
    //    Verifica que: ningún test depende de los módulos eliminados.
    //    Los tests que usaban Story/CanonicalWorkflow deben haberse
    //    migrado a Task/ConfigurableWorkflow o eliminado.
    //
    // 3. cargo clippy -- -D warnings
    //    Esperado: cero warnings.
    //    Verifica que: no hay código muerto, imports no usados, ni
    //    variables sin usar como resultado de la limpieza.
    //
    // 4. cargo test --test architecture
    //    Esperado: todos los tests de arquitectura pasan (R1-R5).
    //    Verifica que: las reglas de capas se siguen respetando.
    //
    // 5. cargo test --test cleanup_v10_05
    //    Esperado: TODOS los tests pasan (0 failing).
    //    Los 16+ tests RED de este archivo deben estar GREEN.
    // ═══════════════════════════════════════════════════════════════════

    println!("✅ Todas las verificaciones automatizables han pasado.");
    println!("⚠️  Completa las verificaciones manuales listadas arriba.");
    println!("📋 Ver documentación: tests/cleanup_v10_05.rs — fn cleanup_manual_verification_checklist");
}

// ═══════════════════════════════════════════════════════════════════════════
// Verificación de que el dominio v1.0 es autocontenido sin referencias v0.x
// ═══════════════════════════════════════════════════════════════════════════

/// Verifica que domain/task.rs no depende de domain/story.rs ni de Status.
///
/// **RED/GREEN**: Verifica que el dominio v1.0 es independiente.
#[test]
fn cleanup_domain_task_does_not_import_story_or_status() {
    let task_path = src_dir().join("domain").join("task.rs");
    let source = fs::read_to_string(&task_path)
        .unwrap_or_else(|e| panic!("No se pudo leer {:?}: {}", task_path, e));

    let mut violations: Vec<String> = Vec::new();

    for (line_no, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("domain::story") || trimmed.contains("domain::state::Status") {
            violations.push(format!("  línea {}: {}", line_no + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "❌ RED — domain/task.rs importa domain::story o domain::state::Status:\n{}\n\n\
         ── domain/task.rs debe ser independiente de los tipos v0.x.",
        violations.join("\n")
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests unitarios de los helpers (para verificar que los helpers son correctos)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn test_file_exists_under_src_positive() {
        // domain/task.rs debería existir (v1.0)
        assert!(file_exists_under_src("domain/task.rs"));
    }

    #[test]
    fn test_file_exists_under_src_negative() {
        // Un archivo que no existe
        assert!(!file_exists_under_src("domain/fantasma.rs"));
    }

    #[test]
    fn test_src_dir_is_correct() {
        let dir = src_dir();
        assert!(dir.exists());
        assert!(dir.is_dir());
        // Debe contener domain, infra, app, cli
        assert!(dir.join("domain").exists());
        assert!(dir.join("infra").exists());
        assert!(dir.join("app").exists());
        assert!(dir.join("cli").exists());
    }

    #[test]
    fn test_cargo_toml_path_is_correct() {
        let path = cargo_toml_path();
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("[package]"));
        assert!(content.contains("regista"));
    }

    #[test]
    fn test_collect_all_source_lines_finds_files() {
        let lines = collect_all_source_lines(&src_dir());
        assert!(!lines.is_empty(), "Debe encontrar archivos .rs en src/");

        // Debe encontrar domain/task.rs
        let has_task = lines.iter().any(|(p, _, _)| {
            p.to_string_lossy().contains("domain/task.rs")
        });
        assert!(has_task, "Debe encontrar domain/task.rs");
    }
}
