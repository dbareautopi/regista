//! Loop principal del orquestador.
//!
//! Carga historias, construye el grafo de dependencias, evalúa deadlocks,
//! y dispara agentes según la máquina de estados. Es el corazón del pipeline.

use crate::agent::{self, AgentOptions};
use crate::checkpoint::OrchestratorState;
use crate::config::Config;
use crate::deadlock::{self, DeadlockResolution};
use crate::dependency_graph::DependencyGraph;
use crate::prompts::PromptContext;
use crate::state::Status;
use crate::story::Story;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

/// Opciones de filtrado y modo de ejecución para el orquestador.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Ejecutar una sola iteración y salir.
    pub once: bool,
    /// Solo procesar esta historia (ID exacto, ej: "STORY-001").
    pub story_filter: Option<String>,
    /// Solo procesar historias de esta épica (ej: "EPIC-001").
    pub epic_filter: Option<String>,
    /// Solo procesar historias en este rango de épicas (inclusivo).
    /// Tupla (start, end), ej: ("EPIC-001", "EPIC-003").
    pub epics_range: Option<(String, String)>,
    /// Modo simulación: no invoca agentes ni modifica archivos.
    pub dry_run: bool,
    /// Suprimir logs de progreso (útil con --json).
    pub quiet: bool,
}

/// Filtra historias según las opciones de ejecución.
fn filter_stories(stories: Vec<Story>, options: &RunOptions) -> Vec<Story> {
    let mut stories = stories;

    if let Some(ref story_id) = options.story_filter {
        stories.retain(|s| s.id == *story_id);
    }

    if let Some(ref epic_id) = options.epic_filter {
        stories.retain(|s| s.epic.as_ref().is_some_and(|e| e == epic_id));
    }

    if let Some((ref start, ref end)) = options.epics_range {
        let start_num = extract_numeric(start);
        let end_num = extract_numeric(end);
        stories.retain(|s| {
            s.epic.as_ref().is_some_and(|e| {
                let num = extract_numeric(e);
                num >= start_num && num <= end_num
            })
        });
    }

    stories
}

/// Ejecuta el pipeline completo sobre un proyecto.
///
/// En modo normal invoca agentes `pi` y modifica archivos.
/// En modo dry-run simula todo el pipeline en memoria.
pub fn run(
    project_root: &Path,
    cfg: &Config,
    options: &RunOptions,
    resume_state: Option<OrchestratorState>,
) -> anyhow::Result<RunReport> {
    if options.dry_run {
        return run_dry(project_root, cfg, options);
    }
    run_real(project_root, cfg, options, resume_state)
}

/// Ejecución real del pipeline (invocando agentes).
fn run_real(
    project_root: &Path,
    cfg: &Config,
    options: &RunOptions,
    resume_state: Option<OrchestratorState>,
) -> anyhow::Result<RunReport> {
    let start = Instant::now();
    let max_wall = std::time::Duration::from_secs(cfg.limits.max_wall_time_seconds);

    let (mut reject_cycles, mut story_iterations, mut story_errors, start_iteration) =
        if let Some(state) = resume_state {
            tracing::info!(
                "📂 Reanudando desde checkpoint: iteración {}",
                state.iteration
            );
            (
                state.reject_cycles,
                state.story_iterations,
                state.story_errors,
                state.iteration,
            )
        } else {
            (HashMap::new(), HashMap::new(), HashMap::new(), 0u32)
        };

    let mut iteration: u32 = start_iteration;
    let mut stop_reason: Option<String> = None;

    // Calcular límite efectivo de iteraciones una sola vez al inicio.
    // Si el usuario no lo configuró (0), se escala con el nº de historias.
    let initial_stories = load_all_stories(project_root, cfg)?;
    let effective_max = effective_max_iterations(cfg.limits.max_iterations, initial_stories.len());
    if effective_max != cfg.limits.max_iterations {
        tracing::info!(
            "max_iterations auto: {} ({} historias × 6)",
            effective_max,
            initial_stories.len()
        );
    }

    loop {
        iteration += 1;
        if iteration > effective_max {
            stop_reason = Some(format!("max_iterations ({})", effective_max));
            tracing::warn!("Alcanzado el máximo de {} iteraciones", effective_max);
            break;
        }
        if start.elapsed() >= max_wall {
            stop_reason = Some(format!("max_wall_time ({}s)", max_wall.as_secs()));
            tracing::warn!("Límite de tiempo total alcanzado ({}s)", max_wall.as_secs());
            break;
        }

        if !options.quiet {
            tracing::info!("══════ Iteración {iteration} ══════");
        }

        // 1. Cargar todas las historias
        let stories = load_all_stories(project_root, cfg)?;
        let full_graph = DependencyGraph::from_stories(&stories);

        // 2. Aplicar transiciones automáticas sobre TODAS las historias
        let stories =
            apply_automatic_transitions(stories, &full_graph, &mut reject_cycles, cfg, false)?;

        // 3. Filtrar historias según opciones de ejecución (--story, --epic, --epics)
        let stories = filter_stories(stories, options);
        if stories.is_empty() {
            tracing::info!("Sin historias que procesar con los filtros actuales.");
            break;
        }

        // 4. Reconstruir grafo solo con las historias filtradas
        let graph = DependencyGraph::from_stories(&stories);

        // 5. Detectar deadlock
        let resolution = deadlock::analyze(&stories, &graph);

        if !handle_deadlock(&resolution, project_root, cfg)? {
            break;
        }

        // Procesar según la resolución
        match &resolution {
            DeadlockResolution::InvokePoFor {
                story_id, reason, ..
            } => {
                if !options.quiet {
                    tracing::info!("🔓 Deadlock detectado: {reason}");
                }
                let story = stories.iter().find(|s| s.id == *story_id).unwrap();
                let iter = story_iterations.entry(story.id.clone()).or_insert(0);
                *iter += 1;
                let agent_opts = build_agent_opts(story, cfg);
                if let Err(e) =
                    process_story(story, project_root, cfg, &mut reject_cycles, &agent_opts)
                {
                    story_errors
                        .entry(story.id.clone())
                        .or_insert_with(|| e.to_string());
                }
                save_checkpoint(
                    project_root,
                    iteration,
                    &reject_cycles,
                    &story_iterations,
                    &story_errors,
                );
            }
            DeadlockResolution::NoDeadlock => {
                // 5. Procesar la historia de mayor prioridad en el flujo normal
                if let Some(story) = pick_next_actionable(&stories, &graph) {
                    let id = story.id.clone();
                    let iter = story_iterations.entry(id.clone()).or_insert(0);
                    *iter += 1;
                    let agent_opts = build_agent_opts(story, cfg);
                    if let Err(e) =
                        process_story(story, project_root, cfg, &mut reject_cycles, &agent_opts)
                    {
                        story_errors
                            .entry(id.clone())
                            .or_insert_with(|| e.to_string());
                    }
                    save_checkpoint(
                        project_root,
                        iteration,
                        &reject_cycles,
                        &story_iterations,
                        &story_errors,
                    );
                }
            }
            DeadlockResolution::PipelineComplete => {
                if !options.quiet {
                    tracing::info!("✅ Pipeline completo: todas las historias en estado terminal.");
                }
                OrchestratorState::remove(project_root);
                break;
            }
        }

        if options.once {
            if !options.quiet {
                tracing::info!("🏁 Modo --once: completado tras una iteración.");
            }
            break;
        }
    }

    // Generar reporte final
    let stories = filter_stories(load_all_stories(project_root, cfg)?, options);
    build_report(
        &stories,
        iteration,
        start.elapsed(),
        &story_iterations,
        &reject_cycles,
        &story_errors,
        stop_reason,
    )
}

/// Ejecución simulada del pipeline (dry-run).
fn run_dry(project_root: &Path, cfg: &Config, options: &RunOptions) -> anyhow::Result<RunReport> {
    let start = Instant::now();

    tracing::info!("🧪 DRY-RUN — No se ejecutarán agentes ni se modificarán archivos.");
    tracing::info!("");

    // Cargar historias UNA VEZ para el modo simulación
    let mut stories = filter_stories(load_all_stories(project_root, cfg)?, options);
    if stories.is_empty() {
        tracing::info!("Sin historias que procesar.");
        return build_report(
            &stories,
            0,
            start.elapsed(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            None,
        );
    }

    let reject_cycles: HashMap<String, u32> = HashMap::new();
    let mut story_iterations: HashMap<String, u32> = HashMap::new();
    let story_errors: HashMap<String, String> = HashMap::new();
    let mut iteration: u32 = 0;

    // Calcular límite efectivo de iteraciones
    let effective_max = effective_max_iterations(cfg.limits.max_iterations, stories.len());
    if effective_max != cfg.limits.max_iterations {
        tracing::info!(
            "max_iterations auto: {} ({} historias × 6)",
            effective_max,
            stories.len()
        );
    }

    loop {
        iteration += 1;
        if iteration > effective_max {
            break;
        }

        tracing::info!("═══ Iteración {iteration} ═══");

        // Aplicar transiciones automáticas en memoria
        // Primero recolectamos los estados actuales para evitar borrow conflict
        let status_snapshot: Vec<(String, Status, Vec<String>)> = stories
            .iter()
            .map(|s| (s.id.clone(), s.status, s.blockers.clone()))
            .collect();

        for (id, status, blockers) in &status_snapshot {
            if *status == Status::Blocked {
                let all_done = blockers.iter().all(|b| {
                    stories
                        .iter()
                        .any(|s| s.id == *b && s.status == Status::Done)
                });
                if all_done {
                    tracing::info!("  → {id} (Blocked) desbloqueada automáticamente → Ready");
                    if let Some(story) = stories.iter_mut().find(|s| s.id == *id) {
                        story.advance_status_in_memory(Status::Ready);
                    }
                }
            }
        }

        let graph = DependencyGraph::from_stories(&stories);
        let resolution = deadlock::analyze(&stories, &graph);

        match &resolution {
            DeadlockResolution::PipelineComplete => {
                tracing::info!("✅ Pipeline completo.");
                break;
            }
            DeadlockResolution::InvokePoFor {
                story_id,
                reason,
                unblocks,
                ..
            } => {
                tracing::info!("  → {story_id} (Draft) sería procesada por PO (groom) → Ready");
                tracing::info!("    Razón: {reason}");
                if *unblocks > 0 {
                    tracing::info!("    Desbloquearía: {unblocks} historias");
                }
                if let Some(story) = stories.iter_mut().find(|s| s.id == *story_id) {
                    let iter = story_iterations.entry(story.id.clone()).or_insert(0);
                    *iter += 1;
                    story.advance_status_in_memory(Status::Ready);
                }
            }
            DeadlockResolution::NoDeadlock => {
                if let Some(id) = {
                    let graph = DependencyGraph::from_stories(&stories);
                    pick_next_actionable(&stories, &graph).map(|s| s.id.clone())
                } {
                    if let Some(story) = stories.iter_mut().find(|s| s.id == id) {
                        let next = next_status(story.status);
                        let label = actor_label(story.status);
                        let iter = story_iterations.entry(story.id.clone()).or_insert(0);
                        *iter += 1;
                        tracing::info!(
                            "  → {} ({}) sería procesada por {} → {}",
                            story.id,
                            story.status,
                            label,
                            next
                        );
                        let unblocks = graph.blocks_count(&story.id);
                        if unblocks > 0 {
                            tracing::info!("    Desbloquearía: {unblocks} historias");
                        }
                        story.advance_status_in_memory(next);
                    }
                }
            }
        }

        if options.once {
            tracing::info!("🏁 Modo --once: simulada una iteración.");
            break;
        }
    }

    tracing::info!("");
    tracing::info!("═══ Resumen Dry-Run ═══");
    tracing::info!("  Total historias: {}", stories.len());
    let done = stories.iter().filter(|s| s.status == Status::Done).count();
    let failed = stories
        .iter()
        .filter(|s| s.status == Status::Failed)
        .count();
    let blocked = stories
        .iter()
        .filter(|s| s.status == Status::Blocked)
        .count();
    let draft = stories.iter().filter(|s| s.status == Status::Draft).count();
    tracing::info!("  Done:           {done}");
    tracing::info!("  Failed:         {failed}");
    tracing::info!("  Blocked:        {blocked}");
    tracing::info!("  Draft:          {draft}");
    tracing::info!("  Iteraciones estimadas: {iteration}");
    // Tiempo estimado: ~5 min por iteración como promedio entre agentes
    let est_minutes = iteration as u64 * 5;
    tracing::info!(
        "  Tiempo estimado: ~{}-{} min",
        est_minutes,
        est_minutes * 2
    );

    build_report(
        &stories,
        iteration,
        start.elapsed(),
        &story_iterations,
        &reject_cycles,
        &story_errors,
        None, // dry-run no tiene stop_reason relevante
    )
}

/// Construye el RunReport final a partir del estado de las historias.
fn build_report(
    stories: &[Story],
    iterations: u32,
    elapsed: std::time::Duration,
    story_iterations: &HashMap<String, u32>,
    reject_cycles: &HashMap<String, u32>,
    story_errors: &HashMap<String, String>,
    stop_reason: Option<String>,
) -> anyhow::Result<RunReport> {
    let done = stories.iter().filter(|s| s.status == Status::Done).count();
    let failed = stories
        .iter()
        .filter(|s| s.status == Status::Failed)
        .count();
    let blocked = stories
        .iter()
        .filter(|s| s.status == Status::Blocked)
        .count();
    let draft = stories.iter().filter(|s| s.status == Status::Draft).count();
    let total = stories.len();

    let story_records: Vec<StoryRecord> = stories
        .iter()
        .map(|s| {
            let iter_count = story_iterations.get(&s.id).copied().unwrap_or(0);
            let rej_count = reject_cycles.get(&s.id).copied().unwrap_or(0);
            let error = story_errors.get(&s.id).cloned();
            StoryRecord {
                id: s.id.clone(),
                status: s.status.to_string(),
                epic: s.epic.clone(),
                iterations: iter_count,
                reject_cycles: rej_count,
                error,
            }
        })
        .collect();

    Ok(RunReport {
        total,
        done,
        failed,
        blocked,
        draft,
        iterations,
        elapsed,
        elapsed_seconds: elapsed.as_secs(),
        stories: story_records,
        stop_reason,
    })
}

/// Reporte final de la ejecución del orquestador.
#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub blocked: usize,
    pub draft: usize,
    pub iterations: u32,
    #[serde(skip)]
    pub elapsed: std::time::Duration,
    pub elapsed_seconds: u64,
    pub stories: Vec<StoryRecord>,
    /// Razón de parada temprana (None = pipeline terminó naturalmente).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Registro individual de una historia para el reporte JSON.
#[derive(Debug, Clone, Serialize)]
pub struct StoryRecord {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    pub iterations: u32,
    pub reject_cycles: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── helpers ──────────────────────────────────────────────────────────────

/// Carga todas las historias del directorio configurado.
fn load_all_stories(project_root: &Path, cfg: &Config) -> anyhow::Result<Vec<Story>> {
    let stories_dir = project_root.join(&cfg.project.stories_dir);
    let pattern = stories_dir.join(&cfg.project.story_pattern);

    let mut stories = vec![];
    for entry in glob::glob(pattern.to_str().unwrap())? {
        let path = entry?;
        match Story::load(&path) {
            Ok(story) => stories.push(story),
            Err(e) => tracing::warn!("Error cargando {}: {e}", path.display()),
        }
    }

    Ok(stories)
}

/// Aplica transiciones que ejecuta el orquestador sin intervención de agentes:
/// - Blocked → Ready: todas las dependencias están Done.
/// - * → Failed: se superó max_reject_cycles.
///
/// Si `simulate` es true, no escribe a disco (dry-run).
fn apply_automatic_transitions(
    stories: Vec<Story>,
    _graph: &DependencyGraph,
    reject_cycles: &mut HashMap<String, u32>,
    cfg: &Config,
    simulate: bool,
) -> anyhow::Result<Vec<Story>> {
    let mut stories = stories;

    // Primero verificamos ciclos de rechazo y marcamos Failed
    for story in stories.iter_mut() {
        if story.status.is_terminal() {
            continue;
        }
        let cycles = reject_cycles.get(&story.id).copied().unwrap_or(0);
        if cycles >= cfg.limits.max_reject_cycles {
            tracing::warn!(
                "❌ {}: {} ciclos de rechazo agotados → Failed",
                story.id,
                cycles
            );
            if simulate {
                story.advance_status_in_memory(Status::Failed);
            } else {
                story.set_status(Status::Failed)?;
            }
            continue;
        }

        // Si la historia está en flujo de rechazo (InProgress/InReview pero con ciclos altos)
        if cycles > 0 && cycles >= cfg.limits.max_reject_cycles {
            if simulate {
                story.advance_status_in_memory(Status::Failed);
            } else {
                story.set_status(Status::Failed)?;
            }
        }
    }

    // Luego: Blocked → Ready si dependencias resueltas
    let status_map: HashMap<String, Status> =
        stories.iter().map(|s| (s.id.clone(), s.status)).collect();

    for story in stories.iter_mut() {
        if story.status != Status::Blocked {
            continue;
        }

        let all_blockers_done = story
            .blockers
            .iter()
            .all(|b| status_map.get(b).is_some_and(|s| *s == Status::Done));

        if all_blockers_done {
            tracing::info!("🔓 {}: dependencias resueltas → Ready", story.id);
            if simulate {
                story.advance_status_in_memory(Status::Ready);
            } else {
                story.set_status(Status::Ready)?;
            }
        }
    }

    // Verificar si historias accionables tienen dependencias no resueltas → Blocked
    let status_map_after: HashMap<String, Status> =
        stories.iter().map(|s| (s.id.clone(), s.status)).collect();

    for story in stories.iter_mut() {
        if story.status.is_terminal() || story.status == Status::Blocked {
            continue;
        }
        if story.blockers.is_empty() {
            continue;
        }

        let any_blocker_not_done = story
            .blockers
            .iter()
            .any(|b| !status_map_after.get(b).is_some_and(|s| *s == Status::Done));

        if any_blocker_not_done {
            tracing::info!("⛔ {}: dependencias no resueltas → Blocked", story.id);
            if simulate {
                story.advance_status_in_memory(Status::Blocked);
            } else {
                story.set_status(Status::Blocked)?;
            }
        }
    }

    Ok(stories)
}

/// Procesa el resultado del deadlock analysis.
/// Retorna false si debemos salir del loop (pipeline completo).
fn handle_deadlock(
    resolution: &DeadlockResolution,
    _project_root: &Path,
    _cfg: &Config,
) -> anyhow::Result<bool> {
    match resolution {
        DeadlockResolution::PipelineComplete => {
            tracing::info!("✅ Pipeline completo.");
            Ok(false)
        }
        DeadlockResolution::InvokePoFor {
            story_id, reason, ..
        } => {
            tracing::info!("🔓 Deadlock → PO debe refinar {story_id}: {reason}");
            Ok(true)
        }
        DeadlockResolution::NoDeadlock => Ok(true),
    }
}

/// Elige la siguiente historia accionable con mayor prioridad.
///
/// Prioridad por estado + cantidad de historias que desbloquea.
fn pick_next_actionable<'a>(stories: &'a [Story], graph: &DependencyGraph) -> Option<&'a Story> {
    stories
        .iter()
        .filter(|s| s.status.is_actionable())
        .max_by_key(|s| {
            (
                status_priority(s.status),
                graph.blocks_count(&s.id),
                // Negativo del ID numérico para priorizar más bajos
                -(extract_numeric(&s.id) as i32),
            )
        })
}

/// Prioridad numérica de un estado (mayor = más urgente).
fn status_priority(status: Status) -> u32 {
    match status {
        Status::BusinessReview => 6,
        Status::InReview => 5,
        Status::InProgress => 4,
        Status::TestsReady => 3,
        Status::Ready => 2,
        _ => 0,
    }
}

/// Procesa una historia individual: dispara el agente correspondiente.
fn process_story(
    story: &Story,
    project_root: &Path,
    cfg: &Config,
    reject_cycles: &mut HashMap<String, u32>,
    agent_opts: &AgentOptions,
) -> anyhow::Result<()> {
    let ctx = PromptContext {
        story_id: story.id.clone(),
        stories_dir: cfg.project.stories_dir.clone(),
        decisions_dir: cfg.project.decisions_dir.clone(),
        last_rejection: story.last_rejection.clone(),
        from: story.status,
        to: next_status(story.status),
    };

    let (skill_path, prompt, label) = match story.status {
        Status::Draft => {
            let skill = project_root.join(&cfg.agents.product_owner);
            (skill, ctx.po_groom(), "PO (groom)")
        }
        Status::Ready => {
            let skill = project_root.join(&cfg.agents.qa_engineer);
            (skill, ctx.qa_tests(), "QA (tests)")
        }
        Status::TestsReady => {
            // Si el último actor es Dev, significa que reportó problemas con los tests.
            // QA debe corregirlos (TestsReady → TestsReady) en vez de Dev implementar.
            if story.last_actor().as_deref() == Some("Dev") {
                let qa_ctx = PromptContext {
                    to: Status::TestsReady,
                    story_id: ctx.story_id.clone(),
                    stories_dir: ctx.stories_dir.clone(),
                    decisions_dir: ctx.decisions_dir.clone(),
                    last_rejection: ctx.last_rejection.clone(),
                    from: ctx.from,
                };
                let skill = project_root.join(&cfg.agents.qa_engineer);
                (skill, qa_ctx.qa_fix_tests(), "QA (fix tests)")
            } else {
                let skill = project_root.join(&cfg.agents.developer);
                (skill, ctx.dev_implement(), "Dev (implement)")
            }
        }
        Status::InProgress => {
            let skill = project_root.join(&cfg.agents.developer);
            (skill, ctx.dev_fix(), "Dev (fix)")
        }
        Status::InReview => {
            let skill = project_root.join(&cfg.agents.reviewer);
            (skill, ctx.reviewer(), "Reviewer")
        }
        Status::BusinessReview => {
            let skill = project_root.join(&cfg.agents.product_owner);
            (skill, ctx.po_validate(), "PO (validate)")
        }
        _ => {
            tracing::warn!("{}: estado {} no procesable", story.id, story.status);
            return Ok(());
        }
    };

    tracing::info!(
        "  🎯 {label} | {} ({} → {})",
        story.id,
        story.status,
        ctx.to
    );

    // Snapshot git antes de la invocación (si está habilitado)
    let prev_hash = if cfg.git.enabled {
        crate::git::snapshot(project_root, &format!("{label}-{}", story.id))
    } else {
        None
    };

    let result = agent::invoke_with_retry(&skill_path, &prompt, &cfg.limits, agent_opts);

    match result {
        Ok(_) => {
            // Verificar que el agente realmente cambió el estado
            let updated = Story::load(&story.path)?;
            if updated.status == story.status {
                tracing::warn!(
                    "  ⚠ {}: el agente no cambió el estado (sigue en {})",
                    story.id,
                    story.status
                );
            } else if (updated.status == Status::InProgress || updated.status == Status::InReview)
                && (story.status == Status::InReview || story.status == Status::BusinessReview)
            {
                // El agente rechazó: incrementar contador
                let cycles = reject_cycles.entry(story.id.clone()).or_insert(0);
                *cycles += 1;
                tracing::info!(
                    "  📊 {}: ciclo de rechazo {}/{}",
                    story.id,
                    cycles,
                    cfg.limits.max_reject_cycles
                );
            }

            // Ejecutar hook post-fase si está definido
            let hook_result = match story.status {
                Status::Ready => crate::hooks::run_hook(cfg.hooks.post_qa.as_deref(), "post_qa"),
                Status::TestsReady | Status::InProgress => {
                    crate::hooks::run_hook(cfg.hooks.post_dev.as_deref(), "post_dev")
                }
                Status::InReview => {
                    crate::hooks::run_hook(cfg.hooks.post_reviewer.as_deref(), "post_reviewer")
                }
                _ => Ok(()),
            };

            if let Err(e) = hook_result {
                tracing::warn!("  ❌ hook falló: {e}");
                if let Some(ref hash) = prev_hash {
                    crate::git::rollback(project_root, hash, label);
                }
            }
        }
        Err(e) => {
            tracing::error!("  ❌ {}: falló la invocación del agente: {e}", story.id);
            // Rollback si hay snapshot
            if let Some(ref hash) = prev_hash {
                crate::git::rollback(project_root, hash, label);
            }
        }
    }

    Ok(())
}

/// Infiere el estado esperado tras la intervención del agente.
fn next_status(current: Status) -> Status {
    match current {
        Status::Draft => Status::Ready,
        Status::Ready => Status::TestsReady,
        Status::TestsReady => Status::InReview,
        Status::InProgress => Status::InReview,
        Status::InReview => Status::BusinessReview,
        Status::BusinessReview => Status::Done,
        _ => current,
    }
}

/// Etiqueta legible del actor para un estado dado.
fn actor_label(status: Status) -> &'static str {
    match status {
        Status::Draft => "PO (groom)",
        Status::Ready => "QA (tests)",
        Status::TestsReady => "Dev (implement)",
        Status::InProgress => "Dev (fix)",
        Status::InReview => "Reviewer",
        Status::BusinessReview => "PO (validate)",
        _ => "?",
    }
}

/// Extrae el número de un ID tipo "STORY-NNN".
fn extract_numeric(id: &str) -> u32 {
    id.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Calcula el número máximo efectivo de iteraciones.
///
/// Si el usuario configuró un valor explícito (>0), se respeta.
/// Si es 0 (default), se calcula como `max(10, story_count * 6)`
/// para escalar automáticamente con el tamaño del proyecto.
fn effective_max_iterations(cfg_max: u32, story_count: usize) -> u32 {
    if cfg_max > 0 {
        cfg_max
    } else {
        let computed = story_count as u32 * 6;
        computed.max(10)
    }
}

/// Construye AgentOptions con los valores de configuración actuales.
fn build_agent_opts(story: &Story, cfg: &Config) -> AgentOptions {
    AgentOptions {
        story_id: Some(story.id.clone()),
        decisions_dir: Some(Path::new(&cfg.project.decisions_dir).to_path_buf()),
        inject_feedback: cfg.limits.inject_feedback_on_retry,
    }
}

/// Guarda el checkpoint del orquestador.
fn save_checkpoint(
    project_root: &Path,
    iteration: u32,
    reject_cycles: &HashMap<String, u32>,
    story_iterations: &HashMap<String, u32>,
    story_errors: &HashMap<String, String>,
) {
    let state = OrchestratorState {
        iteration,
        reject_cycles: reject_cycles.clone(),
        story_iterations: story_iterations.clone(),
        story_errors: story_errors.clone(),
    };
    if let Err(e) = state.save(project_root) {
        tracing::warn!("⚠️  no se pudo guardar el checkpoint: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_priority_order() {
        assert!(status_priority(Status::BusinessReview) > status_priority(Status::InReview));
        assert!(status_priority(Status::InReview) > status_priority(Status::TestsReady));
        assert!(status_priority(Status::TestsReady) > status_priority(Status::Ready));
        assert!(status_priority(Status::Ready) > status_priority(Status::Draft));
    }

    #[test]
    fn next_status_follows_happy_path() {
        assert_eq!(next_status(Status::Draft), Status::Ready);
        assert_eq!(next_status(Status::Ready), Status::TestsReady);
        assert_eq!(next_status(Status::TestsReady), Status::InReview);
        assert_eq!(next_status(Status::InReview), Status::BusinessReview);
        assert_eq!(next_status(Status::BusinessReview), Status::Done);
    }

    #[test]
    fn next_status_fix_path() {
        assert_eq!(next_status(Status::InProgress), Status::InReview);
    }

    // ── filter_stories ──────────────────────────────────────────────

    fn story_fixture(id: &str, status: Status, epic: Option<&str>) -> Story {
        Story {
            id: id.to_string(),
            path: format!("stories/{id}.md").into(),
            status,
            epic: epic.map(|s| s.to_string()),
            blockers: vec![],
            last_rejection: None,
            raw_content: String::new(),
        }
    }

    #[test]
    fn filter_no_options_keeps_all() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, Some("EPIC-001")),
            story_fixture("STORY-002", Status::Draft, Some("EPIC-002")),
            story_fixture("STORY-003", Status::Done, None),
        ];
        let options = RunOptions::default();
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn filter_by_story_id_includes_only_match() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, None),
            story_fixture("STORY-002", Status::Draft, None),
            story_fixture("STORY-003", Status::Done, None),
        ];
        let options = RunOptions {
            story_filter: Some("STORY-002".into()),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "STORY-002");
    }

    #[test]
    fn filter_by_story_id_empty_when_no_match() {
        let stories = vec![story_fixture("STORY-001", Status::Ready, None)];
        let options = RunOptions {
            story_filter: Some("STORY-999".into()),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_by_epic_includes_only_matching_epic() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, Some("EPIC-001")),
            story_fixture("STORY-002", Status::Draft, Some("EPIC-001")),
            story_fixture("STORY-003", Status::Ready, Some("EPIC-002")),
            story_fixture("STORY-004", Status::Draft, None),
        ];
        let options = RunOptions {
            epic_filter: Some("EPIC-001".into()),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|s| s.epic.as_deref() == Some("EPIC-001")));
    }

    #[test]
    fn filter_by_epic_excludes_stories_without_epic() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, None),
            story_fixture("STORY-002", Status::Ready, Some("EPIC-001")),
        ];
        let options = RunOptions {
            epic_filter: Some("EPIC-001".into()),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "STORY-002");
    }

    #[test]
    fn filter_by_epics_range_inclusive() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, Some("EPIC-001")),
            story_fixture("STORY-002", Status::Draft, Some("EPIC-002")),
            story_fixture("STORY-003", Status::Ready, Some("EPIC-003")),
            story_fixture("STORY-004", Status::Draft, Some("EPIC-004")),
            story_fixture("STORY-005", Status::Ready, Some("EPIC-005")),
        ];
        let options = RunOptions {
            epics_range: Some(("EPIC-002".into(), "EPIC-004".into())),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 3);
        let ids: Vec<&str> = filtered.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"STORY-002"));
        assert!(ids.contains(&"STORY-003"));
        assert!(ids.contains(&"STORY-004"));
    }

    #[test]
    fn filter_by_epics_range_single_epic() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, Some("EPIC-001")),
            story_fixture("STORY-002", Status::Draft, Some("EPIC-001")),
            story_fixture("STORY-003", Status::Ready, Some("EPIC-002")),
        ];
        let options = RunOptions {
            epics_range: Some(("EPIC-001".into(), "EPIC-001".into())),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_combined_story_and_epic_both_must_match() {
        // Ambos filtros actúan como AND (aunque la CLI no permite combinarlos)
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, Some("EPIC-001")),
            story_fixture("STORY-002", Status::Draft, Some("EPIC-002")),
        ];
        let options = RunOptions {
            story_filter: Some("STORY-001".into()),
            epic_filter: Some("EPIC-001".into()),
            ..Default::default()
        };
        let filtered = filter_stories(stories, &options);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "STORY-001");
    }

    // ── RunOptions defaults ──────────────────────────────────────────

    #[test]
    fn run_options_default_has_no_filters() {
        let opts = RunOptions::default();
        assert!(!opts.once);
        assert!(opts.story_filter.is_none());
        assert!(opts.epic_filter.is_none());
        assert!(opts.epics_range.is_none());
    }

    // ── extract_numeric ──────────────────────────────────────────────

    #[test]
    fn extract_numeric_from_story_id() {
        assert_eq!(extract_numeric("STORY-001"), 1);
        assert_eq!(extract_numeric("STORY-042"), 42);
        assert_eq!(extract_numeric("story-007"), 7);
    }

    #[test]
    fn extract_numeric_from_epic_id() {
        assert_eq!(extract_numeric("EPIC-001"), 1);
        assert_eq!(extract_numeric("EPIC-010"), 10);
        assert_eq!(extract_numeric("EPIC-123"), 123);
    }

    #[test]
    fn extract_numeric_fallback_zero() {
        assert_eq!(extract_numeric("ABC"), 0);
        assert_eq!(extract_numeric(""), 0);
    }

    // ── pick_next_actionable ─────────────────────────────────────────

    #[test]
    fn pick_next_actionable_returns_highest_priority() {
        let stories = vec![
            story_fixture("STORY-001", Status::Ready, None),
            story_fixture("STORY-002", Status::BusinessReview, None),
            story_fixture("STORY-003", Status::TestsReady, None),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        let picked = pick_next_actionable(&stories, &graph);
        assert!(picked.is_some());
        // BusinessReview tiene la prioridad más alta
        assert_eq!(picked.unwrap().id, "STORY-002");
    }

    #[test]
    fn pick_next_actionable_breaks_tie_by_lower_id() {
        let stories = vec![
            story_fixture("STORY-005", Status::Ready, None),
            story_fixture("STORY-002", Status::Ready, None),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        let picked = pick_next_actionable(&stories, &graph);
        assert!(picked.is_some());
        // Mismo estado, gana ID más bajo
        assert_eq!(picked.unwrap().id, "STORY-002");
    }

    #[test]
    fn pick_next_actionable_returns_none_when_no_actionable() {
        let stories = vec![
            story_fixture("STORY-001", Status::Draft, None),
            story_fixture("STORY-002", Status::Done, None),
            story_fixture("STORY-003", Status::Blocked, None),
            story_fixture("STORY-004", Status::Failed, None),
        ];
        let graph = DependencyGraph::from_stories(&stories);
        let picked = pick_next_actionable(&stories, &graph);
        assert!(picked.is_none());
    }
}
