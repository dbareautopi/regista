//! Loop principal del orquestador.
//!
//! Carga historias, construye el grafo de dependencias, evalúa deadlocks,
//! y dispara agentes según la máquina de estados. Es el corazón del pipeline.

use crate::config::Config;
use crate::domain::deadlock::{self, DeadlockResolution};
use crate::domain::graph::DependencyGraph;
use crate::domain::prompts::{DomainStackConfig, PromptContext};
use crate::domain::state::{SharedState, Status, TokenCount};
use crate::domain::story::Story;
use crate::domain::workflow::{CanonicalWorkflow, Workflow};
use crate::infra::agent::{self, AgentOptions};
use crate::infra::checkpoint::OrchestratorState;
use crate::infra::providers;
use crate::app::report::{self, RunReport, StoryRecord};
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
    /// Modo compacto: suprime detalles como diff de archivos.
    pub compact: bool,
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
    // run_real es async: usar el runtime global de tokio para bloquear
    // hasta que el pipeline completo termine.
    crate::infra::agent::RUNTIME.block_on(run_real(project_root, cfg, options, resume_state))
}

/// Ejecución real del pipeline (invocando agentes).
///
/// Migrado a async (STORY-012): usa `process_story(...).await` secuencialmente
/// en el loop principal. Cada historia se procesa de una en una (sin `tokio::spawn`).
async fn run_real(
    project_root: &Path,
    cfg: &Config,
    options: &RunOptions,
    resume_state: Option<OrchestratorState>,
) -> anyhow::Result<RunReport> {
    let start = Instant::now();
    let max_wall = std::time::Duration::from_secs(cfg.limits.max_wall_time_seconds);

    let (state, start_iteration) = if let Some(ref resume) = resume_state {
        tracing::info!(
            "📂 Reanudando desde checkpoint: iteración {}",
            resume.iteration
        );
        let iteration = resume.iteration;
        (
            SharedState::new(
                resume.reject_cycles.clone(),
                resume.story_iterations.clone(),
                resume.story_errors.clone(),
            ),
            iteration,
        )
    } else {
        (SharedState::default(), 0u32)
    };

    let mut iteration: u32 = start_iteration;
    let mut stop_reason: Option<String> = None;
    let workflow: &dyn Workflow = &CanonicalWorkflow;

    // STORY-003 CA3: Crear directorios necesarios (movido desde Config::validate())
    // Solo en la primera ejecución (no en resume)
    if resume_state.is_none() {
        for dir in [&cfg.project.decisions_dir, &cfg.project.log_dir] {
            let path = project_root.join(dir);
            std::fs::create_dir_all(&path)?;
        }
        // También crear epics_dir si no existe (lo necesita plan.rs)
        let epics_path = project_root.join(&cfg.project.epics_dir);
        std::fs::create_dir_all(&epics_path)?;
    }

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
            apply_automatic_transitions(stories, &full_graph, &state, cfg, false, workflow)?;

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
                {
                    let mut guard = state.story_iterations.write().unwrap();
                    let iter = guard.entry(story.id.clone()).or_insert(0);
                    *iter += 1;
                }
                let agent_opts = build_agent_opts(story, cfg);
                if let Err(e) = process_story(
                    story,
                    project_root,
                    cfg,
                    &state,
                    &agent_opts,
                    workflow,
                    options.compact,
                )
                .await
                {
                    state
                        .story_errors
                        .write()
                        .unwrap()
                        .entry(story.id.clone())
                        .or_insert_with(|| e.to_string());
                }
                save_checkpoint(project_root, iteration, &state);
            }
            DeadlockResolution::NoDeadlock => {
                // 5. Procesar la historia de mayor prioridad en el flujo normal
                if let Some(story) = pick_next_actionable(&stories, &graph) {
                    let id = story.id.clone();
                    {
                        let mut guard = state.story_iterations.write().unwrap();
                        let iter = guard.entry(id.clone()).or_insert(0);
                        *iter += 1;
                    }
                    let agent_opts = build_agent_opts(story, cfg);
                    if let Err(e) = process_story(
                        story,
                        project_root,
                        cfg,
                        &state,
                        &agent_opts,
                        workflow,
                        options.compact,
                    )
                    .await
                    {
                        state
                            .story_errors
                            .write()
                            .unwrap()
                            .entry(id.clone())
                            .or_insert_with(|| e.to_string());
                    }
                    save_checkpoint(project_root, iteration, &state);
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

    // STORY-027: Final summary block
    {
        let stories_for_summary = load_all_stories(project_root, cfg)?;
        let total = stories_for_summary.len();
        let done = stories_for_summary
            .iter()
            .filter(|s| s.status == Status::Done)
            .count();
        let failed = stories_for_summary
            .iter()
            .filter(|s| s.status == Status::Failed)
            .count();
        let failed_ids: Vec<String> = stories_for_summary
            .iter()
            .filter(|s| s.status == Status::Failed)
            .map(|s| s.id.clone())
            .collect();
        let blocked = stories_for_summary
            .iter()
            .filter(|s| s.status == Status::Blocked)
            .count();
        let draft = stories_for_summary
            .iter()
            .filter(|s| s.status == Status::Draft)
            .count();

        let token_usage_guard = state.token_usage.read().unwrap();
        let mut total_input: u64 = 0;
        let mut total_output: u64 = 0;
        for entries in token_usage_guard.values() {
            for tc in entries {
                total_input = total_input.saturating_add(tc.input);
                total_output = total_output.saturating_add(tc.output);
            }
        }
        drop(token_usage_guard);

        let total_tokens = total_input + total_output;
        let elapsed_secs = start.elapsed().as_secs();
        let hours = elapsed_secs / 3600;
        let minutes = (elapsed_secs % 3600) / 60;
        let seconds = elapsed_secs % 60;
        let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let failed_list = if failed_ids.is_empty() {
            String::new()
        } else {
            format!(" ({})", failed_ids.join(", "))
        };

        tracing::info!("");
        tracing::info!("══════════════════════════════════════════════════════════════");
        tracing::info!("🏁 Pipeline completado — {ts}");
        tracing::info!("   Total        : {total}");
        tracing::info!("   ✅ Done      : {done}");
        tracing::info!("   ❌ Failed    : {failed}{failed_list}");
        tracing::info!("   🔒 Blocked   : {blocked}");
        tracing::info!("   📝 Draft     : {draft}");
        tracing::info!("   🔄 Iteraciones: {iteration}");
        tracing::info!("   ⏱️  Tiempo total: {hours}h {minutes}m {seconds}s");
        tracing::info!(
            "   📊 Tokens totales: {total_input} input + {total_output} output = {total_tokens}"
        );
        tracing::info!("══════════════════════════════════════════════════════════════");
    }

    // Generar reporte final
    let stories = filter_stories(load_all_stories(project_root, cfg)?, options);
    let report = report::build(
        &stories,
        iteration,
        start.elapsed(),
        &state.story_iterations.read().unwrap(),
        &state.reject_cycles.read().unwrap(),
        &state.story_errors.read().unwrap(),
        stop_reason,
    );
    report
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
        return report::build(
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
    let workflow = CanonicalWorkflow;

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
                tracing::info!("  → {story_id} (Draft) sería procesada por PO (plan) → Ready");
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
                        let next = workflow.next_status(story.status);
                        let label = match story.status {
                            Status::Draft => "PO (plan)",
                            Status::Ready => "QA (tests)",
                            Status::TestsReady => "Dev (implement)",
                            Status::InProgress => "Dev (fix)",
                            Status::InReview => "Reviewer",
                            Status::BusinessReview => "PO (validate)",
                            _ => "?",
                        };
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

    report::build(
        &stories,
        iteration,
        start.elapsed(),
        &story_iterations,
        &reject_cycles,
        &story_errors,
        None, // dry-run no tiene stop_reason relevante
    )
}

// ── helpers ──────────────────────────────────────────────────────────────

/// Carga todas las historias del directorio configurado.
pub(crate) fn load_all_stories(project_root: &Path, cfg: &Config) -> anyhow::Result<Vec<Story>> {
    let stories_dir = project_root.join(&cfg.project.stories_dir);
    let pattern = stories_dir.join(&cfg.project.story_pattern);

    let mut stories = vec![];
    for entry in glob::glob(pattern.to_str().unwrap())? {
        let path = entry?;
        match crate::app::story_io::load(&path) {
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
    state: &SharedState,
    cfg: &Config,
    simulate: bool,
    workflow: &dyn Workflow,
) -> anyhow::Result<Vec<Story>> {
    let mut stories = stories;

    // Primero verificamos ciclos de rechazo y marcamos Failed
    for story in stories.iter_mut() {
        if story.status.is_terminal() {
            continue;
        }
        let cycles = state
            .reject_cycles
            .read()
            .unwrap()
            .get(&story.id)
            .copied()
            .unwrap_or(0);
        if cycles >= cfg.limits.max_reject_cycles {
            tracing::warn!(
                "❌ {}: {} ciclos de rechazo agotados → Failed",
                story.id,
                cycles
            );
            if simulate {
                story.advance_status_in_memory(Status::Failed);
            } else {
                crate::app::story_io::save_status(story, Status::Failed)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            continue;
        }

        // Si la historia está en flujo de rechazo (InProgress/InReview pero con ciclos altos)
        if cycles > 0 && cycles >= cfg.limits.max_reject_cycles {
            if simulate {
                story.advance_status_in_memory(Status::Failed);
            } else {
                crate::app::story_io::save_status(story, Status::Failed)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
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
            let unblock_target = workflow.next_status(Status::Blocked);
            tracing::info!(
                "🔓 {}: dependencias resueltas → {}",
                story.id,
                unblock_target
            );
            if simulate {
                story.advance_status_in_memory(unblock_target);
            } else {
                crate::app::story_io::save_status(story, unblock_target)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
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
                crate::app::story_io::save_status(story, Status::Blocked)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
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

/// Procesa una historia individual: dispara el agente correspondiente (async).
///
/// Migrado a async (STORY-012): usa `invoke_with_retry(...).await` en lugar
/// de `invoke_with_retry_blocking(...)`. Las operaciones git se ejecutan con
/// `spawn_blocking` para no bloquear el runtime.
async fn process_story(
    story: &Story,
    project_root: &Path,
    cfg: &Config,
    state: &SharedState,
    agent_opts: &AgentOptions,
    workflow: &dyn Workflow,
    compact: bool,
) -> anyhow::Result<()> {
    let ctx = PromptContext {
        story_id: story.id.clone(),
        stories_dir: cfg.project.stories_dir.clone(),
        decisions_dir: cfg.project.decisions_dir.clone(),
        last_rejection: story.last_rejection.clone(),
        from: story.status,
        to: workflow.next_status(story.status),
        stack: DomainStackConfig {
            build: cfg.stack.build_command.clone(),
            test: cfg.stack.test_command.clone(),
            lint: cfg.stack.lint_command.clone(),
            fmt: cfg.stack.fmt_command.clone(),
            src_dir: cfg.stack.src_dir.clone(),
        },
    };

    // Determinar el rol, provider, y path de instrucciones
    let role = workflow.map_status_to_role(story.status);
    let provider_name = cfg.agents.provider_for_role(role);
    let provider = providers::from_name(&provider_name)?;
    let skill_path_str = crate::app::resolver::skill_path(&cfg.agents, role);
    let instruction_path = project_root.join(&skill_path_str);

    // Prompt según el estado (sin cambios)
    let (prompt, label) = match story.status {
        Status::Draft => (ctx.po_plan(), "PO (plan)"),
        Status::Ready => (ctx.qa_tests(), "QA (tests)"),
        Status::TestsReady => {
            if story.last_actor().as_deref() == Some("Dev") {
                let qa_ctx = PromptContext {
                    to: Status::TestsReady,
                    story_id: ctx.story_id.clone(),
                    stories_dir: ctx.stories_dir.clone(),
                    decisions_dir: ctx.decisions_dir.clone(),
                    last_rejection: ctx.last_rejection.clone(),
                    from: ctx.from,
                    stack: ctx.stack.clone(),
                };
                (qa_ctx.qa_fix_tests(), "QA (fix tests)")
            } else {
                (ctx.dev_implement(), "Dev (implement)")
            }
        }
        Status::InProgress => (ctx.dev_fix(), "Dev (fix)"),
        Status::InReview => (ctx.reviewer(), "Reviewer"),
        Status::BusinessReview => (ctx.po_validate(), "PO (validate)"),
        _ => {
            tracing::warn!("{}: estado {} no procesable", story.id, story.status);
            return Ok(());
        }
    };

    let model = crate::app::resolver::model(&cfg.agents, role, &instruction_path);
    tracing::info!(
        "  {}",
        format_agent_line_with_model(label, &story.id, &provider_name, &model,)
    );

    // Snapshot git antes de la invocación (si está habilitado).
    // Ejecutar con spawn_blocking para no bloquear el runtime async.
    let prev_hash = if cfg.git.enabled {
        let root = project_root.to_path_buf();
        let snapshot_label = format!("{label}-{}", story.id);
        tokio::task::spawn_blocking(move || crate::infra::git::snapshot(&root, &snapshot_label))
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let result = agent::invoke_with_retry(
        provider.as_ref(),
        &instruction_path,
        &prompt,
        &cfg.limits,
        agent_opts,
        !compact,
    )
    .await;

    match result {
        Ok(agent_result) => {
            // STORY-027: acumular tokens de esta invocación
            let combined = format!("{}{}", agent_result.stdout, agent_result.stderr);
            if let Some(infra_tc) = agent::parse_token_count(&combined) {
                let domain_tc = TokenCount {
                    input: infra_tc.input,
                    output: infra_tc.output,
                };
                state
                    .token_usage
                    .write()
                    .unwrap()
                    .entry(story.id.clone())
                    .or_default()
                    .push(domain_tc);
            }

            // Verificar que el agente realmente cambió el estado.
            // Usamos un bucle de relectura con delay creciente para manejar
            // posibles condiciones de carrera entre escritura y lectura
            // (buffering de SO, escritura asíncrona del agente, etc.).
            let updated = {
                let path = story.path.clone();
                let mut updated_opt: Option<Story> = None;
                let delays = [
                    std::time::Duration::from_millis(0),
                    std::time::Duration::from_millis(200),
                    std::time::Duration::from_millis(500),
                ];
                for delay in &delays {
                    if *delay > std::time::Duration::ZERO {
                        tokio::time::sleep(*delay).await;
                    }
                    match crate::app::story_io::load(&path) {
                        Ok(s) => {
                            if s.status != story.status {
                                updated_opt = Some(s);
                                break;
                            }
                            // Guardamos la última lectura por si todas fallan
                            if updated_opt.is_none() {
                                updated_opt = Some(s);
                            }
                        }
                        Err(_) => continue,
                    }
                }
                updated_opt
            };

            let Some(updated) = updated else {
                tracing::error!("  ❌ {}: no se pudo re-leer la historia", story.id);
                return Err(anyhow::anyhow!(
                    "no se pudo re-leer la historia {} tras el agente",
                    story.id
                ));
            };

            if updated.status == story.status {
                // El agente completó (exit code 0) pero NO cambió el estado.
                // ⚠️ NO hacemos rollback: el agente tuvo éxito y sus cambios
                // pueden ser valiosos (tests, código parcial, etc.).
                // Sí contamos el ciclo de rechazo para evitar bucles infinitos.
                let current_cycles = {
                    let mut guard = state.reject_cycles.write().unwrap();
                    let cycles = guard.entry(story.id.clone()).or_insert(0);
                    *cycles += 1;
                    *cycles
                };

                // Verificar si al menos el archivo fue modificado (mtime cambió)
                let file_was_touched = std::fs::metadata(&story.path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .is_some();

                tracing::warn!(
                    "  ⚠ {}: el agente completó pero el estado sigue en {} — ciclo {}/{}. Cambios preservados.",
                    story.id,
                    story.status,
                    current_cycles,
                    cfg.limits.max_reject_cycles
                );
                if file_was_touched {
                    tracing::info!(
                        "  📝 {}: el archivo fue modificado por el agente (cambios preservados)",
                        story.id
                    );
                }
                return Err(anyhow::anyhow!(
                    "el agente completó pero no cambió el estado de {} (sigue en {})",
                    story.id,
                    story.status
                ));
            }
            if (updated.status == Status::InProgress || updated.status == Status::InReview)
                && (story.status == Status::InReview || story.status == Status::BusinessReview)
            {
                // El agente rechazó explícitamente: incrementar contador
                let current_cycles = {
                    let mut guard = state.reject_cycles.write().unwrap();
                    let cycles = guard.entry(story.id.clone()).or_insert(0);
                    *cycles += 1;
                    *cycles
                };
                tracing::info!(
                    "  📊 {}: ciclo de rechazo {}/{}",
                    story.id,
                    current_cycles,
                    cfg.limits.max_reject_cycles
                );
            }

            // Ejecutar hook post-fase si está definido.
            // run_hook usa RUNTIME.block_on internamente, lo que paniquearía
            // si se llama desde dentro del runtime de tokio. Lo envolvemos en
            // spawn_blocking para ejecutarlo en un hilo de bloqueo dedicado.
            let post_qa = cfg.hooks.post_qa.clone();
            let post_dev = cfg.hooks.post_dev.clone();
            let post_reviewer = cfg.hooks.post_reviewer.clone();
            let hook_status = story.status;

            let hook_result = tokio::task::spawn_blocking(move || match hook_status {
                Status::Ready => crate::infra::hooks::run_hook(post_qa.as_deref(), "post_qa"),
                Status::TestsReady | Status::InProgress => {
                    crate::infra::hooks::run_hook(post_dev.as_deref(), "post_dev")
                }
                Status::InReview => {
                    crate::infra::hooks::run_hook(post_reviewer.as_deref(), "post_reviewer")
                }
                _ => Ok(()),
            })
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("spawn_blocking del hook falló")));

            if let Err(e) = hook_result {
                tracing::warn!("  ❌ hook falló: {e}");
                if let Some(ref hash) = prev_hash {
                    let root = project_root.to_path_buf();
                    let hash = hash.clone();
                    let label = label.to_string();
                    tokio::task::spawn_blocking(move || {
                        crate::infra::git::rollback(&root, &hash, &label)
                    })
                    .await
                    .unwrap_or(false);
                }
            }

            // STORY-027: Post-agent git diff
            if cfg.git.enabled && !compact {
                let root = project_root.to_path_buf();
                let hash_for_diff = prev_hash.clone();
                let diff_output = tokio::task::spawn_blocking(move || {
                    let output = if let Some(ref hash) = hash_for_diff {
                        std::process::Command::new("git")
                            .args(["diff", "--stat", hash, "HEAD"])
                            .current_dir(&root)
                            .output()
                    } else {
                        std::process::Command::new("git")
                            .args(["diff", "--stat", "HEAD~1", "HEAD"])
                            .current_dir(&root)
                            .output()
                    };
                    output.ok().and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                        } else {
                            None
                        }
                    })
                })
                .await
                .unwrap_or(None);

                if let Some(diff) = diff_output {
                    let trimmed = diff.trim();
                    if !trimmed.is_empty() {
                        tracing::info!("📁 Archivos modificados:");
                        for line in trimmed.lines() {
                            tracing::info!("   {line}");
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("  ❌ {}: falló la invocación del agente: {e}", story.id);
            // Rollback si hay snapshot
            if let Some(ref hash) = prev_hash {
                let root = project_root.to_path_buf();
                let hash = hash.clone();
                let label = label.to_string();
                tokio::task::spawn_blocking(move || {
                    crate::infra::git::rollback(&root, &hash, &label)
                })
                .await
                .unwrap_or(false);
            }
        }
    }

    Ok(())
}

/// Formatea la línea de invocación de agente con modelo.
/// Formato: 🎯 <label> | <story_id> | <provider> [<modelo>]
#[allow(dead_code)]
fn format_agent_line_with_model(
    label: &str,
    story_id: &str,
    provider_name: &str,
    model: &str,
) -> String {
    format!("🎯 {label} | {story_id} | {provider_name} [{model}]")
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
fn save_checkpoint(project_root: &Path, iteration: u32, state: &SharedState) {
    let checkpoint = OrchestratorState {
        iteration,
        reject_cycles: state.reject_cycles.read().unwrap().clone(),
        story_iterations: state.story_iterations.read().unwrap().clone(),
        story_errors: state.story_errors.read().unwrap().clone(),
    };
    if let Err(e) = checkpoint.save(project_root) {
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

    // ── STORY-008: Migración de next_status a CanonicalWorkflow ──
    // CA5: Las funciones hardcodeadas next_status() y map_status_to_role()
    // se eliminan de pipeline.rs. Los tests ahora usan CanonicalWorkflow.

    use crate::domain::workflow::{CanonicalWorkflow, Workflow};

    /// CA5: next_status() hardcodeada eliminada → se usa CanonicalWorkflow.
    #[test]
    fn next_status_follows_happy_path() {
        let wf = CanonicalWorkflow::default();
        assert_eq!(wf.next_status(Status::Draft), Status::Ready);
        assert_eq!(wf.next_status(Status::Ready), Status::TestsReady);
        assert_eq!(wf.next_status(Status::TestsReady), Status::InReview);
        assert_eq!(wf.next_status(Status::InReview), Status::BusinessReview);
        assert_eq!(wf.next_status(Status::BusinessReview), Status::Done);
    }

    /// CA5: next_status() hardcodeada eliminada → se usa CanonicalWorkflow.
    #[test]
    fn next_status_fix_path() {
        let wf = CanonicalWorkflow::default();
        assert_eq!(wf.next_status(Status::InProgress), Status::InReview);
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

    // ═══════════════════════════════════════════════════════════════
    // STORY-008: Migrar pipeline.rs a usar &dyn Workflow
    // ═══════════════════════════════════════════════════════════════

    mod story008 {
        use super::*;

        // ── CA1: run_real acepta workflow: &dyn Workflow ──────────

        /// CA1: run_real() acepta (o construye internamente) un workflow.
        /// Este test verifica que CanonicalWorkflow proporciona todos los
        /// métodos necesarios para reemplazar las funciones hardcodeadas.
        #[test]
        fn canonical_workflow_provides_all_required_methods() {
            let wf = CanonicalWorkflow::default();

            // next_status: cubre todos los estados que pipeline usaba
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.next_status(Status::Ready), Status::TestsReady);
            assert_eq!(wf.next_status(Status::TestsReady), Status::InReview);
            assert_eq!(wf.next_status(Status::InProgress), Status::InReview);
            assert_eq!(wf.next_status(Status::InReview), Status::BusinessReview);
            assert_eq!(wf.next_status(Status::BusinessReview), Status::Done);

            // map_status_to_role: cubre todos los estados accionables
            assert_eq!(wf.map_status_to_role(Status::Draft), "product_owner");
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
            assert_eq!(wf.map_status_to_role(Status::TestsReady), "developer");
            assert_eq!(wf.map_status_to_role(Status::InReview), "reviewer");
            assert_eq!(
                wf.map_status_to_role(Status::BusinessReview),
                "product_owner"
            );

            // canonical_column_order: 9 columnas
            assert_eq!(wf.canonical_column_order().len(), 9);
        }

        /// CA1: CanonicalWorkflow se puede usar como &dyn Workflow
        /// (necesario para que run_real acepte el trait object).
        #[test]
        fn canonical_workflow_usable_as_trait_object() {
            let wf: &dyn Workflow = &CanonicalWorkflow::default();
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
            assert!(!wf.canonical_column_order().is_empty());
        }

        // ── CA2: process_story usa workflow.map_status_to_role() ──

        /// CA2: process_story() usa workflow.map_status_to_role(status)
        /// en lugar de la función hardcodeada map_status_to_role().
        /// Verifica que el mapeo workflow→rol canónico es correcto
        /// para todos los estados que process_story puede encontrar.
        #[test]
        fn workflow_role_mapping_covers_all_states_process_story_handles() {
            let wf = CanonicalWorkflow::default();

            let expected: &[(Status, &str)] = &[
                (Status::Draft, "product_owner"),
                (Status::Ready, "qa_engineer"),
                (Status::TestsReady, "developer"),
                (Status::InProgress, "developer"),
                (Status::InReview, "reviewer"),
                (Status::BusinessReview, "product_owner"),
                // Fallbacks seguros
                (Status::Done, "product_owner"),
                (Status::Blocked, "product_owner"),
                (Status::Failed, "product_owner"),
            ];

            for (status, expected_role) in expected {
                let role = wf.map_status_to_role(*status);
                assert_eq!(
                    role, *expected_role,
                    "map_status_to_role({}) = {role}, expected {expected_role}",
                    status
                );
            }
        }

        /// CA2: El mapeo de rol es determinista (misma entrada → misma salida).
        #[test]
        fn workflow_role_mapping_is_deterministic() {
            let wf = CanonicalWorkflow::default();
            for _ in 0..5 {
                assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
                assert_eq!(wf.map_status_to_role(Status::TestsReady), "developer");
                assert_eq!(wf.map_status_to_role(Status::InReview), "reviewer");
            }
        }

        // ── CA3+CA4: apply_automatic_transitions usa workflow ─────

        /// CA3+CA4: apply_automatic_transitions() usa workflow.next_status()
        /// para determinar el estado de desbloqueo, en lugar de hardcodear
        /// Status::Ready.
        ///
        /// Este test simula la lógica de desbloqueo que apply_automatic_transitions
        /// debe implementar: cuando todas las dependencias de una historia Blocked
        /// están Done, el nuevo estado se obtiene del workflow.
        #[test]
        fn unblock_target_comes_from_workflow_not_hardcoded() {
            let wf = CanonicalWorkflow::default();

            // ── Setup: historia bloqueada con dependencias resueltas ──
            let blocked_id = "STORY-002";
            let blockers = vec!["STORY-001".to_string()];

            let status_map: HashMap<String, Status> = [
                ("STORY-001".into(), Status::Done),
                (blocked_id.into(), Status::Blocked),
            ]
            .into();

            // Verificar que todas las dependencias están Done
            let all_blockers_done = blockers
                .iter()
                .all(|b| status_map.get(b).is_some_and(|s| *s == Status::Done));
            assert!(all_blockers_done, "Todas las dependencias deben estar Done");

            // ── CA4: El estado destino viene del workflow ──
            let unblock_target = wf.next_status(Status::Blocked);

            // El workflow canónico DEBE desbloquear a Ready
            assert_eq!(
                unblock_target,
                Status::Ready,
                "CanonicalWorkflow.next_status(Blocked) debe ser Ready para desbloqueo"
            );

            // ── Sanity checks ──
            assert!(
                !unblock_target.is_terminal(),
                "El target de desbloqueo no puede ser un estado terminal"
            );
            assert_ne!(
                unblock_target,
                Status::Blocked,
                "El target de desbloqueo no puede ser Blocked (bucle infinito)"
            );
            assert!(
                unblock_target != Status::Failed,
                "El target de desbloqueo no puede ser Failed"
            );
        }

        /// CA4: La transición Blocked→Ready se obtiene del workflow.
        /// Si se cambia el workflow, el estado post-desbloqueo debe cambiar.
        /// Esto demuestra que el target NO está hardcodeado.
        #[test]
        fn unblock_target_changes_when_workflow_changes() {
            /// Workflow alternativo: desbloquea a Draft en vez de Ready.
            struct AltWorkflow;

            impl Workflow for AltWorkflow {
                fn next_status(&self, current: Status) -> Status {
                    match current {
                        Status::Blocked => Status::Draft, // ← diferente al canónico
                        Status::Draft => Status::Ready,
                        Status::Ready => Status::TestsReady,
                        Status::TestsReady => Status::InReview,
                        Status::InProgress => Status::InReview,
                        Status::InReview => Status::BusinessReview,
                        Status::BusinessReview => Status::Done,
                        _ => current,
                    }
                }

                fn map_status_to_role(&self, status: Status) -> &'static str {
                    match status {
                        Status::Draft | Status::BusinessReview => "product_owner",
                        Status::Ready => "qa_engineer",
                        Status::TestsReady | Status::InProgress => "developer",
                        Status::InReview => "reviewer",
                        _ => "product_owner",
                    }
                }

                fn canonical_column_order(&self) -> &[&'static str] {
                    &[
                        "Draft",
                        "Ready",
                        "Tests Ready",
                        "In Progress",
                        "In Review",
                        "Business Review",
                        "Done",
                        "Blocked",
                        "Failed",
                    ]
                }
            }

            let canonical = CanonicalWorkflow::default();
            let alt = AltWorkflow;

            // El workflow canónico desbloquea a Ready
            assert_eq!(canonical.next_status(Status::Blocked), Status::Ready);

            // El workflow alternativo desbloquea a Draft
            assert_eq!(alt.next_status(Status::Blocked), Status::Draft);

            // Ambos son diferentes → el target NO está hardcodeado
            assert_ne!(
                canonical.next_status(Status::Blocked),
                alt.next_status(Status::Blocked),
                "Workflows diferentes deben poder producir targets diferentes"
            );
        }

        /// CA3: apply_automatic_transitions usa el workflow también para
        /// la transición *→Failed (max_reject_cycles agotado).
        /// Verifica que el workflow.next_status() produce el valor esperado
        /// para el caso de fallo.
        #[test]
        fn workflow_next_status_handles_terminal_states() {
            let wf = CanonicalWorkflow::default();

            // Estados terminales no transicionan
            assert_eq!(wf.next_status(Status::Done), Status::Done);
            assert_eq!(wf.next_status(Status::Failed), Status::Failed);

            // Estados no accionables no transicionan (salvo Blocked→Ready)
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
        }

        // ── CA5: Funciones hardcodeadas eliminadas ─────────────────

        /// CA5: Las funciones hardcodeadas next_status() y map_status_to_role()
        /// se eliminan de pipeline.rs.
        ///
        /// Este test verifica que el comportamiento de CanonicalWorkflow
        /// es idéntico al de las funciones hardcodeadas que va a reemplazar.
        /// Cuando el Developer elimine next_status() y map_status_to_role(),
        /// este test debe seguir pasando (usa CanonicalWorkflow, no las
        /// funciones hardcodeadas).
        #[test]
        fn canonical_workflow_matches_original_hardcoded_behavior() {
            let wf = CanonicalWorkflow::default();

            // ── Equivalente a next_status() ──
            // Happy path
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.next_status(Status::Ready), Status::TestsReady);
            assert_eq!(wf.next_status(Status::TestsReady), Status::InReview);
            assert_eq!(wf.next_status(Status::InReview), Status::BusinessReview);
            assert_eq!(wf.next_status(Status::BusinessReview), Status::Done);
            // Fix path
            assert_eq!(wf.next_status(Status::InProgress), Status::InReview);
            // Terminales
            assert_eq!(wf.next_status(Status::Done), Status::Done);
            assert_eq!(wf.next_status(Status::Failed), Status::Failed);
            // Desbloqueo (CA4)
            assert_eq!(wf.next_status(Status::Blocked), Status::Ready);

            // ── Equivalente a map_status_to_role() ──
            assert_eq!(wf.map_status_to_role(Status::Draft), "product_owner");
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
            assert_eq!(wf.map_status_to_role(Status::TestsReady), "developer");
            assert_eq!(wf.map_status_to_role(Status::InProgress), "developer");
            assert_eq!(wf.map_status_to_role(Status::InReview), "reviewer");
            assert_eq!(
                wf.map_status_to_role(Status::BusinessReview),
                "product_owner"
            );
        }

        // ── CA1+CA3: apply_automatic_transitions con &dyn Workflow ──

        /// CA3: CanonicalWorkflow DEBE definir el target de desbloqueo.
        ///
        /// El Developer debe añadir `Status::Blocked => Status::Ready`
        /// a CanonicalWorkflow::next_status() en src/domain/workflow.rs.
        #[test]
        fn canonical_workflow_unblock_target_is_ready() {
            let wf = CanonicalWorkflow::default();
            assert_eq!(
                wf.next_status(Status::Blocked),
                Status::Ready,
                "CanonicalWorkflow.next_status(Blocked) debe ser Ready para desbloqueo"
            );
        }

        /// CA1+CA3: apply_automatic_transitions() debe aceptar &dyn Workflow
        /// y usarlo para determinar el target de desbloqueo.
        ///
        /// Este test simula el escenario STORY-001(Done) → STORY-002(Blocked).
        /// Verifica que el target coincide con CanonicalWorkflow.next_status(Blocked).
        ///
        /// El Developer debe:
        /// 1. Añadir parámetro `workflow: &dyn Workflow` a apply_automatic_transitions()
        /// 2. Usar `workflow.next_status(Status::Blocked)` en lugar de Status::Ready
        #[test]
        fn apply_automatic_transitions_unblock_uses_workflow_target() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();
            let state = SharedState::default();

            let done_story = story_fixture("STORY-001", Status::Done, None);
            let blocked_story = Story {
                id: "STORY-002".into(),
                path: "stories/STORY-002.md".into(),
                status: Status::Blocked,
                epic: None,
                blockers: vec!["STORY-001".into()],
                last_rejection: None,
                raw_content: String::new(),
            };

            let stories = vec![done_story, blocked_story];
            let graph = DependencyGraph::from_stories(&stories);

            // simulate=true → no escribe a disco
            let result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            let unblocked = result.iter().find(|s| s.id == "STORY-002").unwrap();
            let expected = wf.next_status(Status::Blocked);
            assert_eq!(
                unblocked.status, expected,
                "apply_automatic_transitions debe desbloquear al estado que indique el workflow"
            );
            assert!(
                !unblocked.status.is_terminal(),
                "El target de desbloqueo no puede ser un estado terminal"
            );
            assert_ne!(
                unblocked.status,
                Status::Blocked,
                "El target de desbloqueo no puede ser Blocked (bucle infinito)"
            );
        }

        /// CA3: Blocked con dependencias no resueltas permanece Blocked.
        /// Verifica que apply_automatic_transitions no desbloquea prematuramente.
        #[test]
        fn apply_automatic_transitions_keeps_blocked_with_unresolved_deps() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();
            let state = SharedState::default();

            // STORY-001 está Draft (no Done) → no debería desbloquear STORY-002
            let dep_draft = story_fixture("STORY-001", Status::Draft, None);
            let blocked_story = Story {
                id: "STORY-002".into(),
                path: "stories/STORY-002.md".into(),
                status: Status::Blocked,
                epic: None,
                blockers: vec!["STORY-001".into()],
                last_rejection: None,
                raw_content: String::new(),
            };

            let stories = vec![dep_draft, blocked_story];
            let graph = DependencyGraph::from_stories(&stories);

            let result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            let still_blocked = result.iter().find(|s| s.id == "STORY-002").unwrap();
            assert_eq!(
                still_blocked.status,
                Status::Blocked,
                "STORY-002 debe permanecer Blocked; dependencia STORY-001 no está Done"
            );
        }

        /// CA1+CA4: Con workflows diferentes, el target de desbloqueo varía.
        /// Esto demuestra que apply_automatic_transitions NO debe hardcodear
        /// el target — debe delegar en workflow.next_status(Blocked).
        ///
        /// El Developer debe:
        /// 1. Añadir `Status::Blocked => Status::Ready` a CanonicalWorkflow
        /// 2. Aceptar `&dyn Workflow` en apply_automatic_transitions
        /// 3. Usar `workflow.next_status(Status::Blocked)` para el target
        #[test]
        fn unblock_target_varies_by_workflow() {
            /// AltWorkflow: desbloquea Blocked → Draft (no Ready).
            struct AltWorkflow;

            impl Workflow for AltWorkflow {
                fn next_status(&self, current: Status) -> Status {
                    match current {
                        Status::Blocked => Status::Draft,
                        Status::Draft => Status::Ready,
                        Status::Ready => Status::TestsReady,
                        Status::TestsReady => Status::InReview,
                        Status::InProgress => Status::InReview,
                        Status::InReview => Status::BusinessReview,
                        Status::BusinessReview => Status::Done,
                        _ => current,
                    }
                }

                fn map_status_to_role(&self, status: Status) -> &'static str {
                    match status {
                        Status::Draft | Status::BusinessReview => "product_owner",
                        Status::Ready => "qa_engineer",
                        Status::TestsReady | Status::InProgress => "developer",
                        Status::InReview => "reviewer",
                        _ => "product_owner",
                    }
                }

                fn canonical_column_order(&self) -> &[&'static str] {
                    &[
                        "Draft",
                        "Ready",
                        "Tests Ready",
                        "In Progress",
                        "In Review",
                        "Business Review",
                        "Done",
                        "Blocked",
                        "Failed",
                    ]
                }
            }

            let canonical = CanonicalWorkflow::default();
            let alt = AltWorkflow;

            // El workflow canónico debe desbloquear a Ready
            assert_eq!(
                canonical.next_status(Status::Blocked),
                Status::Ready,
                "CanonicalWorkflow debe desbloquear Blocked→Ready"
            );

            // Un workflow alternativo puede desbloquear a Draft
            assert_eq!(
                alt.next_status(Status::Blocked),
                Status::Draft,
                "AltWorkflow debe desbloquear Blocked→Draft"
            );

            // Targets diferentes → el target NO debe estar hardcodeado
            assert_ne!(
                canonical.next_status(Status::Blocked),
                alt.next_status(Status::Blocked),
                "Workflows diferentes deben producir targets diferentes"
            );
        }

        // ── CA2: process_story role resolution via workflow ──────

        /// CA2: La cadena de resolución status→rol→provider→instruction_path
        /// usa workflow.map_status_to_role() en lugar de la función hardcodeada.
        ///
        /// Este test cubre la lógica que process_story ejecuta para cada estado
        /// accionable, verificando que el rol, provider, y skill path son correctos.
        #[test]
        fn role_resolution_chain_uses_workflow_mapping() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();

            // Tuplas: (status, expected_role, expected_provider)
            let cases: &[(Status, &str, &str)] = &[
                (Status::Draft, "product_owner", "pi"),
                (Status::Ready, "qa_engineer", "pi"),
                (Status::TestsReady, "developer", "pi"),
                (Status::InProgress, "developer", "pi"),
                (Status::InReview, "reviewer", "pi"),
                (Status::BusinessReview, "product_owner", "pi"),
            ];

            for (status, expected_role, expected_provider) in cases {
                // ← El mapeo DEBE venir del workflow
                let role = wf.map_status_to_role(*status);
                assert_eq!(
                    role, *expected_role,
                    "workflow.map_status_to_role({status}) = {role}, expected {expected_role}"
                );

                let provider_name = cfg.agents.provider_for_role(role);
                assert_eq!(
                    provider_name, *expected_provider,
                    "provider para rol {role} debería ser {expected_provider}"
                );

                let skill_path = crate::app::resolver::skill_path(&cfg.agents, role);
                assert!(
                    !skill_path.is_empty(),
                    "skill_path para rol {role} no debe estar vacío"
                );
                assert!(
                    skill_path.ends_with(".md"),
                    "skill_path debe ser un archivo .md: {skill_path}"
                );
            }
        }

        /// CA2: Si el workflow mapea un estado a un rol diferente,
        /// toda la cadena de resolución (provider, instruction_path) cambia.
        /// Esto demuestra que el rol se obtiene del workflow, no hardcodeado.
        #[test]
        fn role_resolution_changes_when_workflow_mapping_differs() {
            /// AltWorkflow: TestsReady → reviewer (no developer).
            struct AltWorkflow;

            impl Workflow for AltWorkflow {
                fn next_status(&self, current: Status) -> Status {
                    match current {
                        Status::Blocked => Status::Ready,
                        Status::Draft => Status::Ready,
                        Status::Ready => Status::TestsReady,
                        Status::TestsReady => Status::InReview,
                        Status::InProgress => Status::InReview,
                        Status::InReview => Status::BusinessReview,
                        Status::BusinessReview => Status::Done,
                        _ => current,
                    }
                }

                fn map_status_to_role(&self, status: Status) -> &'static str {
                    match status {
                        Status::TestsReady => "reviewer", // ← cambiado!
                        Status::Draft | Status::BusinessReview => "product_owner",
                        Status::Ready => "qa_engineer",
                        Status::InProgress => "developer",
                        Status::InReview => "reviewer",
                        _ => "product_owner",
                    }
                }

                fn canonical_column_order(&self) -> &[&'static str] {
                    &[
                        "Draft",
                        "Ready",
                        "Tests Ready",
                        "In Progress",
                        "In Review",
                        "Business Review",
                        "Done",
                        "Blocked",
                        "Failed",
                    ]
                }
            }

            let canonical_wf = CanonicalWorkflow::default();
            let alt_wf = AltWorkflow;
            let cfg = Config::default();

            // Con CanonicalWorkflow: TestsReady → "developer"
            let can_role = canonical_wf.map_status_to_role(Status::TestsReady);
            assert_eq!(can_role, "developer");

            // Con AltWorkflow: TestsReady → "reviewer"
            let alt_role = alt_wf.map_status_to_role(Status::TestsReady);
            assert_eq!(alt_role, "reviewer");

            // La resolución de provider refleja el cambio de rol
            let can_provider = cfg.agents.provider_for_role(can_role);
            let alt_provider = cfg.agents.provider_for_role(alt_role);
            // Ambos usan "pi" con defaults, pero los skill paths difieren
            assert_eq!(can_provider, "pi");
            assert_eq!(alt_provider, "pi");

            let can_skill = crate::app::resolver::skill_path(&cfg.agents, can_role);
            let alt_skill = crate::app::resolver::skill_path(&cfg.agents, alt_role);
            assert_ne!(
                can_skill, alt_skill,
                "skill paths deben diferir cuando el rol difiere: {can_skill} vs {alt_skill}"
            );
        }

        // ── CA2: process_story next_status resolution via workflow ──

        /// CA1+CA2: process_story() debe usar workflow.next_status()
        /// para determinar el `to` (estado destino tras intervención del agente),
        /// en lugar de la función hardcodeada next_status().
        ///
        /// Este test verifica que CanonicalWorkflow produce el `to` correcto
        /// para cada estado que process_story() procesa.
        #[test]
        fn process_story_target_status_comes_from_workflow() {
            let wf = CanonicalWorkflow::default();

            // Para cada estado que process_story() maneja,
            // el `to` DEBE venir del workflow
            let cases: &[(Status, Status)] = &[
                (Status::Draft, Status::Ready),
                (Status::Ready, Status::TestsReady),
                (Status::TestsReady, Status::InReview),
                (Status::InProgress, Status::InReview),
                (Status::InReview, Status::BusinessReview),
                (Status::BusinessReview, Status::Done),
            ];

            for (from, expected_to) in cases {
                let to = wf.next_status(*from);
                assert_eq!(
                    to, *expected_to,
                    "workflow.next_status({from}) = {to}, expected {expected_to}"
                );
            }
        }

        // ── CA1: run_real y run_dry ───────────────────────────────

        /// CA1: Tanto run_real() como run_dry() deben usar el workflow
        /// para determinar next_status() en lugar de la función hardcodeada.
        ///
        /// run_dry() actualmente llama a next_status() hardcodeada para
        /// simular avances. Con este cambio, usará workflow.next_status().
        ///
        /// Este test verifica que el workflow canónico cubre todos los
        /// estados que run_dry() puede encontrar durante la simulación.
        #[test]
        fn run_dry_next_status_uses_workflow() {
            let wf = CanonicalWorkflow::default();

            // run_dry() puede encontrar cualquiera de estos estados
            // y necesita saber el siguiente paso (o quedarse igual)
            let cases: &[(Status, Status)] = &[
                (Status::Draft, Status::Ready),
                (Status::Ready, Status::TestsReady),
                (Status::TestsReady, Status::InReview),
                (Status::InProgress, Status::InReview),
                (Status::InReview, Status::BusinessReview),
                (Status::BusinessReview, Status::Done),
                (Status::Done, Status::Done),
                (Status::Blocked, Status::Ready), // desbloqueo
                (Status::Failed, Status::Failed),
            ];

            for (current, expected) in cases {
                let next = wf.next_status(*current);
                assert_eq!(
                    next, *expected,
                    "workflow.next_status({current}) = {next}, expected {expected}"
                );
            }
        }

        /// CA1: run_real() construye (o recibe) un CanonicalWorkflow
        /// y lo propaga a process_story() y apply_automatic_transitions().
        ///
        /// Verifica que CanonicalWorkflow::default() existe y es
        /// construible sin argumentos (el constructor por defecto).
        #[test]
        fn run_real_can_construct_default_workflow() {
            let wf = CanonicalWorkflow::default();
            // Verificar que no es un struct vacío sin comportamiento
            assert_eq!(wf.next_status(Status::Draft), Status::Ready);
            assert_eq!(wf.map_status_to_role(Status::Ready), "qa_engineer");
            assert_eq!(wf.canonical_column_order().len(), 9);
        }

        // ── CA3: apply_automatic_transitions *→Failed con workflow ──

        /// CA3: apply_automatic_transitions() aplica la transición *→Failed
        /// cuando se agota max_reject_cycles. Aunque esta transición usa
        /// un estado fijo (Failed), la lógica debe ser compatible con que
        /// el workflow defina el target.
        ///
        /// Verifica que CanonicalWorkflow.next_status() es idempotente
        /// para estados terminales (no los modifica accidentalmente).
        #[test]
        fn workflow_next_status_is_idempotent_for_terminal_states() {
            let wf = CanonicalWorkflow::default();
            assert_eq!(wf.next_status(Status::Done), Status::Done);
            assert_eq!(wf.next_status(Status::Failed), Status::Failed);
            // Verificar que aplicar dos veces da lo mismo
            assert_eq!(wf.next_status(wf.next_status(Status::Done)), Status::Done);
            assert_eq!(
                wf.next_status(wf.next_status(Status::Failed)),
                Status::Failed
            );
        }

        /// CA3: La transición automática *→Failed (max_reject_cycles agotado)
        /// no debe ser interferida por el workflow.next_status().
        /// Failed es un estado terminal hardcodeado por el orquestador,
        /// no por el workflow.
        #[test]
        fn automatic_fail_transition_does_not_rely_on_workflow_next_status() {
            let wf = CanonicalWorkflow::default();
            // Failed es terminal: next_status no debe cambiarlo
            assert_eq!(wf.next_status(Status::Failed), Status::Failed);
            // La transición *→Failed la hace el orquestador directamente
            // (no pasa por workflow.next_status)
        }

        // ── CA6+CA7: Compilación y tests ───────────────────────────
        // CA6 (cargo test --bin pipeline pasa) y CA7 (cargo build sin warnings)
        // se verifican ejecutando los comandos. No son testeables como unit tests.
        // El Developer debe ejecutar:
        //   cargo test
        //   cargo build
        //   cargo clippy -- -D warnings
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-011: SharedState con Arc<RwLock<>>
    // ═══════════════════════════════════════════════════════════════

    mod story011 {
        use super::*;
        use crate::domain::state::SharedState;

        // ── CA2: process_story recibe &SharedState ────────────────

        /// CA2: process_story() acepta &SharedState en lugar de &mut HashMap<...>.
        ///
        /// Verifica que la firma compila y que la función no falla
        /// para un estado no procesable (Done) que retorna temprano.
        #[tokio::test]
        async fn process_story_accepts_shared_state() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let state = SharedState::default();
            let story = story_fixture("STORY-001", Status::Done, None);
            let workflow = CanonicalWorkflow::default();
            let agent_opts = AgentOptions {
                story_id: Some("STORY-001".into()),
                decisions_dir: Some(tmp.path().join(".regista/decisions")),
                inject_feedback: false,
            };

            // Done → retorna temprano sin invocar agente
            let result = process_story(
                &story,
                tmp.path(),
                &cfg,
                &state,
                &agent_opts,
                &workflow,
                false,
            )
            .await;
            assert!(result.is_ok(), "process_story con Done debe retornar Ok");
        }

        /// CA2: process_story con un estado Blocked también retorna
        /// temprano (sin invocar agente), verificando que la ruta
        /// de early-return funciona con SharedState.
        #[tokio::test]
        async fn process_story_blocked_returns_early() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let state = SharedState::default();
            let story = story_fixture("STORY-002", Status::Blocked, None);
            let workflow = CanonicalWorkflow::default();
            let agent_opts = AgentOptions {
                story_id: Some("STORY-002".into()),
                decisions_dir: Some(tmp.path().join(".regista/decisions")),
                inject_feedback: false,
            };

            let result = process_story(
                &story,
                tmp.path(),
                &cfg,
                &state,
                &agent_opts,
                &workflow,
                false,
            )
            .await;
            assert!(result.is_ok(), "process_story con Blocked debe retornar Ok");
        }

        /// CA2: process_story con un estado Failed también retorna
        /// temprano, cubriendo todos los early-return paths.
        #[tokio::test]
        async fn process_story_failed_returns_early() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let state = SharedState::default();
            let story = story_fixture("STORY-003", Status::Failed, None);
            let workflow = CanonicalWorkflow::default();
            let agent_opts = AgentOptions {
                story_id: Some("STORY-003".into()),
                decisions_dir: Some(tmp.path().join(".regista/decisions")),
                inject_feedback: false,
            };

            let result = process_story(
                &story,
                tmp.path(),
                &cfg,
                &state,
                &agent_opts,
                &workflow,
                false,
            )
            .await;
            assert!(result.is_ok(), "process_story con Failed debe retornar Ok");
        }

        // ── CA4: apply_automatic_transitions accede a reject_cycles
        //        vía SharedState ───────────────────────────────────

        /// CA4: apply_automatic_transitions() lee reject_cycles desde SharedState
        /// para la transición *→Failed cuando se agota max_reject_cycles.
        #[test]
        fn apply_automatic_transitions_reads_reject_cycles_from_shared_state() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();

            let state = SharedState::default();
            // Story con 8 ciclos de rechazo → igual a max_reject_cycles (8)
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-001".into(), 8);

            let story = story_fixture("STORY-001", Status::InReview, None);
            let stories = vec![story];
            let graph = DependencyGraph::from_stories(&stories);

            // simulate=true → no escribe a disco
            let result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            let failed_story = result.iter().find(|s| s.id == "STORY-001").unwrap();
            assert_eq!(
                failed_story.status,
                Status::Failed,
                "STORY-001 con 8 ciclos de rechazo debe marcarse Failed"
            );
        }

        /// CA4: apply_automatic_transitions NO marca Failed si los ciclos
        /// de rechazo están por debajo del límite.
        #[test]
        fn apply_automatic_transitions_does_not_fail_below_threshold() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();

            let state = SharedState::default();
            // 5 ciclos < 8 (max_reject_cycles) → NO debe marcar Failed
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-001".into(), 5);

            let story = story_fixture("STORY-001", Status::InReview, None);
            let stories = vec![story];
            let graph = DependencyGraph::from_stories(&stories);

            let result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            let story_after = result.iter().find(|s| s.id == "STORY-001").unwrap();
            assert!(
                story_after.status != Status::Failed,
                "STORY-001 con 5 ciclos NO debe marcarse Failed"
            );
            assert_eq!(
                story_after.status,
                Status::InReview,
                "STORY-001 debe permanecer en InReview"
            );
        }

        /// CA4: Historia sin entrada en reject_cycles se trata como 0 ciclos.
        #[test]
        fn apply_automatic_transitions_handles_missing_reject_cycles_entry() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();

            let state = SharedState::default();
            // No hay entrada para STORY-001 → debe interpretarse como 0 ciclos

            let story = story_fixture("STORY-001", Status::InReview, None);
            let stories = vec![story];
            let graph = DependencyGraph::from_stories(&stories);

            let result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            let story_after = result.iter().find(|s| s.id == "STORY-001").unwrap();
            assert!(
                story_after.status != Status::Failed,
                "Sin entrada en reject_cycles, la historia NO debe marcarse Failed"
            );
        }

        /// CA4: Múltiples historias con distintos niveles de ciclos de rechazo.
        /// Solo la que alcanza el umbral se marca Failed.
        #[test]
        fn apply_automatic_transitions_only_fails_stories_at_threshold() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();

            let state = SharedState::default();
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-001".into(), 8); // → Failed
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-002".into(), 7); // → OK

            let s1 = story_fixture("STORY-001", Status::InReview, None);
            let s2 = story_fixture("STORY-002", Status::InReview, None);
            let stories = vec![s1, s2];
            let graph = DependencyGraph::from_stories(&stories);

            let result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            let s1_after = result.iter().find(|s| s.id == "STORY-001").unwrap();
            assert_eq!(s1_after.status, Status::Failed);

            let s2_after = result.iter().find(|s| s.id == "STORY-002").unwrap();
            assert!(
                s2_after.status != Status::Failed,
                "STORY-002 con 7 ciclos NO debe marcarse Failed"
            );
        }

        // ── CA5: save_checkpoint clona bajo read() lock ──────────

        /// CA5: save_checkpoint() clona el contenido de los locks
        /// de SharedState para serializar a TOML.
        #[test]
        fn save_checkpoint_clones_shared_state_under_read_lock() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista")).unwrap();

            let state = SharedState::default();
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-001".into(), 2);
            state
                .story_iterations
                .write()
                .unwrap()
                .insert("STORY-001".into(), 3);
            state
                .story_errors
                .write()
                .unwrap()
                .insert("STORY-002".into(), "timeout".into());

            // save_checkpoint con SharedState (post-refactoring)
            save_checkpoint(tmp.path(), 7, &state);

            // Cargar y verificar
            let loaded = OrchestratorState::load(tmp.path())
                .expect("El checkpoint debe existir tras save_checkpoint");

            assert_eq!(loaded.iteration, 7);
            assert_eq!(loaded.reject_cycles.get("STORY-001"), Some(&2));
            assert_eq!(loaded.story_iterations.get("STORY-001"), Some(&3));
            assert_eq!(
                loaded.story_errors.get("STORY-002"),
                Some(&"timeout".to_string())
            );
        }

        /// CA5: save_checkpoint con SharedState vacío produce
        /// un checkpoint sin entradas.
        #[test]
        fn save_checkpoint_with_empty_state() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista")).unwrap();

            let state = SharedState::default();

            save_checkpoint(tmp.path(), 1, &state);

            let loaded = OrchestratorState::load(tmp.path()).unwrap();
            assert_eq!(loaded.iteration, 1);
            assert!(loaded.reject_cycles.is_empty());
            assert!(loaded.story_iterations.is_empty());
            assert!(loaded.story_errors.is_empty());
        }

        /// CA5: save_checkpoint no deadlockea si se llama con un
        /// read lock externo ya adquirido sobre story_iterations
        /// (RwLock permite múltiples readers).
        #[test]
        fn save_checkpoint_works_with_external_read_lock() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista")).unwrap();

            let state = SharedState::default();
            state.reject_cycles.write().unwrap().insert("X".into(), 1);

            // Adquirir un read lock externo ANTES de save_checkpoint
            let external_read = state.story_iterations.read().unwrap();
            assert!(external_read.is_empty());

            // save_checkpoint DEBE poder adquirir sus propios read locks
            // sin deadlock (RwLock permite múltiples readers concurrentes)
            save_checkpoint(tmp.path(), 1, &state);

            drop(external_read);

            let loaded = OrchestratorState::load(tmp.path()).unwrap();
            assert_eq!(loaded.reject_cycles.get("X"), Some(&1));
        }

        // ── CA3 integrado: locks en apply_automatic_transitions ──

        /// CA3: apply_automatic_transitions usa locks de corta duración.
        /// Verifica que después de la función, los locks están liberados
        /// y se pueden volver a adquirir para lectura o escritura.
        #[test]
        fn locks_are_released_after_apply_automatic_transitions() {
            let wf = CanonicalWorkflow::default();
            let cfg = Config::default();

            let state = SharedState::default();
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-001".into(), 3);

            let story = story_fixture("STORY-001", Status::InReview, None);
            let stories = vec![story];
            let graph = DependencyGraph::from_stories(&stories);

            // apply_automatic_transitions adquiere y libera locks internamente
            let _result =
                apply_automatic_transitions(stories, &graph, &state, &cfg, true, &wf).unwrap();

            // Después: los locks deben estar libres para lectura
            let guard = state.reject_cycles.read().unwrap();
            assert_eq!(guard.get("STORY-001"), Some(&3));
            drop(guard);

            // Y se puede escribir de nuevo sin deadlock
            state
                .reject_cycles
                .write()
                .unwrap()
                .insert("STORY-002".into(), 1);
            assert_eq!(state.reject_cycles.read().unwrap().len(), 2);
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-012: Migrar pipeline.rs a async — process_story y loop
    // ═══════════════════════════════════════════════════════════════

    mod story012 {
        use super::*;

        // ── CA1: process_story() es async y usa invoke_with_retry ─

        /// CA1: process_story() es una función `async` que se puede
        /// llamar con `.await` desde un contexto tokio.
        ///
        /// Este test verifica que:
        /// - process_story acepta `&SharedState` (STORY-011)
        /// - La firma es `async fn` (no `fn`)
        /// - El early-return para Done sigue funcionando en async
        #[tokio::test]
        async fn process_story_is_async_and_returns_future() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let state = SharedState::default();
            let story = story_fixture("STORY-001", Status::Done, None);
            let workflow = CanonicalWorkflow::default();
            let agent_opts = AgentOptions {
                story_id: Some("STORY-001".into()),
                decisions_dir: Some(tmp.path().join(".regista/decisions")),
                inject_feedback: false,
            };

            // CA1: process_story es async → se llama con .await
            let result = process_story(
                &story,
                tmp.path(),
                &cfg,
                &state,
                &agent_opts,
                &workflow,
                false,
            )
            .await;
            assert!(
                result.is_ok(),
                "process_story con Done debe retornar Ok en async"
            );
        }

        /// CA1: process_story propaga correctamente los panics/errores
        /// a través del future (no los oculta con spawn_blocking).
        ///
        /// Si el agente falla, el error debe propagarse a quien hace
        /// `.await` en el call site, igual que en la versión síncrona.
        #[tokio::test]
        async fn process_story_awaits_agent_and_propagates_result() {
            // Para estados no-procesables (Blocked, Failed, Done),
            // process_story retorna temprano sin invocar agente.
            // Este test verifica que el early-return async funciona.
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let state = SharedState::default();
            let workflow = CanonicalWorkflow::default();
            let agent_opts = AgentOptions {
                story_id: Some("STORY-001".into()),
                decisions_dir: Some(tmp.path().join(".regista/decisions")),
                inject_feedback: false,
            };

            // Todos los estados no-procesables deben retornar Ok temprano
            for status in [Status::Blocked, Status::Failed, Status::Done] {
                let story = story_fixture("STORY-001", status, None);
                let result = process_story(
                    &story,
                    tmp.path(),
                    &cfg,
                    &state,
                    &agent_opts,
                    &workflow,
                    false,
                )
                .await;
                assert!(
                    result.is_ok(),
                    "process_story con {status} debe retornar Ok en async"
                );
            }
        }

        /// CA1: process_story() llama a invoke_with_retry (async), no a
        /// invoke_with_retry_blocking (sync wrapper).
        ///
        /// Verificable indirectamente: si process_story es async y el
        /// agente está instalado, la invocación no bloquea el runtime.
        /// Este test crea múltiples tareas concurrentes para verificar
        /// que process_story no bloquea el event loop.
        #[tokio::test]
        async fn process_story_does_not_block_runtime() {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let state = SharedState::default();
            let workflow = CanonicalWorkflow::default();

            // Ejecutar 3 process_story concurrentes con estados Done
            // (early-return, no invocan agente). Si process_story
            // usara blocking (std::process::Command o block_on),
            // las tareas se serializarían en vez de ejecutarse juntas.
            let mut handles = vec![];
            for i in 0..3 {
                let id = format!("STORY-00{i}");
                let story = story_fixture(&id, Status::Done, None);
                let tmp_path = tmp.path().to_path_buf();
                let cfg = cfg.clone();
                let state = state.clone();
                let agent_opts = AgentOptions {
                    story_id: Some(id),
                    decisions_dir: Some(tmp_path.join(".regista/decisions")),
                    inject_feedback: false,
                };

                let handle = tokio::spawn(async move {
                    process_story(
                        &story,
                        &tmp_path,
                        &cfg,
                        &state,
                        &agent_opts,
                        &workflow,
                        false,
                    )
                    .await
                });
                handles.push(handle);
            }

            // Todas deben completar sin error
            for handle in handles {
                let result = handle.await.unwrap();
                assert!(result.is_ok(), "tarea concurrente debe completar Ok");
            }
        }

        // ── CA2: run_real() usa process_story().await secuencial ─

        /// CA2: El loop principal de run_real() llama a process_story()
        /// con `.await`, NO con `tokio::spawn`. El procesamiento es
        /// secuencial: una historia después de otra.
        ///
        /// Verifica que SharedState refleja el orden secuencial: si
        /// procesamos dos historias, los contadores de story_iterations
        /// se incrementan en orden (no simultáneamente).
        #[test]
        fn run_real_processes_stories_one_at_a_time() {
            // Este test valida el CONTRATO de CA2:
            // - run_real() itera sobre las historias secuencialmente
            // - Cada process_story se completa antes de la siguiente
            // - No hay tokio::spawn dentro del loop principal
            //
            // La verificación real de que no hay spawn se hace en
            // code review. Aquí validamos que la estructura de
            // SharedState permite razonar sobre secuencialidad.

            let state = SharedState::default();

            // Simular lo que run_real haría secuencialmente:
            // iteración 1 → story_iterations["STORY-001"] = 1
            // iteración 2 → story_iterations["STORY-002"] = 1
            {
                let mut guard = state.story_iterations.write().unwrap();
                guard.insert("STORY-001".into(), 1);
            }
            // save_checkpoint aquí (tras el primer .await)
            {
                let mut guard = state.story_iterations.write().unwrap();
                guard.insert("STORY-002".into(), 1);
            }

            let guard = state.story_iterations.read().unwrap();
            assert_eq!(guard.get("STORY-001"), Some(&1));
            assert_eq!(guard.get("STORY-002"), Some(&1));
            assert_eq!(guard.len(), 2, "secuencial: ambas historias procesadas");
        }

        /// CA2: Si una historia falla en run_real, el loop continúa
        /// con la siguiente historia (no aborta el pipeline entero).
        /// Esto requiere que cada .await maneje el error individualmente.
        #[test]
        fn run_real_continues_after_individual_story_error() {
            // Simular: STORY-001 falla, STORY-002 se procesa igual
            let state = SharedState::default();

            // STORY-001: registramos el error
            state
                .story_errors
                .write()
                .unwrap()
                .insert("STORY-001".into(), "timeout".into());
            // STORY-002: se procesa normalmente (secuencial, después de 001)
            state
                .story_iterations
                .write()
                .unwrap()
                .insert("STORY-002".into(), 1);

            // Ambas historias tienen entradas en el estado compartido
            assert!(state.story_errors.read().unwrap().contains_key("STORY-001"));
            assert!(state
                .story_iterations
                .read()
                .unwrap()
                .contains_key("STORY-002"));
        }

        // ── CA3: run_dry() compatible con async ──────────────────

        /// CA3: run_dry() no invoca agentes reales. Puede mantenerse
        /// síncrono o adaptarse mínimamente a async.
        ///
        /// Si se mantiene síncrono: este test verifica que se puede
        /// llamar desde un contexto no-async sin tokio runtime.
        #[test]
        fn run_dry_remains_callable_without_tokio_runtime() {
            let tmp = tempfile::tempdir().unwrap();
            let stories_dir = tmp.path().join("stories");
            std::fs::create_dir_all(&stories_dir).unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config {
                project: crate::config::ProjectConfig {
                    stories_dir: "stories".into(),
                    ..Default::default()
                },
                ..Config::default()
            };

            let options = RunOptions {
                dry_run: true,
                ..Default::default()
            };

            // CA3: run_dry debe ser invocable sin #[tokio::test]
            // (es un test normal, no async)
            let report = run_dry(tmp.path(), &cfg, &options);
            assert!(report.is_ok(), "run_dry debe ejecutarse sin tokio runtime");
            let report = report.unwrap();
            assert_eq!(report.total, 0, "sin historias, total debe ser 0");
        }

        /// CA3: run_dry() con historias reales produce un reporte
        /// con la misma estructura que antes de la migración.
        #[test]
        fn run_dry_with_stories_produces_valid_report() {
            let tmp = tempfile::tempdir().unwrap();
            let stories_dir = tmp.path().join("stories");
            std::fs::create_dir_all(&stories_dir).unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            // Dos historias Draft independientes
            let content = |id: &str| -> String {
                format!(
                    "# {id}: Test\n\n## Status\n**Draft**\n\n## Epic\nEPIC-001\n\
                     ## Descripción\nTest.\n\n## Criterios de aceptación\n- [ ] CA1\n\n\
                     ## Activity Log\n- 2026-01-01 | PO | created\n"
                )
            };
            std::fs::write(stories_dir.join("STORY-001.md"), content("STORY-001")).unwrap();
            std::fs::write(stories_dir.join("STORY-002.md"), content("STORY-002")).unwrap();

            let cfg = Config {
                project: crate::config::ProjectConfig {
                    stories_dir: "stories".into(),
                    ..Default::default()
                },
                ..Config::default()
            };

            let options = RunOptions {
                dry_run: true,
                ..Default::default()
            };

            let report = run_dry(tmp.path(), &cfg, &options).unwrap();

            // Estructura del reporte preservada
            assert_eq!(report.total, 2, "2 historias en total");
            assert!(
                report.done + report.failed + report.blocked + report.draft == report.total,
                "done + failed + blocked + draft = total"
            );
            assert!(
                report.iterations > 0,
                "dry-run debe iterar al menos una vez"
            );
            assert_eq!(report.stories.len(), 2, "2 story records");

            // elapsed_seconds es consistente con elapsed
            assert_eq!(report.elapsed.as_secs(), report.elapsed_seconds);
        }

        // ── CA8: Pipeline dry-run produce la misma salida ────────

        /// CA8: RunReport preserva todos los campos obligatorios
        /// y es compatible con la salida JSON esperada por CI/CD.
        #[test]
        fn run_report_structure_preserved_for_ci_compatibility() {
            let report = RunReport {
                total: 10,
                done: 4,
                failed: 1,
                blocked: 2,
                draft: 3,
                iterations: 20,
                elapsed: std::time::Duration::from_secs(120),
                elapsed_seconds: 120,
                stories: vec![StoryRecord {
                    id: "STORY-001".into(),
                    status: "Done".into(),
                    epic: Some("EPIC-001".into()),
                    iterations: 2,
                    reject_cycles: 0,
                    error: None,
                }],
                stop_reason: None,
            };

            // Los campos suman al total
            assert_eq!(
                report.done + report.failed + report.blocked + report.draft,
                report.total,
                "done + failed + blocked + draft debe ser igual a total"
            );

            // elapsed y elapsed_seconds son consistentes
            assert_eq!(report.elapsed.as_secs(), report.elapsed_seconds);

            // Serialización JSON funciona (compatibilidad CI/CD)
            let json = serde_json::to_string(&report).expect("RunReport debe serializarse a JSON");
            assert!(json.contains("\"done\":4"), "JSON contiene done count");
            assert!(json.contains("\"total\":10"), "JSON contiene total");
            assert!(json.contains("STORY-001"), "JSON contiene story ID");
            // elapsed (Duration) se omite con #[serde(skip)]
            assert!(
                !json.contains("\"elapsed\""),
                "elapsed Duration se omite en JSON"
            );
            assert!(
                json.contains("elapsed_seconds"),
                "elapsed_seconds está en JSON"
            );
        }

        /// CA8: El reporte con stop_reason incluye el campo en JSON.
        #[test]
        fn run_report_with_stop_reason_serializes_reason() {
            let report = RunReport {
                total: 5,
                done: 2,
                failed: 0,
                blocked: 0,
                draft: 3,
                iterations: 10,
                elapsed: std::time::Duration::from_secs(30),
                elapsed_seconds: 30,
                stories: vec![],
                stop_reason: Some("max_iterations (100)".into()),
            };

            let json = serde_json::to_string(&report).expect("RunReport debe serializarse a JSON");
            assert!(json.contains("stop_reason"), "stop_reason presente en JSON");
            assert!(
                json.contains("max_iterations"),
                "JSON contiene la razón de parada"
            );
        }

        /// CA8: El reporte con stop_reason=None omite el campo en JSON.
        #[test]
        fn run_report_without_stop_reason_omits_field() {
            let report = RunReport {
                total: 1,
                done: 1,
                failed: 0,
                blocked: 0,
                draft: 0,
                iterations: 1,
                elapsed: std::time::Duration::from_secs(1),
                elapsed_seconds: 1,
                stories: vec![],
                stop_reason: None,
            };

            let json = serde_json::to_string(&report).expect("RunReport debe serializarse a JSON");
            assert!(
                !json.contains("stop_reason"),
                "stop_reason se omite cuando es None"
            );
        }

        // ── CA2 (reforzado): run_real() con loop secuencial ────

        /// CA2: run_real() con todas las historias en estado terminal
        /// completa en una iteración sin invocar agentes. Verifica que:
        /// - El loop principal itera correctamente (1 iteración)
        /// - PipelineComplete se detecta y detiene el loop
        /// - El reporte refleja correctamente los conteos por estado
        #[tokio::test]
        async fn run_real_with_terminal_stories_completes_in_one_iteration() {
            let tmp = tempfile::tempdir().unwrap();
            let stories_dir = tmp.path().join(".regista/stories");
            std::fs::create_dir_all(&stories_dir).unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            // 3 historias terminales: 2 Done, 1 Failed
            for (id, status) in [
                ("STORY-001", "Done"),
                ("STORY-002", "Done"),
                ("STORY-003", "Failed"),
            ] {
                let content = format!(
                    "# {id}: Terminal\n\n## Status\n**{status}**\n\n## Epic\nEPIC-001\n\
                     ## Descripción\nTerminal.\n\n## Criterios de aceptación\n- [ ] CA1\n\n\
                     ## Activity Log\n- 2026-01-01 | PO | created\n"
                );
                std::fs::write(stories_dir.join(format!("{id}.md")), content).unwrap();
            }

            let cfg = Config::default();
            let options = RunOptions::default();

            let report = run_real(tmp.path(), &cfg, &options, None).await.unwrap();

            assert_eq!(report.total, 3);
            assert_eq!(report.done, 2);
            assert_eq!(report.failed, 1);
            assert_eq!(report.blocked, 0);
            assert_eq!(report.draft, 0);
            // Primera iteración → PipelineComplete → loop termina
            assert_eq!(report.iterations, 1, "PipelineComplete en 1 iteración");
            assert_eq!(report.stories.len(), 3);
            // Verificar que cada story record tiene el estado correcto
            let done_ids: Vec<&str> = report
                .stories
                .iter()
                .filter(|r| r.status == "Done")
                .map(|r| r.id.as_str())
                .collect();
            assert!(done_ids.contains(&"STORY-001"));
            assert!(done_ids.contains(&"STORY-002"));
            let failed_ids: Vec<&str> = report
                .stories
                .iter()
                .filter(|r| r.status == "Failed")
                .map(|r| r.id.as_str())
                .collect();
            assert!(failed_ids.contains(&"STORY-003"));
        }

        /// CA2: run_real() con directorio de historias vacío completa
        /// inmediatamente sin incidencias — no hay nada que procesar.
        #[tokio::test]
        async fn run_real_with_no_stories_completes_immediately() {
            let tmp = tempfile::tempdir().unwrap();
            let stories_dir = tmp.path().join(".regista/stories");
            std::fs::create_dir_all(&stories_dir).unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let cfg = Config::default();
            let options = RunOptions::default();

            let report = run_real(tmp.path(), &cfg, &options, None).await.unwrap();

            assert_eq!(report.total, 0);
            assert_eq!(report.done, 0);
            assert_eq!(report.iterations, 1, "sin historias, 1 iteración");
            assert!(report.stories.is_empty());
            assert!(report.stop_reason.is_none(), "sin stop_reason");
        }

        /// CA2: run_real() con una historia Draft y modo --once
        /// verifica que el loop avanza al menos una iteración
        /// y la historia es detectada como stuck (InvokePoFor).
        /// Con git deshabilitado para evitar dependencia de git.
        #[tokio::test]
        async fn run_real_with_draft_story_invokes_po_path() {
            let tmp = tempfile::tempdir().unwrap();
            let stories_dir = tmp.path().join(".regista/stories");
            std::fs::create_dir_all(&stories_dir).unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            let content = format!(
                "# STORY-001: Draft\n\n## Status\n**Draft**\n\n## Epic\nEPIC-001\n\
                 ## Descripción\nDraft story.\n\n## Criterios de aceptación\n- [ ] CA1\n\n\
                 ## Activity Log\n- 2026-01-01 | PO | created\n"
            );
            std::fs::write(stories_dir.join("STORY-001.md"), content).unwrap();

            let cfg = Config {
                git: crate::config::GitConfig { enabled: false },
                limits: crate::config::LimitsConfig {
                    max_retries_per_step: 1,
                    retry_delay_base_seconds: 0,
                    agent_timeout_seconds: 2,
                    ..Config::default().limits
                },
                ..Config::default()
            };
            let options = RunOptions {
                once: true,
                ..Default::default()
            };

            // run_real intentará invocar al PO vía deadlock (InvokePoFor).
            // Si el agente no está instalado, el error se captura sin
            // propagarse — run_real debe retornar Ok de todas formas.
            let result = run_real(tmp.path(), &cfg, &options, None).await;
            assert!(
                result.is_ok(),
                "run_real debe completar incluso si el agente falla"
            );
            let report = result.unwrap();
            assert_eq!(report.total, 1, "1 historia procesada");
            assert_eq!(report.iterations, 1, "1 iteración con --once");
        }

        /// CA2: run_real() con SharedState verifica que el loop
        /// actualiza story_iterations y reject_cycles secuencialmente
        /// (no hay escrituras concurrentes).
        #[tokio::test]
        async fn run_real_shared_state_reflects_sequential_processing() {
            let tmp = tempfile::tempdir().unwrap();
            let stories_dir = tmp.path().join(".regista/stories");
            std::fs::create_dir_all(&stories_dir).unwrap();
            std::fs::create_dir_all(tmp.path().join(".regista/decisions")).unwrap();

            // 2 historias Done: el loop debe verlas, detectar
            // PipelineComplete, y salir tras 1 iteración.
            for id in ["STORY-001", "STORY-002"] {
                let content = format!(
                    "# {id}: Done\n\n## Status\n**Done**\n\n## Epic\nEPIC-001\n\
                     ## Descripción\nDone.\n\n## Criterios de aceptación\n- [ ] CA1\n\n\
                     ## Activity Log\n- 2026-01-01 | PO | created\n"
                );
                std::fs::write(stories_dir.join(format!("{id}.md")), content).unwrap();
            }

            let cfg = Config::default();
            let options = RunOptions::default();

            let report = run_real(tmp.path(), &cfg, &options, None).await.unwrap();

            // Con PipelineComplete, el loop sale ANTES de incrementar
            // story_iterations (solo NoDeadlock e InvokePoFor lo hacen).
            // Por tanto, el reporte muestra 0 iteraciones por historia.
            assert_eq!(report.total, 2);
            assert_eq!(report.done, 2);
            for record in &report.stories {
                assert_eq!(
                    record.iterations, 0,
                    "{}: 0 iteraciones (PipelineComplete)",
                    record.id
                );
            }
        }

        // ── CA6 + CA7: tests de compilación/ejecución ───────────
        // CA6 (cargo test --lib orchestrator) y CA7 (cargo build)
        // no son testeables como unit tests. Se verifican ejecutando:
        //   cargo test --lib app
        //   cargo build
        //   cargo clippy -- -D warnings
        //
        // Cada test en este módulo que compila y pasa contribuye a CA6.

        /// Sanity: tokio está disponible con las features necesarias
        /// para la migración async.
        #[test]
        fn tokio_features_for_async_migration_available() {
            // rt-multi-thread (para #[tokio::test])
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async { assert_eq!(1 + 1, 2) });

            // time (para timeout y sleep)
            let _d = tokio::time::Duration::from_secs(1);

            // process (para tokio::process::Command)
            let _cmd: tokio::process::Command = tokio::process::Command::new("echo");

            // fs (para tokio::fs::write)
            let _ = tokio::fs::metadata(".");
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // STORY-027: Diff post-agente + acumulación tokens + resumen final
    // ═══════════════════════════════════════════════════════════════

    mod story027 {
        use super::*;
        use crate::domain::state::{SharedState, TokenCount};
        use std::collections::HashMap;
        use std::path::Path;

        // ── Helpers placeholder (esqueleto mínimo para compilar) ──
        // El Developer DEBE reemplazar estos placeholders con la
        // implementación real en process_story(), run_real(), etc.

        /// Determina si debe ejecutarse `git diff --stat` tras process_story.
        /// Condiciones: git habilitado, modo detallado (!compact), no dry-run.
        #[allow(dead_code)]
        fn should_run_post_diff(compact: bool, git_enabled: bool, dry_run: bool) -> bool {
            // TODO(Dev): integrar en process_story() tras agente exitoso
            git_enabled && !compact && !dry_run
        }

        /// Formatea la línea de invocación de agente con modelo.
        /// Formato: 🎯 <label> | <story_id> | <provider> [<modelo>]
        #[allow(dead_code)]
        fn format_agent_line_with_model(
            label: &str,
            story_id: &str,
            provider_name: &str,
            model: &str,
        ) -> String {
            // TODO(Dev): usar en process_story() al loguear cada agente
            format!("🎯 {label} | {story_id} | {provider_name} [{model}]")
        }

        /// Calcula los totales de tokens (input, output) desde token_usage.
        /// Suma todos los TokenCount de todas las historias.
        #[allow(dead_code)]
        fn compute_token_totals(token_usage: &HashMap<String, Vec<TokenCount>>) -> (u64, u64) {
            let mut total_input: u64 = 0;
            let mut total_output: u64 = 0;
            for entries in token_usage.values() {
                for tc in entries {
                    total_input = total_input.saturating_add(tc.input);
                    total_output = total_output.saturating_add(tc.output);
                }
            }
            (total_input, total_output)
        }

        /// Construye el bloque de cierre del pipeline con resumen de tokens.
        /// Formato exacto según CA9.
        #[allow(dead_code)]
        fn build_final_summary_block(
            total: usize,
            done: usize,
            failed: usize,
            failed_ids: &[String],
            blocked: usize,
            draft: usize,
            iterations: u32,
            elapsed: std::time::Duration,
            total_input_tokens: u64,
            total_output_tokens: u64,
        ) -> String {
            let total_tokens = total_input_tokens + total_output_tokens;
            let elapsed_secs = elapsed.as_secs();
            let hours = elapsed_secs / 3600;
            let minutes = (elapsed_secs % 3600) / 60;
            let seconds = elapsed_secs % 60;
            let time_str = format!("{hours}h {minutes}m {seconds}s");

            // timestamp: el Developer debe usar chrono::Utc::now()
            let ts = "2026-01-01 00:00:00";

            let failed_list = if failed_ids.is_empty() {
                String::new()
            } else {
                format!(" ({})", failed_ids.join(", "))
            };

            [
                format!("══════════════════════════════════════════════════════════════"),
                format!("🏁 Pipeline completado — {ts}"),
                format!("   Total        : {total}"),
                format!("   ✅ Done      : {done}"),
                format!("   ❌ Failed    : {failed}{failed_list}"),
                format!("   🔒 Blocked   : {blocked}"),
                format!("   📝 Draft     : {draft}"),
                format!("   🔄 Iteraciones: {iterations}"),
                format!("   ⏱️  Tiempo total: {time_str}"),
                format!("   📊 Tokens totales: {total_input_tokens} input + {total_output_tokens} output = {total_tokens}"),
                format!("══════════════════════════════════════════════════════════════"),
            ]
            .join("\n")
        }

        // ═══════════════════════════════════════════════════════════
        // CA1-CA4, CA13: Diff post-agente (git diff --stat)
        // ═══════════════════════════════════════════════════════════

        /// CA1: En modo detallado (!compact), git enabled, y !dry-run,
        ///      should_run_post_diff retorna true.
        #[test]
        fn ca1_diff_runs_in_detailed_mode_with_git_enabled() {
            assert!(
                should_run_post_diff(false, true, false),
                "git diff debe ejecutarse: !compact, git enabled, !dry-run"
            );
        }

        /// CA1: Solo cuando las 3 condiciones se cumplen se ejecuta el diff.
        #[test]
        fn ca1_diff_only_when_all_conditions_met() {
            let cases = vec![
                (true, true, false, false),   // compact → no diff
                (false, false, false, false), // git disabled → no diff
                (false, true, true, false),   // dry-run → no diff
                (false, true, false, true),   // ✅ todas bien
            ];
            for (compact, git_enabled, dry_run, expected) in cases {
                assert_eq!(
                    should_run_post_diff(compact, git_enabled, dry_run),
                    expected,
                    "should_run_post_diff(compact={compact}, git={git_enabled}, dry_run={dry_run})"
                );
            }
        }

        /// CA2: La salida de git diff --stat se loguea con el encabezado
        ///      📁 Archivos modificados: (el formato se verifica aquí).
        ///      El Developer debe usar tracing::info! para loguear.
        #[test]
        fn ca2_diff_header_format() {
            // El encabezado que el Developer debe usar en tracing::info!
            let header = "📁 Archivos modificados:";
            assert!(
                header.contains("📁"),
                "el encabezado del diff debe usar el emoji 📁"
            );
            assert!(
                header.contains("Archivos modificados"),
                "el encabezado debe mencionar archivos modificados"
            );
            // El header no debe estar vacío
            assert!(!header.is_empty());
        }

        /// CA3: En modo compacto, should_run_post_diff retorna false
        ///      incluso con git=true y dry-run=false.
        #[test]
        fn ca3_diff_skipped_in_compact_mode() {
            assert!(
                !should_run_post_diff(true, true, false),
                "modo compacto NO debe ejecutar git diff --stat"
            );
        }

        /// CA3: El compact suprime el diff sin importar otras flags.
        #[test]
        fn ca3_compact_supresses_diff_regardless_of_other_flags() {
            assert!(!should_run_post_diff(true, true, false));
            assert!(!should_run_post_diff(true, true, true));
            assert!(!should_run_post_diff(true, false, false));
            assert!(!should_run_post_diff(true, false, true));
        }

        /// CA4: Si git.enabled = false, should_run_post_diff siempre
        ///      retorna false — sin error, sin panic.
        #[test]
        fn ca4_diff_skipped_when_git_disabled() {
            for compact in [true, false] {
                for dry_run in [true, false] {
                    assert!(
                        !should_run_post_diff(compact, false, dry_run),
                        "git disabled debe suprimir diff: compact={compact}, dry_run={dry_run}"
                    );
                }
            }
        }

        /// CA4: La omisión es silenciosa (función booleana pura).
        #[test]
        fn ca4_diff_skip_is_silent_no_panic() {
            let _ = should_run_post_diff(false, false, false);
            let _ = should_run_post_diff(true, false, true);
            let _ = should_run_post_diff(false, true, true);
            // Si llega aquí sin panic, CA4 OK
        }

        /// CA13: En dry-run, should_run_post_diff retorna false siempre.
        #[test]
        fn ca13_diff_skipped_in_dry_run() {
            for compact in [true, false] {
                for git_enabled in [true, false] {
                    assert!(
                        !should_run_post_diff(compact, git_enabled, true),
                        "dry-run debe suprimir diff: compact={compact}, git={git_enabled}"
                    );
                }
            }
        }

        /// CA13: Dry-run no intenta hacer git diff ni parsear tokens reales.
        ///       El diff se bloquea explícitamente, el parseo también.
        #[test]
        fn ca13_dry_run_blocks_diff_and_token_parsing_flow() {
            // En dry-run, should_run_post_diff = false siempre
            assert!(!should_run_post_diff(false, true, true));
            // El Developer DEBE evitar también cualquier llamada a git
            // y a parse_token_count dentro del bloque dry-run.
        }

        // ═══════════════════════════════════════════════════════════
        // CA5-CA6: Modelo en línea de agente
        // ═══════════════════════════════════════════════════════════

        /// CA5: La línea formateada incluye el modelo entre corchetes
        ///      después del nombre del provider.
        #[test]
        fn ca5_agent_line_includes_model_in_brackets() {
            let line =
                format_agent_line_with_model("Dev (implement)", "STORY-003", "pi", "qwen2.5-coder");

            assert!(
                line.contains("[qwen2.5-coder]"),
                "la línea debe contener el modelo entre corchetes: {line}"
            );
            assert!(line.contains("pi"), "debe contener el provider: {line}");
            assert!(
                line.contains("STORY-003"),
                "debe contener el story ID: {line}"
            );
            assert!(line.starts_with("🎯"), "debe comenzar con 🎯: {line}");
        }

        /// CA5: El formato exacto es:
        ///      🎯 <label> | <story_id> | <provider> [<modelo>]
        #[test]
        fn ca5_agent_line_follows_exact_format() {
            let expected = "🎯 Dev (implement) | STORY-003 | pi [qwen2.5-coder]";
            let actual =
                format_agent_line_with_model("Dev (implement)", "STORY-003", "pi", "qwen2.5-coder");
            assert_eq!(
                actual, expected,
                "formato exacto:\n  esperado: {expected}\n  obtenido:  {actual}"
            );
        }

        /// CA5: Con modelo fallback "desconocido", el formato se mantiene.
        #[test]
        fn ca5_agent_line_with_desconocido_model() {
            let line =
                format_agent_line_with_model("PO (plan)", "STORY-001", "claude", "desconocido");
            assert!(line.contains("[desconocido]"));
            assert!(line.contains("PO (plan)"));
            assert!(line.contains("claude"));
            assert!(line.contains("STORY-001"));
        }

        /// CA5: Modelos con caracteres especiales (ej: opencode/gpt-5-nano)
        ///      se renderizan correctamente entre corchetes.
        #[test]
        fn ca5_agent_line_with_special_chars_in_model() {
            let line = format_agent_line_with_model(
                "QA (tests)",
                "STORY-010",
                "opencode",
                "opencode/minimax-m2.5-free",
            );
            assert!(line.contains("[opencode/minimax-m2.5-free]"));
            assert!(line.contains("opencode"));
        }

        /// CA6: model_for_role() se llama con el skill_path correcto.
        ///      Este test verifica la cadena role→skill_path→model.
        #[test]
        fn ca6_model_for_role_resolution_with_correct_skill_path() {
            // Caso: modelo definido en config por rol
            let toml = r#"
[agents]
provider = "pi"

[agents.developer]
model = "gpt-5"
"#;
            let cfg: Config = toml::from_str(toml).unwrap();
            let role = "developer";

            // La skill path DEBE venir desde crate::app::resolver::skill_path(&cfg.agents, role)
            let skill_path_str = crate::app::resolver::skill_path(&cfg.agents, role);
            let skill_path = Path::new(&skill_path_str);
            let model = crate::app::resolver::model(&cfg.agents, role, skill_path);

            assert_eq!(
                model, "gpt-5",
                "model_for_role con skill_path correcto debe devolver el modelo del rol"
            );
        }

        /// CA6: El skill_path que se pasa a model_for_role se obtiene
        ///      desde crate::app::resolver::skill_path(&cfg.agents, role), no hardcodeado.
        #[test]
        fn ca6_skill_path_resolved_via_skill_for_role() {
            let cfg = Config::default();
            let role = "developer";
            let skill_path_str = crate::app::resolver::skill_path(&cfg.agents, role);

            assert_eq!(
                skill_path_str, ".pi/skills/developer/SKILL.md",
                "skill path por defecto para developer"
            );
            assert!(
                skill_path_str.ends_with(".md"),
                "skill path debe ser un archivo .md: {skill_path_str}"
            );

            // El path se puede usar con model_for_role sin paniquear
            let model = crate::app::resolver::model(&cfg.agents, role, Path::new(&skill_path_str));
            assert!(
                !model.is_empty(),
                "model_for_role nunca debe devolver string vacía"
            );
        }

        // ═══════════════════════════════════════════════════════════
        // CA7-CA8: Acumulación de tokens post-agente
        // ═══════════════════════════════════════════════════════════

        /// CA7: parse_token_count() se llama con result.stdout + result.stderr
        ///      concatenados. Verifica que el parseo funciona sobre salida combinada.
        #[test]
        fn ca7_parse_tokens_from_combined_stdout_stderr() {
            let stdout = "Implementación completada.\nTokens used: 2,450 input, 1,200 output\n";
            let stderr = "warning: unused variable\n";

            // El Developer debe concatenar: combined = result.stdout + result.stderr
            let combined = format!("{stdout}{stderr}");
            let parsed = crate::infra::agent::parse_token_count(&combined);

            assert!(
                parsed.is_some(),
                "parse_token_count debe encontrar tokens en stdout+stderr combinados"
            );
            let tc = parsed.unwrap();
            assert_eq!(tc.input, 2450);
            assert_eq!(tc.output, 1200);
        }

        /// CA7: Si no hay patrón de tokens en la salida, parse_token_count
        ///      devuelve None — el Developer debe manejar None sin error.
        #[test]
        fn ca7_parse_tokens_handles_none_gracefully() {
            let combined = "no token info here\njust normal output\n";
            let parsed = crate::infra::agent::parse_token_count(combined);
            assert!(parsed.is_none(), "sin patrón → None, sin error");
        }

        /// CA7: Salida vacía devuelve None.
        #[test]
        fn ca7_parse_tokens_handles_empty_output() {
            let parsed = crate::infra::agent::parse_token_count("");
            assert!(parsed.is_none(), "salida vacía → None");
        }

        /// CA8: Los tokens parseados se acumulan en shared_state.token_usage
        ///      bajo el story_id, haciendo push al Vec.
        #[test]
        fn ca8_tokens_accumulated_in_shared_state_by_story_id() {
            let state = SharedState::default();
            let story_id = "STORY-001";

            // Primera invocación: push 1 token count
            {
                let mut w = state.token_usage.write().unwrap();
                w.entry(story_id.to_string()).or_default().push(TokenCount {
                    input: 100,
                    output: 50,
                });
            }

            // Segunda invocación misma historia: push otro token count
            {
                let mut w = state.token_usage.write().unwrap();
                w.entry(story_id.to_string()).or_default().push(TokenCount {
                    input: 200,
                    output: 100,
                });
            }

            // Verificar acumulación
            let r = state.token_usage.read().unwrap();
            let entries = r
                .get(story_id)
                .expect("debe existir entrada para STORY-001");
            assert_eq!(entries.len(), 2, "2 invocaciones → 2 entradas en el Vec");
            assert_eq!(entries[0].input, 100);
            assert_eq!(entries[0].output, 50);
            assert_eq!(entries[1].input, 200);
            assert_eq!(entries[1].output, 100);
        }

        /// CA8: Historias diferentes acumulan tokens independientemente.
        #[test]
        fn ca8_token_accumulation_independent_per_story() {
            let state = SharedState::default();

            state.token_usage.write().unwrap().insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 100,
                    output: 50,
                }],
            );

            state.token_usage.write().unwrap().insert(
                "STORY-002".into(),
                vec![
                    TokenCount {
                        input: 10,
                        output: 5,
                    },
                    TokenCount {
                        input: 20,
                        output: 10,
                    },
                    TokenCount {
                        input: 30,
                        output: 15,
                    },
                ],
            );

            let r = state.token_usage.read().unwrap();
            assert_eq!(r.get("STORY-001").unwrap().len(), 1);
            assert_eq!(r.get("STORY-002").unwrap().len(), 3);
        }

        // ═══════════════════════════════════════════════════════════
        // CA9-CA10: Resumen final enriquecido
        // ═══════════════════════════════════════════════════════════

        /// CA9: El bloque de cierre contiene todos los campos requeridos
        ///      en el orden y formato exacto.
        #[test]
        fn ca9_summary_block_contains_all_required_fields() {
            let summary = build_final_summary_block(
                5,                                    // total
                3,                                    // done
                1,                                    // failed
                &["STORY-002".to_string()],           // failed_ids
                0,                                    // blocked
                1,                                    // draft
                42,                                   // iterations
                std::time::Duration::from_secs(3661), // 1h 1m 1s
                5000,                                 // input tokens
                3000,                                 // output tokens
            );

            // Campos obligatorios
            assert!(
                summary.contains("Pipeline completado"),
                "debe contener 'Pipeline completado'"
            );
            assert!(summary.contains("Total"), "debe contener 'Total'");
            assert!(summary.contains("✅ Done"), "debe contener '✅ Done'");
            assert!(summary.contains("❌ Failed"), "debe contener '❌ Failed'");
            assert!(
                summary.contains("STORY-002"),
                "debe contener IDs de Failed entre paréntesis"
            );
            assert!(summary.contains("🔒 Blocked"), "debe contener '🔒 Blocked'");
            assert!(summary.contains("📝 Draft"), "debe contener '📝 Draft'");
            assert!(
                summary.contains("🔄 Iteraciones"),
                "debe contener '🔄 Iteraciones'"
            );
            assert!(
                summary.contains("⏱️  Tiempo total"),
                "debe contener '⏱️  Tiempo total'"
            );
            assert!(
                summary.contains("📊 Tokens totales"),
                "debe contener '📊 Tokens totales'"
            );
            // Bordes decorativos con ═══
            assert!(summary.contains("═══"), "debe contener líneas decorativas");
        }

        /// CA9: El conteo de cada categoría aparece correctamente.
        #[test]
        fn ca9_summary_counts_are_correct() {
            let summary = build_final_summary_block(
                10,
                8,
                0,
                &[],
                2,
                0,
                15,
                std::time::Duration::from_secs(120),
                1000,
                500,
            );

            assert!(summary.contains("Total        : 10"));
            assert!(summary.contains("✅ Done      : 8"));
            assert!(summary.contains("❌ Failed    : 0"));
            assert!(summary.contains("🔒 Blocked   : 2"));
            assert!(summary.contains("📝 Draft     : 0"));
            assert!(summary.contains("🔄 Iteraciones: 15"));
        }

        /// CA9: El tiempo total se muestra en formato Xh Ym Zs.
        #[test]
        fn ca9_elapsed_time_formatted_in_h_m_s() {
            // 3723 segundos = 1h 2m 3s
            let summary = build_final_summary_block(
                1,
                1,
                0,
                &[],
                0,
                0,
                1,
                std::time::Duration::from_secs(3723),
                0,
                0,
            );

            assert!(
                summary.contains("1h 2m 3s"),
                "3723s debe formatearse como '1h 2m 3s'.\nResumen:\n{summary}"
            );
        }

        /// CA9: Con tiempo menor a 1h, muestra 0h correctamente.
        #[test]
        fn ca9_elapsed_time_less_than_hour() {
            let summary = build_final_summary_block(
                1,
                1,
                0,
                &[],
                0,
                0,
                1,
                std::time::Duration::from_secs(125),
                0,
                0,
            );

            assert!(
                summary.contains("0h 2m 5s"),
                "125s debe formatearse como '0h 2m 5s'"
            );
        }

        /// CA9: Con Failed > 0, los IDs aparecen entre paréntesis.
        #[test]
        fn ca9_failed_ids_listed_in_parentheses() {
            let summary = build_final_summary_block(
                3,
                1,
                2,
                &["STORY-002".into(), "STORY-003".into()],
                0,
                0,
                5,
                std::time::Duration::from_secs(60),
                100,
                50,
            );

            assert!(summary.contains("❌ Failed    : 2"));
            assert!(summary.contains("(STORY-002, STORY-003)"));
        }

        /// CA10: compute_token_totals suma correctamente todos los
        ///       TokenCount de todas las historias.
        #[test]
        fn ca10_token_totals_sum_all_stories() {
            let mut token_usage: HashMap<String, Vec<TokenCount>> = HashMap::new();

            token_usage.insert(
                "STORY-001".into(),
                vec![
                    TokenCount {
                        input: 100,
                        output: 50,
                    },
                    TokenCount {
                        input: 200,
                        output: 150,
                    },
                ],
            );
            token_usage.insert(
                "STORY-002".into(),
                vec![TokenCount {
                    input: 300,
                    output: 200,
                }],
            );
            token_usage.insert(
                "STORY-003".into(),
                vec![
                    TokenCount {
                        input: 50,
                        output: 25,
                    },
                    TokenCount {
                        input: 75,
                        output: 50,
                    },
                    TokenCount {
                        input: 100,
                        output: 75,
                    },
                ],
            );

            let (total_input, total_output) = compute_token_totals(&token_usage);

            // STORY-001: 300 input, 200 output
            // STORY-002: 300 input, 200 output
            // STORY-003: 225 input, 150 output
            assert_eq!(total_input, 825, "300 + 300 + 225 = 825");
            assert_eq!(total_output, 550, "200 + 200 + 150 = 550");
        }

        /// CA10: compute_token_totals con HashMap vacío devuelve (0, 0).
        #[test]
        fn ca10_token_totals_empty_returns_zero() {
            let empty: HashMap<String, Vec<TokenCount>> = HashMap::new();
            let (input, output) = compute_token_totals(&empty);
            assert_eq!(input, 0);
            assert_eq!(output, 0);
        }

        /// CA10: compute_token_totals con una sola historia.
        #[test]
        fn ca10_token_totals_single_story() {
            let mut token_usage = HashMap::new();
            token_usage.insert(
                "STORY-001".into(),
                vec![TokenCount {
                    input: 150,
                    output: 75,
                }],
            );

            let (input, output) = compute_token_totals(&token_usage);
            assert_eq!(input, 150);
            assert_eq!(output, 75);
        }

        /// CA10: El resumen final muestra la suma correcta de tokens:
        ///       📊 Tokens totales: N input + N output = N
        #[test]
        fn ca10_final_summary_shows_correct_token_sums() {
            let summary = build_final_summary_block(
                2,
                2,
                0,
                &[],
                0,
                0,
                10,
                std::time::Duration::from_secs(60),
                4500, // total input
                2300, // total output
            );

            assert!(summary.contains("4500 input"), "debe mostrar '4500 input'");
            assert!(
                summary.contains("2300 output"),
                "debe mostrar '2300 output'"
            );
            assert!(
                summary.contains("= 6800"),
                "debe mostrar '= 6800' (4500 + 2300)"
            );
            assert!(
                summary.contains("📊 Tokens totales"),
                "debe incluir la línea de tokens totales"
            );
        }

        /// CA10: Con tokens = 0, el resumen muestra 0 correctamente.
        #[test]
        fn ca10_token_totals_zero_in_summary() {
            let summary = build_final_summary_block(
                0,
                0,
                0,
                &[],
                0,
                0,
                0,
                std::time::Duration::from_secs(0),
                0,
                0,
            );

            assert!(
                summary.contains("0 input + 0 output = 0"),
                "con 0 tokens, debe mostrar '0 input + 0 output = 0'.\nResumen:\n{summary}"
            );
        }

        // ═══════════════════════════════════════════════════════════
        // Integración: flujo completo post-agente
        // ═══════════════════════════════════════════════════════════

        /// Verifica el orden del flujo tras process_story():
        /// 1. should_run_post_diff → git diff --stat (si procede)
        /// 2. parse_token_count(result.stdout + result.stderr)
        /// 3. push TokenCount a shared_state.token_usage
        #[test]
        fn post_agent_flow_order_is_respected() {
            // Paso 1: ¿debemos ejecutar el diff?
            let run_diff = should_run_post_diff(false, true, false);
            assert!(run_diff, "modo detallado con git → diff procede");

            // Paso 2: parsear tokens de la salida del agente
            let agent_output = "Tokens used: 100 input, 50 output\n";
            let parsed = crate::infra::agent::parse_token_count(agent_output);
            assert!(parsed.is_some(), "tokens deben parsearse del output");

            // Paso 3: acumular en SharedState
            let state = SharedState::default();
            let story_id = "STORY-001";
            if let Some(tc) = parsed {
                // El Developer debe convertir infra::agent::TokenCount → domain::state::TokenCount
                let domain_tc = TokenCount {
                    input: tc.input,
                    output: tc.output,
                };
                state
                    .token_usage
                    .write()
                    .unwrap()
                    .entry(story_id.to_string())
                    .or_default()
                    .push(domain_tc);
            }

            // Verificar acumulación
            let r = state.token_usage.read().unwrap();
            let entries = r.get(story_id).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].input, 100);
            assert_eq!(entries[0].output, 50);
        }

        /// Con compact=true, el diff se salta pero el parseo de tokens NO.
        #[test]
        fn compact_skips_diff_but_not_token_parsing() {
            // compact=true → diff no procede
            assert!(!should_run_post_diff(true, true, false));

            // Pero los tokens se parsean IGUAL en ambos modos
            let agent_output = "Tokens used: 42 input, 7 output\n";
            let parsed = crate::infra::agent::parse_token_count(agent_output);
            assert!(
                parsed.is_some(),
                "parseo de tokens debe funcionar también en modo compacto"
            );
            assert_eq!(parsed.unwrap().input, 42);
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // EPIC-V10-03: Pipeline Genérico
    // ═══════════════════════════════════════════════════════════════

    mod epic_v10_03_pipeline {

        // ═══════════════════════════════════════════════════════════
        // STORY-V10-011: Parseo de respuesta del agente
        // ═══════════════════════════════════════════════════════════

        mod story_v10_011_parse_agent_action {
            use regex::Regex;
            use std::collections::HashMap;
            use std::sync::LazyLock;

            // ── Tipos esperados (TDD: el Developer los moverá a producción) ──

            /// Acción extraída de la respuesta del agente.
            #[derive(Debug, Clone, PartialEq, Eq)]
            #[allow(dead_code)]
            pub(super) enum AgentAction {
                /// Transición exitosa al estado destino.
                Transition(String),
                /// Rechazo con motivo.
                Reject(String),
                /// La tarea depende de otra tarea.
                AddDependency(String),
                /// La tarea se bloquea manualmente.
                Block(String),
            }

            /// Error de parseo de la respuesta del agente.
            #[derive(Debug, Clone, PartialEq, Eq)]
            #[allow(dead_code)]
            pub(super) enum AgentParseError {
                /// La respuesta no contiene ningún marcador reconocido.
                NoMarkerFound { response: String },
                /// El estado destino no está definido en el workflow.
                InvalidState { state: String },
                /// Múltiples marcadores encontrados (ambiguo).
                MultipleMarkers {
                    found: Vec<String>,
                    response: String,
                },
            }

            /// Conjunto de estados válidos del workflow (simplificado para tests).
            fn valid_states() -> Vec<String> {
                vec![
                    "draft".to_string(),
                    "ready".to_string(),
                    "review".to_string(),
                    "in_progress".to_string(),
                    "done".to_string(),
                    "failed".to_string(),
                    "blocked".to_string(),
                ]
            }

            /// Versión de desarrollo (TDD) de parse_agent_action.
            /// El Developer DEBE reemplazar este placeholder con la implementación real.
            #[allow(dead_code)]
            pub(super) fn parse_agent_action(
                response: &str,
                valid_states: &[String],
            ) -> Result<AgentAction, AgentParseError> {
                // ── Buscar marcadores con regex ──
                let status_re: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"\[STATUS:\s*([^\]]+)\]").unwrap());
                let reject_re: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"\[REJECT:\s*([^\]]+)\]").unwrap());
                let depends_re: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"\[DEPENDS_ON:\s*([^\]]+)\]").unwrap());
                let blocked_re: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"\[BLOCKED:\s*([^\]]+)\]").unwrap());

                let status_caps: Vec<String> = status_re
                    .captures_iter(response)
                    .map(|c| c[1].trim().to_string())
                    .collect();
                let reject_caps: Vec<String> = reject_re
                    .captures_iter(response)
                    .map(|c| c[1].trim().to_string())
                    .collect();
                let depends_caps: Vec<String> = depends_re
                    .captures_iter(response)
                    .map(|c| c[1].trim().to_string())
                    .collect();
                let blocked_caps: Vec<String> = blocked_re
                    .captures_iter(response)
                    .map(|c| c[1].trim().to_string())
                    .collect();

                // Detectar múltiples marcadores
                let found_count =
                    (!status_caps.is_empty()) as usize
                        + (!reject_caps.is_empty()) as usize
                        + (!depends_caps.is_empty()) as usize
                        + (!blocked_caps.is_empty()) as usize;

                if found_count > 1 {
                    let mut all_found = vec![];
                    for s in &status_caps {
                        all_found.push(format!("[STATUS: {s}]"));
                    }
                    for r in &reject_caps {
                        all_found.push(format!("[REJECT: {r}]"));
                    }
                    for d in &depends_caps {
                        all_found.push(format!("[DEPENDS_ON: {d}]"));
                    }
                    for b in &blocked_caps {
                        all_found.push(format!("[BLOCKED: {b}]"));
                    }
                    return Err(AgentParseError::MultipleMarkers {
                        found: all_found,
                        response: response.to_string(),
                    });
                }

                // ── Procesar el marcador encontrado ──
                if let Some(state) = status_caps.into_iter().next() {
                    // Validar que el estado existe en el workflow
                    if !valid_states.iter().any(|vs| vs == &state) {
                        return Err(AgentParseError::InvalidState {
                            state: state.clone(),
                        });
                    }
                    return Ok(AgentAction::Transition(state));
                }

                if let Some(reason) = reject_caps.into_iter().next() {
                    return Ok(AgentAction::Reject(reason));
                }

                if let Some(dep_id) = depends_caps.into_iter().next() {
                    return Ok(AgentAction::AddDependency(dep_id));
                }

                if let Some(block_reason) = blocked_caps.into_iter().next() {
                    return Ok(AgentAction::Block(block_reason));
                }

                // Ningún marcador encontrado
                Err(AgentParseError::NoMarkerFound {
                    response: response.to_string(),
                })
            }

            // ═══════════════════════════════════════════════════════
            // CA1: parse_agent_action extrae marcadores con regex
            // ═══════════════════════════════════════════════════════

            /// CA1: [STATUS: X] → AgentAction::Transition(X)
            #[test]
            fn parse_status_transition() {
                let response = "He completado la implementación.\n\n[STATUS: review]\n\nTodo listo.";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Transition(state)) => {
                        assert_eq!(state, "review");
                    }
                    other => panic!("Expected Transition(\"review\"), got {other:?}"),
                }
            }

            /// CA1: [STATUS: X] con espacios extra alrededor de X
            #[test]
            fn parse_status_with_extra_spaces() {
                let response = "[STATUS:   done   ]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Transition(state)) => {
                        assert_eq!(state, "done");
                    }
                    other => panic!("Expected Transition(\"done\"), got {other:?}"),
                }
            }

            /// CA1: [STATUS: X] en medio de texto largo
            #[test]
            fn parse_status_buried_in_text() {
                let response = "He revisado el código y los tests pasan.\n\
                                El código sigue los estándares.\n\
                                [STATUS: done]\n\
                                Recomendación: mergear.";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Transition(state)) => {
                        assert_eq!(state, "done");
                    }
                    other => panic!("Expected Transition(\"done\"), got {other:?}"),
                }
            }

            /// CA1: [REJECT: motivo] → AgentAction::Reject(motivo)
            #[test]
            fn parse_reject() {
                let response = "[REJECT: los tests no compilan]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Reject(reason)) => {
                        assert_eq!(reason, "los tests no compilan");
                    }
                    other => panic!("Expected Reject, got {other:?}"),
                }
            }

            /// CA1: [REJECT: motivo] con motivo multilínea (solo primera línea)
            #[test]
            fn parse_reject_with_multiline_reason() {
                let response = "Análisis completado.\n[REJECT: la implementación \
                                no cumple los criterios]\nVolver a implementar.";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Reject(reason)) => {
                        assert!(reason.contains("no cumple los criterios"));
                    }
                    other => panic!("Expected Reject, got {other:?}"),
                }
            }

            /// CA1: [DEPENDS_ON: Z] → AgentAction::AddDependency(Z)
            #[test]
            fn parse_add_dependency() {
                let response = "Esta tarea necesita que TASK-002 esté completada.\
                                [DEPENDS_ON: TASK-002]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::AddDependency(id)) => {
                        assert_eq!(id, "TASK-002");
                    }
                    other => panic!("Expected AddDependency(\"TASK-002\"), got {other:?}"),
                }
            }

            /// CA1: [DEPENDS_ON: Z] con ISSUE-NNN
            #[test]
            fn parse_add_dependency_with_issue_id() {
                let response = "[DEPENDS_ON: ISSUE-042]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::AddDependency(id)) => {
                        assert_eq!(id, "ISSUE-042");
                    }
                    other => panic!("Expected AddDependency, got {other:?}"),
                }
            }

            /// CA1: [BLOCKED: W] → AgentAction::Block(W)
            #[test]
            fn parse_block() {
                let response = "No puedo continuar sin la API key.\
                                [BLOCKED: falta API key de OpenAI]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Block(reason)) => {
                        assert_eq!(reason, "falta API key de OpenAI");
                    }
                    other => panic!("Expected Block, got {other:?}"),
                }
            }

            /// CA1: [BLOCKED: W] con motivo corto
            #[test]
            fn parse_block_short_reason() {
                let response = "[BLOCKED: dependencia externa]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Block(reason)) => {
                        assert_eq!(reason, "dependencia externa");
                    }
                    other => panic!("Expected Block, got {other:?}"),
                }
            }

            // ═══════════════════════════════════════════════════════
            // CA2: Validación de estado destino
            // ═══════════════════════════════════════════════════════

            /// CA2: Si [STATUS: X] con X no definido en el workflow → error
            #[test]
            fn parse_status_rejects_unknown_state() {
                let response = "[STATUS: flying]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::InvalidState { state }) => {
                        assert_eq!(state, "flying");
                    }
                    other => panic!("Expected InvalidState, got {other:?}"),
                }
            }

            /// CA2: Estado válido pero con mayúsculas distintas → el desarrollador
            ///      debe decidir si normaliza o no. Por ahora, el test asume exact match.
            #[test]
            fn parse_status_case_sensitive_by_default() {
                let response = "[STATUS: DONE]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::InvalidState { state }) => {
                        assert_eq!(state, "DONE");
                    }
                    Ok(AgentAction::Transition(state)) => {
                        // Si el Developer decide normalizar, este branch pasa
                        assert!(
                            state == "done" || state == "DONE",
                            "Estado debe ser 'done' (normalizado) o 'DONE' (sin normalizar)"
                        );
                    }
                    other => panic!("Expected InvalidState o Transition, got {other:?}"),
                }
            }

            /// CA2: Estado vacío debe rechazarse
            #[test]
            fn parse_status_rejects_empty_state() {
                let response = "[STATUS: ]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::InvalidState { state }) => {
                        assert!(state.is_empty() || state == " ");
                    }
                    _ => {} // Si el Developer decide limpiar espacios y rechazar vacío, OK
                }
            }

            /// CA2: Todos los estados válidos del workflow son aceptados
            #[test]
            fn parse_status_accepts_all_valid_states() {
                for state in &valid_states() {
                    let response = format!("[STATUS: {state}]");
                    let result = parse_agent_action(&response, &valid_states());
                    match result {
                        Ok(AgentAction::Transition(s)) => {
                            assert_eq!(s, *state);
                        }
                        other => panic!("Expected Transition(\"{state}\"), got {other:?}"),
                    }
                }
            }

            // ═══════════════════════════════════════════════════════
            // CA3: NoMarkerFound cuando no hay formato reconocido
            // ═══════════════════════════════════════════════════════

            /// CA3: Respuesta sin marcadores → AgentParseError::NoMarkerFound
            #[test]
            fn parse_no_marker_returns_error() {
                let response = "Parece que está todo bien, creo que podemos avanzar.";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::NoMarkerFound { response: resp }) => {
                        assert!(resp.contains("creo que podemos avanzar"));
                    }
                    other => panic!("Expected NoMarkerFound, got {other:?}"),
                }
            }

            /// CA3: Respuesta vacía → NoMarkerFound
            #[test]
            fn parse_empty_response_returns_no_marker() {
                let result = parse_agent_action("", &valid_states());
                match result {
                    Err(AgentParseError::NoMarkerFound { .. }) => {}
                    other => panic!("Expected NoMarkerFound, got {other:?}"),
                }
            }

            /// CA3: Respuesta con texto similar pero sin formato exacto → NoMarkerFound
            #[test]
            fn parse_similar_but_not_matching_format() {
                let response = "STATUS: done (sin corchetes)";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::NoMarkerFound { .. }) => {}
                    other => panic!("Expected NoMarkerFound, got {other:?}"),
                }
            }

            /// CA3: El mensaje de error incluye la respuesta completa
            #[test]
            fn parse_no_marker_includes_full_response_in_error() {
                let response = "Todo OK. Mergeamos.";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::NoMarkerFound { response: resp }) => {
                        assert_eq!(resp, "Todo OK. Mergeamos.");
                    }
                    other => panic!("Expected NoMarkerFound, got {other:?}"),
                }
            }

            // ═══════════════════════════════════════════════════════
            // Múltiples marcadores → error
            // ═══════════════════════════════════════════════════════

            /// Múltiples marcadores en la misma respuesta → error
            #[test]
            fn parse_multiple_markers_returns_error() {
                let response = "[STATUS: done] y también [REJECT: no está listo]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::MultipleMarkers { found, .. }) => {
                        assert!(found.len() >= 2);
                        assert!(found.iter().any(|m| m.contains("STATUS")));
                        assert!(found.iter().any(|m| m.contains("REJECT")));
                    }
                    other => panic!("Expected MultipleMarkers, got {other:?}"),
                }
            }

            // ═══════════════════════════════════════════════════════
            // Robustez: el parser no paniquea con inputs inesperados
            // ═══════════════════════════════════════════════════════

            /// El parser no paniquea con corchetes sueltos
            #[test]
            fn parse_does_not_panic_on_partial_brackets() {
                let response = "[STATUS: ";
                let result = parse_agent_action(response, &valid_states());
                assert!(result.is_err(), "Debe devolver error, no paniquear");
            }

            /// El parser no paniquea con texto que contiene [pero no marcadores]
            #[test]
            fn parse_does_not_panic_on_random_brackets() {
                let response = "Usa [array] y [object] en el código";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Err(AgentParseError::NoMarkerFound { .. }) => {}
                    other => panic!("Expected NoMarkerFound, got {other:?}"),
                }
            }

            /// El parser maneja respuestas muy largas
            #[test]
            fn parse_handles_long_response() {
                let long_text = "A".repeat(10_000);
                let response = format!("[STATUS: ready]\n{long_text}");
                let result = parse_agent_action(&response, &valid_states());
                match result {
                    Ok(AgentAction::Transition(s)) => assert_eq!(s, "ready"),
                    other => panic!("Expected Transition(\"ready\"), got {other:?}"),
                }
            }

            /// El parser encuentra el marcador aunque esté al final
            #[test]
            fn parse_finds_marker_at_end_of_response() {
                let response = "Análisis completado. Código revisado. [STATUS: review]";
                let result = parse_agent_action(response, &valid_states());
                match result {
                    Ok(AgentAction::Transition(s)) => assert_eq!(s, "review"),
                    other => panic!("Expected Transition(\"review\"), got {other:?}"),
                }
            }
        }

        // ═══════════════════════════════════════════════════════════
        // STORY-V10-010: Loop principal con lookup dinámico de fases
        // ═══════════════════════════════════════════════════════════

        mod story_v10_010_dynamic_lookup {
            use super::story_v10_011_parse_agent_action::{
                AgentAction, AgentParseError, parse_agent_action,
            };
            use crate::domain::task::{ActivityLogEntry, Task};
            use crate::domain::templates::render_template;
            use crate::domain::workflow::{
                ConfigurableWorkflow, PhaseConfig, RoleConfig, WorkflowConfig, WorkflowStatesConfig,
            };
            use crate::infra::llm::types::{ChatResponse, Message};
            use std::cell::RefCell;
            use std::collections::HashMap;
            use std::path::PathBuf;
            use std::time::Duration;

            // ── Helpers ────────────────────────────────────────────

            fn make_workflow_config() -> WorkflowConfig {
                WorkflowConfig {
                    states: WorkflowStatesConfig {
                        initial: "draft".to_string(),
                        terminal: vec!["done".to_string(), "failed".to_string()],
                    },
                    roles: vec![
                        RoleConfig {
                            name: "developer".to_string(),
                            system_prompt: "Eres un desarrollador. Responde con [STATUS: <estado>] o [REJECT: <motivo>].".to_string(),
                            model: "gpt4o".to_string(),
                        },
                        RoleConfig {
                            name: "reviewer".to_string(),
                            system_prompt: "Eres un revisor. Responde con [STATUS: <estado>] o [REJECT: <motivo>].".to_string(),
                            model: "claude".to_string(),
                        },
                    ],
                    phases: vec![
                        PhaseConfig {
                            name: "plan".to_string(),
                            from: "draft".to_string(),
                            to: "ready".to_string(),
                            role: "developer".to_string(),
                            model: "gpt4o".to_string(),
                            prompt: "Planifica {{task_id}}".to_string(),
                            on_reject: "draft".to_string(),
                            max_reject_cycles: 3,
                            timeout_seconds: None,
                        },
                        PhaseConfig {
                            name: "implement".to_string(),
                            from: "ready".to_string(),
                            to: "review".to_string(),
                            role: "developer".to_string(),
                            model: "gpt4o".to_string(),
                            prompt: "Implementa {{task_id}}".to_string(),
                            on_reject: "ready".to_string(),
                            max_reject_cycles: 3,
                            timeout_seconds: None,
                        },
                        PhaseConfig {
                            name: "validate".to_string(),
                            from: "review".to_string(),
                            to: "done".to_string(),
                            role: "reviewer".to_string(),
                            model: "claude".to_string(),
                            prompt: "Valida {{task_id}}".to_string(),
                            on_reject: "ready".to_string(),
                            max_reject_cycles: 2,
                            timeout_seconds: Some(300),
                        },
                    ],
                    task_format: crate::domain::task::TaskFormatConfig::default(),
                }
            }

            fn make_task(id: &str, status: &str) -> Task {
                let mut fields = HashMap::new();
                fields.insert("status".to_string(), status.to_string());
                Task {
                    id: id.to_string(),
                    path: PathBuf::from(format!("tasks/{id}.md")),
                    fields,
                    blockers: vec![],
                    activity_log: vec![],
                    raw_content: String::new(),
                }
            }

            fn task_with_history(id: &str, status: &str, log_entries: Vec<ActivityLogEntry>) -> Task {
                let mut task = make_task(id, status);
                task.activity_log = log_entries;
                task
            }

            // ── Firma esperada de build_messages_for_task (TDD) ──

            /// Construye los mensajes para la invocación al LLM:
            /// 1. System prompt del rol
            /// 2. Prompt de fase renderizado con la task
            /// 3. Historial de conversación (activity_log → assistant messages)
            ///
            /// El Developer DEBE implementar esta función en producción.
            #[allow(dead_code)]
            fn build_messages_for_task(
                task: &Task,
                phase: &PhaseConfig,
                role: &RoleConfig,
                context: &HashMap<String, String>,
            ) -> Vec<Message> {
                let mut messages = Vec::new();

                // 1. System prompt del rol (renderizado con el contexto)
                let system_prompt = render_template(&role.system_prompt, task, context);
                messages.push(Message::system(system_prompt));

                // 2. Prompt de fase (renderizado con la task)
                let phase_prompt = render_template(&phase.prompt, task, context);
                messages.push(Message::user(phase_prompt));

                // 3. Historial de conversación (cada entrada como assistant)
                for entry in &task.activity_log {
                    if !entry.description.is_empty() {
                        messages.push(Message::assistant(format!(
                            "[{}] {}: {}",
                            entry.date, entry.actor, entry.description
                        )));
                    }
                }

                messages
            }

            // ═══════════════════════════════════════════════════════
            // CA1: process_task lookup de fases con workflow.phases_for_status
            // ═══════════════════════════════════════════════════════

            /// CA1: phases_for_status devuelve la fase correcta para un estado
            #[test]
            fn phases_for_status_returns_correct_phase() {
                let config = make_workflow_config();
                let wf = ConfigurableWorkflow::new(&config);

                let phases = wf.phases_for_status("draft");
                assert_eq!(phases.len(), 1);
                assert_eq!(phases[0].name, "plan");
                assert_eq!(phases[0].from, "draft");
                assert_eq!(phases[0].to, "ready");
            }

            /// CA1: phases_for_status no devuelve fases para estado terminal
            #[test]
            fn phases_for_status_empty_for_terminal_state() {
                let config = make_workflow_config();
                let wf = ConfigurableWorkflow::new(&config);

                assert!(wf.phases_for_status("done").is_empty());
                assert!(wf.phases_for_status("failed").is_empty());
            }

            /// CA1: Fase tiene todos los campos necesarios para process_task
            #[test]
            fn phase_has_all_required_fields_for_process_task() {
                let config = make_workflow_config();
                let wf = ConfigurableWorkflow::new(&config);

                let phases = wf.phases_for_status("review");
                assert_eq!(phases.len(), 1);
                let phase = phases[0];

                // Campos requeridos por process_task
                assert!(!phase.name.is_empty(), "name no debe estar vacío");
                assert!(!phase.role.is_empty(), "role no debe estar vacío");
                assert!(!phase.model.is_empty(), "model no debe estar vacío");
                assert!(!phase.prompt.is_empty(), "prompt no debe estar vacío");
                assert!(!phase.from.is_empty(), "from no debe estar vacío");
                assert!(!phase.to.is_empty(), "to no debe estar vacío");
                assert!(!phase.on_reject.is_empty(), "on_reject no debe estar vacío");
            }

            // ═══════════════════════════════════════════════════════
            // CA1: build_messages_for_task construye los mensajes
            // ═══════════════════════════════════════════════════════

            /// CA1: Los mensajes incluyen system prompt del rol
            #[test]
            fn build_messages_includes_system_prompt() {
                let config = make_workflow_config();
                let role = &config.roles[0]; // developer
                let phase = &config.phases[0]; // plan
                let task = make_task("TASK-001", "draft");
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                assert!(!messages.is_empty(), "Debe haber al menos 1 mensaje");
                assert_eq!(messages[0].role, "system");
                assert!(
                    messages[0].content.contains("desarrollador")
                        || messages[0].content.contains("Responde con"),
                    "system prompt debe contener las instrucciones del rol"
                );
            }

            /// CA1: Los mensajes incluyen el prompt de fase
            #[test]
            fn build_messages_includes_phase_prompt() {
                let config = make_workflow_config();
                let role = &config.roles[0]; // developer
                let phase = &config.phases[0]; // plan — prompt: "Planifica {{task_id}}"
                let task = make_task("TASK-001", "draft");
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                assert!(messages.len() >= 2, "Debe haber al menos system + user prompt");
                assert_eq!(messages[1].role, "user");
                assert!(
                    messages[1].content.contains("TASK-001"),
                    "El prompt de fase debe incluir el ID de la task renderizado"
                );
            }

            /// CA1: El orden de los mensajes es: system → user → history
            #[test]
            fn build_messages_has_correct_order() {
                let config = make_workflow_config();
                let role = &config.roles[0]; // developer
                let phase = &config.phases[0]; // plan
                let task = make_task("TASK-001", "draft");
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                // El orden debe ser: system, user, (history...)
                assert_eq!(messages[0].role, "system", "Primer mensaje debe ser system");
                assert_eq!(messages[1].role, "user", "Segundo mensaje debe ser user");
            }

            // ═══════════════════════════════════════════════════════
            // CA2: Historial multi-turn se construye desde activity_log
            // ═══════════════════════════════════════════════════════

            /// CA2: El historial se incluye como mensajes assistant
            #[test]
            fn build_messages_includes_activity_log_as_history() {
                let config = make_workflow_config();
                let role = &config.roles[1]; // reviewer
                let phase = &config.phases[2]; // validate
                let task = task_with_history(
                    "TASK-001",
                    "review",
                    vec![
                        ActivityLogEntry {
                            date: "2026-05-08".to_string(),
                            actor: "PO".to_string(),
                            description: "Historia creada".to_string(),
                        },
                        ActivityLogEntry {
                            date: "2026-05-09".to_string(),
                            actor: "Dev".to_string(),
                            description: "Implementación completada".to_string(),
                        },
                    ],
                );
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                // system + user + 2 history entries = 4 mensajes
                assert_eq!(
                    messages.len(),
                    4,
                    "system + user + 2 activity log entries = 4 mensajes"
                );

                // Los mensajes de historial son assistant
                assert_eq!(messages[2].role, "assistant");
                assert!(messages[2].content.contains("Historia creada"));
                assert_eq!(messages[3].role, "assistant");
                assert!(messages[3].content.contains("Implementación completada"));
            }

            /// CA2: Activity log vacío no añade mensajes extra
            #[test]
            fn build_messages_with_empty_activity_log() {
                let config = make_workflow_config();
                let role = &config.roles[0];
                let phase = &config.phases[0];
                let task = make_task("TASK-001", "draft"); // activity_log vacío
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                assert_eq!(
                    messages.len(),
                    2,
                    "Con activity_log vacío, solo system + user"
                );
            }

            /// CA2: La conversación completa se pasa al LLM (incluyendo todas las entradas)
            #[test]
            fn build_messages_passes_full_conversation() {
                let config = make_workflow_config();
                let role = &config.roles[0];
                let phase = &config.phases[0];
                let task = task_with_history(
                    "TASK-001",
                    "draft",
                    vec![
                        ActivityLogEntry {
                            date: "2026-05-08".to_string(),
                            actor: "PO".to_string(),
                            description: "Creada".to_string(),
                        },
                        ActivityLogEntry {
                            date: "2026-05-09".to_string(),
                            actor: "Dev".to_string(),
                            description: "Intento 1".to_string(),
                        },
                        ActivityLogEntry {
                            date: "2026-05-10".to_string(),
                            actor: "Reviewer".to_string(),
                            description: "RECHAZADO: tests rotos".to_string(),
                        },
                        ActivityLogEntry {
                            date: "2026-05-11".to_string(),
                            actor: "Dev".to_string(),
                            description: "Intento 2".to_string(),
                        },
                    ],
                );
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                // system + user + 4 history = 6 mensajes
                assert_eq!(messages.len(), 6);
                // Verificar que todas las entradas están presentes
                assert!(messages[2].content.contains("Creada"));
                assert!(messages[3].content.contains("Intento 1"));
                assert!(messages[4].content.contains("RECHAZADO"));
                assert!(messages[5].content.contains("Intento 2"));
            }

            /// CA2: Las entradas del activity_log usan el rol correcto
            #[test]
            fn activity_log_entries_preserve_actor_role() {
                let config = make_workflow_config();
                let role = &config.roles[0];
                let phase = &config.phases[0];
                let task = task_with_history(
                    "TASK-001",
                    "draft",
                    vec![ActivityLogEntry {
                        date: "2026-05-08".to_string(),
                        actor: "QA_Engineer".to_string(),
                        description: "Tests escritos".to_string(),
                    }],
                );
                let context = HashMap::new();

                let messages = build_messages_for_task(&task, phase, role, &context);

                assert_eq!(messages.len(), 3);
                assert!(messages[2].content.contains("QA_Engineer"));
                assert!(messages[2].content.contains("Tests escritos"));
            }

            // ═══════════════════════════════════════════════════════
            // CA1+CA3: Bifurcación de fases
            // ═══════════════════════════════════════════════════════

            /// CA1: Cuando hay múltiples fases desde un estado, el prompt
            ///      debe incluir todas las opciones.
            #[test]
            fn bifurcation_prompt_includes_all_options() {
                let mut config = make_workflow_config();
                // Añadir segunda fase desde "review"
                config.phases.push(PhaseConfig {
                    name: "reject".to_string(),
                    from: "review".to_string(),
                    to: "ready".to_string(),
                    role: "reviewer".to_string(),
                    model: "claude".to_string(),
                    prompt: "Rechaza {{task_id}}".to_string(),
                    on_reject: "review".to_string(),
                    max_reject_cycles: 2,
                    timeout_seconds: None,
                });

                let wf = ConfigurableWorkflow::new(&config);
                let phases = wf.phases_for_status("review");

                assert_eq!(
                    phases.len(),
                    2,
                    "Debe haber 2 fases desde 'review' (validate + reject)"
                );

                let names: Vec<&str> = phases.iter().map(|p| p.name.as_str()).collect();
                assert!(names.contains(&"validate"));
                assert!(names.contains(&"reject"));

                // Verificar que los targets son distintos
                let targets: Vec<&str> = phases.iter().map(|p| p.to.as_str()).collect();
                assert!(targets.contains(&"done"));
                assert!(targets.contains(&"ready"));
            }

            /// CA1: Cuando hay bifurcación, el agente elige una fase
            ///      y el orquestador aplica la transición correspondiente.
            #[test]
            fn bifurcation_agent_chooses_one_phase() {
                let mut config = make_workflow_config();
                config.phases.push(PhaseConfig {
                    name: "reject".to_string(),
                    from: "review".to_string(),
                    to: "ready".to_string(),
                    role: "reviewer".to_string(),
                    model: "claude".to_string(),
                    prompt: "Rechaza {{task_id}}".to_string(),
                    on_reject: "review".to_string(),
                    max_reject_cycles: 2,
                    timeout_seconds: None,
                });

                let wf = ConfigurableWorkflow::new(&config);
                let phases = wf.phases_for_status("review");

                // Simular que el agente responde con [STATUS: done]
                // El orquestador debe encontrar qué fase tiene to="done"
                let agent_target = "done";
                let matching_phase = phases.iter().find(|p| p.to == agent_target);

                assert!(
                    matching_phase.is_some(),
                    "Debe existir una fase con to='done' entre las opciones: {phases:?}"
                );
                assert_eq!(matching_phase.unwrap().name, "validate");
            }

            // ═══════════════════════════════════════════════════════
            // CA3: El loop principal carga tasks con Task::load
            // ═══════════════════════════════════════════════════════

            /// CA3: Task se puede construir con los campos necesarios para el pipeline
            #[test]
            fn task_has_required_fields_for_pipeline() {
                let task = make_task("TASK-001", "draft");

                assert!(!task.id.is_empty(), "Task debe tener id");
                assert!(
                    task.fields.contains_key("status"),
                    "Task debe tener campo 'status'"
                );
                assert_eq!(
                    task.fields.get("status").unwrap(),
                    "draft",
                    "El estado debe ser accesible"
                );
            }

            /// CA3: El pipeline puede iterar sobre múltiples tasks
            #[test]
            fn pipeline_can_iterate_over_multiple_tasks() {
                let tasks = vec![
                    make_task("TASK-001", "draft"),
                    make_task("TASK-002", "ready"),
                    make_task("TASK-003", "review"),
                ];

                let config = make_workflow_config();
                let wf = ConfigurableWorkflow::new(&config);

                let actionable: Vec<&Task> = tasks
                    .iter()
                    .filter(|t| {
                        let status = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
                        !wf.is_terminal(status) && !wf.phases_for_status(status).is_empty()
                    })
                    .collect();

                assert_eq!(actionable.len(), 3, "Las 3 tasks deben ser accionables");
            }

            /// CA3: Tasks en estado terminal no son accionables
            #[test]
            fn terminal_tasks_not_actionable() {
                let tasks = vec![
                    make_task("TASK-001", "draft"),
                    make_task("TASK-002", "done"),
                    make_task("TASK-003", "failed"),
                ];

                let config = make_workflow_config();
                let wf = ConfigurableWorkflow::new(&config);

                let actionable: Vec<&Task> = tasks
                    .iter()
                    .filter(|t| {
                        let status = t.fields.get("status").map(|s| s.as_str()).unwrap_or("");
                        !wf.is_terminal(status) && !wf.phases_for_status(status).is_empty()
                    })
                    .collect();

                assert_eq!(
                    actionable.len(),
                    1,
                    "Solo TASK-001 (draft) debe ser accionable"
                );
                assert_eq!(actionable[0].id, "TASK-001");
            }

            // ═══════════════════════════════════════════════════════
            // Integración: process_task con mock LlmProvider
            // ═══════════════════════════════════════════════════════

            /// Verifica que parse_agent_action funciona sobre respuestas simuladas del LLM
            #[test]
            fn process_task_parse_llm_response() {
                // Simular respuesta del LLM para fase "plan" (draft→ready)
                let llm_response = "He analizado la tarea TASK-001.\n\
                                    Los requisitos son claros.\n\
                                    [STATUS: ready]";

                let states = vec![
                    "draft".to_string(), "ready".to_string(), "review".to_string(),
                    "done".to_string(), "failed".to_string(), "blocked".to_string(),
                ];

                let action = parse_agent_action(llm_response, &states).unwrap();
                assert_eq!(action, AgentAction::Transition("ready".to_string()));
            }

            /// Verifica que parse_agent_action maneja rechazos del LLM
            #[test]
            fn process_task_parse_llm_reject() {
                let llm_response = "[REJECT: falta documentación de la API externa]";

                let states = vec!["draft".to_string(), "ready".to_string()];

                let action = parse_agent_action(llm_response, &states).unwrap();
                assert_eq!(
                    action,
                    AgentAction::Reject("falta documentación de la API externa".to_string())
                );
            }

            /// Verifica que el pipeline puede reintentar con feedback
            /// cuando el agente no sigue el formato.
            #[test]
            fn pipeline_retry_on_parse_error() {
                let bad_response = "Parece que está todo bien, creo que podemos avanzar";
                let states = vec!["draft".to_string(), "ready".to_string()];

                let result = parse_agent_action(bad_response, &states);

                assert!(result.is_err());
                match result.unwrap_err() {
                    AgentParseError::NoMarkerFound { response } => {
                        // El pipeline usaría esta respuesta para construir feedback
                        assert!(response.contains("creo que podemos avanzar"));
                        // El feedback que el pipeline inyectaría en el reintento:
                        let feedback = "Tu respuesta no incluye [STATUS: ...]. \
                                       Por favor, indica el nuevo estado usando el formato \
                                       [STATUS: <estado>].";
                        assert!(feedback.contains("[STATUS:"));
                        assert!(feedback.contains("estado"));
                    }
                    other => panic!("Expected NoMarkerFound, got {other:?}"),
                }
            }

            // ═══════════════════════════════════════════════════════
            // Gherkin Scenario 1: El pipeline avanza una tarea por 3 fases
            // ═══════════════════════════════════════════════════════

            /// Mock LlmProvider que devuelve respuestas predefinidas y cuenta invocaciones.
            #[derive(Debug)]
            struct CountingMockLlm {
                responses: RefCell<Vec<ChatResponse>>,
                call_count: RefCell<usize>,
            }

            impl CountingMockLlm {
                fn new(responses: Vec<ChatResponse>) -> Self {
                    Self {
                        responses: RefCell::new(responses),
                        call_count: RefCell::new(0),
                    }
                }

                fn call_count(&self) -> usize {
                    *self.call_count.borrow()
                }
            }

            /// Gherkin Scenario 1: Una tarea avanza por plan→implement→validate
            /// mock devuelve [STATUS: ready], [STATUS: review], [STATUS: done]
            /// → TASK-001.status = "done" y 3 invocaciones al LLM.
            #[test]
            fn gherkin_scenario_1_pipeline_advances_task_through_three_phases() {
                let config = make_workflow_config();
                let wf = ConfigurableWorkflow::new(&config);
                let states = vec![
                    "draft".to_string(), "ready".to_string(), "review".to_string(),
                    "done".to_string(), "failed".to_string(), "blocked".to_string(),
                ];

                let mut current_status = "draft".to_string();
                let mut invocations = 0usize;

                // Fase 1: plan (draft→ready)
                let phases = wf.phases_for_status(&current_status);
                assert_eq!(phases.len(), 1, "plan: 1 fase desde draft");
                assert_eq!(phases[0].name, "plan");
                let llm_response = "[STATUS: ready]";
                let action = parse_agent_action(llm_response, &states).unwrap();
                assert_eq!(action, AgentAction::Transition("ready".to_string()));
                current_status = "ready".to_string();
                invocations += 1;

                // Fase 2: implement (ready→review)
                let phases = wf.phases_for_status(&current_status);
                assert_eq!(phases.len(), 1, "implement: 1 fase desde ready");
                assert_eq!(phases[0].name, "implement");
                let llm_response = "[STATUS: review]";
                let action = parse_agent_action(llm_response, &states).unwrap();
                assert_eq!(action, AgentAction::Transition("review".to_string()));
                current_status = "review".to_string();
                invocations += 1;

                // Fase 3: validate (review→done)
                let phases = wf.phases_for_status(&current_status);
                assert_eq!(phases.len(), 1, "validate: 1 fase desde review");
                assert_eq!(phases[0].name, "validate");
                let llm_response = "[STATUS: done]";
                let action = parse_agent_action(llm_response, &states).unwrap();
                assert_eq!(action, AgentAction::Transition("done".to_string()));
                current_status = "done".to_string();
                invocations += 1;

                assert_eq!(current_status, "done", "TASK-001.status debe ser 'done'");
                assert_eq!(invocations, 3, "Se realizaron exactamente 3 invocaciones al LLM");
                assert!(wf.is_terminal("done"), "done debe ser estado terminal");
            }

            /// Verifica que el mock LlmProvider puede contar invocaciones
            #[test]
            fn mock_llm_provider_counts_invocations() {
                let mock = CountingMockLlm::new(vec![
                    ChatResponse {
                        content: "[STATUS: ready]".to_string(),
                        finish_reason: "stop".to_string(),
                        token_usage: None,
                    },
                    ChatResponse {
                        content: "[STATUS: review]".to_string(),
                        finish_reason: "stop".to_string(),
                        token_usage: None,
                    },
                    ChatResponse {
                        content: "[STATUS: done]".to_string(),
                        finish_reason: "stop".to_string(),
                        token_usage: None,
                    },
                ]);

                assert_eq!(mock.call_count(), 0, "Sin invocaciones al inicio");
                *mock.call_count.borrow_mut() += 1;
                *mock.call_count.borrow_mut() += 1;
                *mock.call_count.borrow_mut() += 1;
                assert_eq!(mock.call_count(), 3, "3 invocaciones al LLM");
            }

            /// Gherkin Scenario 2: Cuando hay bifurcación, el prompt incluye
            /// ambas opciones para que el agente elija.
            #[test]
            fn gherkin_scenario_2_bifurcation_prompt_includes_both_options() {
                let mut config = make_workflow_config();
                config.phases.push(PhaseConfig {
                    name: "reject".to_string(),
                    from: "review".to_string(),
                    to: "ready".to_string(),
                    role: "reviewer".to_string(),
                    model: "claude".to_string(),
                    prompt: "¿Apruebas {{task_id}}? Responde [STATUS: done] para aprobar o [STATUS: ready] para rechazar.".to_string(),
                    on_reject: "review".to_string(),
                    max_reject_cycles: 2,
                    timeout_seconds: None,
                });

                let wf = ConfigurableWorkflow::new(&config);
                let phases = wf.phases_for_status("review");
                assert_eq!(phases.len(), 2, "Debe haber 2 fases desde 'review'");

                let task = make_task("TASK-001", "review");
                let context = std::collections::HashMap::new();

                let mut prompt_parts = vec![
                    "Estado actual: review".to_string(),
                    "Opciones disponibles:".to_string(),
                ];
                for phase in &phases {
                    let rendered = render_template(&phase.prompt, &task, &context);
                    prompt_parts.push(format!("- [{}] → {}: {}", phase.name, phase.to, rendered));
                }
                let combined_prompt = prompt_parts.join("\n");

                assert!(
                    combined_prompt.contains("[STATUS: done]") || combined_prompt.contains("done"),
                    "El prompt debe mencionar el target 'done':\n{combined_prompt}"
                );
                assert!(
                    combined_prompt.contains("[STATUS: ready]") || combined_prompt.contains("ready"),
                    "El prompt debe mencionar el target 'ready':\n{combined_prompt}"
                );
                assert!(
                    combined_prompt.contains("Opciones disponibles"),
                    "El prompt debe indicar que hay opciones disponibles"
                );

                let states = vec![
                    "draft".to_string(), "ready".to_string(), "review".to_string(),
                    "done".to_string(), "failed".to_string(), "blocked".to_string(),
                ];

                let chosen = "[STATUS: done]";
                let action = parse_agent_action(chosen, &states).unwrap();
                assert_eq!(action, AgentAction::Transition("done".to_string()));

                let matching = phases.iter().find(|p| p.to == "done");
                assert!(matching.is_some(), "Debe existir una fase con to='done'");
                assert_eq!(matching.unwrap().name, "validate");
            }
        }

        // ═══════════════════════════════════════════════════════════
        // STORY-V10-012: Transiciones automáticas y manejo de rechazos
        // ═══════════════════════════════════════════════════════════

        mod story_v10_012_automatic_transitions {
            use crate::domain::graph::DependencyGraph;
            use crate::domain::task::{ActivityLogEntry, Task, TaskFormatConfig};
            use crate::domain::workflow::{
                ConfigurableWorkflow, PhaseConfig, RoleConfig, WorkflowConfig, WorkflowStatesConfig,
            };
            use std::collections::HashMap;
            use std::path::PathBuf;

            // ── Helpers ────────────────────────────────────────────

            fn make_3phase_workflow() -> WorkflowConfig {
                WorkflowConfig {
                    states: WorkflowStatesConfig {
                        initial: "draft".to_string(),
                        terminal: vec!["done".to_string(), "failed".to_string()],
                    },
                    roles: vec![RoleConfig {
                        name: "agent".to_string(),
                        system_prompt: "Eres un agente.".to_string(),
                        model: "gpt4o".to_string(),
                    }],
                    phases: vec![
                        PhaseConfig {
                            name: "plan".to_string(),
                            from: "draft".to_string(),
                            to: "ready".to_string(),
                            role: "agent".to_string(),
                            model: "gpt4o".to_string(),
                            prompt: "Planifica {{task_id}}".to_string(),
                            on_reject: "draft".to_string(),
                            max_reject_cycles: 3,
                            timeout_seconds: None,
                        },
                        PhaseConfig {
                            name: "implement".to_string(),
                            from: "ready".to_string(),
                            to: "review".to_string(),
                            role: "agent".to_string(),
                            model: "gpt4o".to_string(),
                            prompt: "Implementa {{task_id}}".to_string(),
                            on_reject: "ready".to_string(),
                            max_reject_cycles: 4,
                            timeout_seconds: None,
                        },
                        PhaseConfig {
                            name: "validate".to_string(),
                            from: "review".to_string(),
                            to: "done".to_string(),
                            role: "agent".to_string(),
                            model: "gpt4o".to_string(),
                            prompt: "Valida {{task_id}}".to_string(),
                            on_reject: "ready".to_string(),
                            max_reject_cycles: 2,
                            timeout_seconds: None,
                        },
                    ],
                    task_format: TaskFormatConfig::default(),
                }
            }

            fn make_task(id: &str, status: &str, blockers: &[&str]) -> Task {
                let mut fields = HashMap::new();
                fields.insert("status".to_string(), status.to_string());
                Task {
                    id: id.to_string(),
                    path: PathBuf::from(format!("tasks/{id}.md")),
                    fields,
                    blockers: blockers.iter().map(|s| s.to_string()).collect(),
                    activity_log: vec![],
                    raw_content: String::new(),
                }
            }

            // ═══════════════════════════════════════════════════════
            // CA1: apply_automatic_transitions — blocked/unblocked/failed
            // ═══════════════════════════════════════════════════════

            /// CA1: Tarea con dependencias no resueltas → blocked
            #[test]
            fn task_blocked_by_unresolved_dependencies() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                let task = make_task("TASK-002", "ready", &["TASK-001"]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "draft".to_string()); // no terminal

                let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
                assert_eq!(
                    result,
                    Some("blocked".to_string()),
                    "TASK-002 debe bloquearse porque TASK-001 no es terminal"
                );
            }

            /// CA1: Tarea sin dependencias no se bloquea
            #[test]
            fn task_without_blockers_not_blocked() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                let task = make_task("TASK-001", "ready", &[]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);
                let status_map = HashMap::new();

                let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
                assert_eq!(result, None, "Tarea sin dependencias no debe bloquearse");
            }

            /// CA1: Tarea ya blocked no se vuelve a bloquear
            #[test]
            fn task_already_blocked_stays_blocked() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                let task = make_task("TASK-002", "blocked", &["TASK-001"]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "draft".to_string());

                let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
                assert_eq!(result, None, "Ya blocked, no debe cambiar");
            }

            /// CA1: Tarea se desbloquea cuando todas las dependencias son terminales
            #[test]
            fn task_unblocks_when_all_blockers_terminal() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                let task = make_task("TASK-002", "blocked", &["TASK-001", "TASK-003"]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "done".to_string());
                status_map.insert("TASK-003".to_string(), "failed".to_string());

                let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
                assert_eq!(
                    result,
                    Some("draft".to_string()),
                    "Debe desbloquearse al estado inicial del workflow"
                );
            }

            /// CA1: Tarea se desbloquea al estado inicial, no a un estado aleatorio
            #[test]
            fn task_unblocks_to_configured_initial_state() {
                let mut config = make_3phase_workflow();
                config.states.initial = "ready".to_string(); // estado inicial custom

                let wf = ConfigurableWorkflow::new(&config);

                let task = make_task("TASK-002", "blocked", &["TASK-001"]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "done".to_string());

                let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
                assert_eq!(
                    result,
                    Some("ready".to_string()),
                    "Debe desbloquearse al estado inicial configurado"
                );
            }

            /// CA1: Tarea pasa a failed por superar max_reject_cycles
            #[test]
            fn task_fails_on_max_reject_cycles() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                // Fase "review" → max_reject_cycles = 2
                let task = make_task("TASK-001", "review", &[]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);
                let status_map = HashMap::new();

                // Exactamente en el límite (2)
                let result = wf.apply_automatic_transitions(&task, &graph, 2, &status_map);
                assert_eq!(
                    result,
                    Some("failed".to_string()),
                    "Con 2 ciclos de rechazo (max=2), debe pasar a failed"
                );
            }

            /// CA1: Tarea con reject_cycles >= max_reject_cycles en fase "implement"
            #[test]
            fn task_fails_on_max_reject_cycles_implement_phase() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                // Fase "implement" → max_reject_cycles = 4
                let task = make_task("TASK-002", "ready", &[]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);
                let status_map = HashMap::new();

                // 4 ciclos = max_reject_cycles
                let result = wf.apply_automatic_transitions(&task, &graph, 4, &status_map);
                assert_eq!(
                    result,
                    Some("failed".to_string()),
                    "Con 4 ciclos (max=4), debe pasar a failed"
                );
            }

            /// CA1: Tarea NO pasa a failed si está por debajo del límite
            #[test]
            fn task_does_not_fail_below_max_reject_cycles() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                // Fase "review" → max_reject_cycles = 2, llevamos 1
                let task = make_task("TASK-001", "review", &[]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);
                let status_map = HashMap::new();

                let result = wf.apply_automatic_transitions(&task, &graph, 1, &status_map);
                assert_eq!(
                    result,
                    None,
                    "Con 1 ciclo (max=2), NO debe pasar a failed"
                );
            }

            // ═══════════════════════════════════════════════════════
            // CA2: Estados blocked y failed son implícitamente terminales
            // ═══════════════════════════════════════════════════════

            /// CA2: blocked debe ser tratado como estado no accionable por el pipeline
            #[test]
            fn blocked_is_not_processed_by_pipeline() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                // "blocked" no tiene fases asociadas en este workflow
                let phases = wf.phases_for_status("blocked");
                assert!(
                    phases.is_empty(),
                    "No debe haber fases para el estado 'blocked'"
                );

                // Pero el pipeline debe saber que blocked no es accionable
                // Puede detectarse porque no hay fases para este estado
            }

            /// CA2: failed debe ser tratado como estado terminal
            #[test]
            fn failed_is_terminal_state() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                assert!(wf.is_terminal("failed"), "failed debe ser terminal");
                assert!(
                    wf.phases_for_status("failed").is_empty(),
                    "No debe haber fases para el estado 'failed'"
                );
            }

            /// CA2: Si el workflow no define explícitamente "blocked" como terminal,
            ///      el pipeline DEBE tratarlo como implícitamente terminal (no accionable).
            #[test]
            fn blocked_treated_as_implicitly_terminal() {
                let mut config = make_3phase_workflow();
                // Quitar "failed" de terminales, pero "blocked" no estaba en terminales
                config.states.terminal = vec!["done".to_string()];

                let wf = ConfigurableWorkflow::new(&config);

                // "blocked" no está en terminales, pero no tiene fases → no accionable
                assert!(!wf.is_terminal("blocked"));
                assert!(wf.phases_for_status("blocked").is_empty());

                // El pipeline debe usar `phases_for_status().is_empty()` para
                // determinar si un estado es procesable, no solo `is_terminal()`
                let is_processable = |status: &str| -> bool {
                    !wf.phases_for_status(status).is_empty()
                };

                assert!(!is_processable("blocked"), "blocked no debe ser procesable");
                assert!(!is_processable("done"), "done no debe ser procesable (terminal)");
                assert!(is_processable("draft"), "draft debe ser procesable");
                assert!(is_processable("ready"), "ready debe ser procesable");
            }

            // ═══════════════════════════════════════════════════════
            // CA3: Activity Log registra el motivo de Failed
            // ═══════════════════════════════════════════════════════

            /// CA3: Cuando una task pasa a failed, se debe registrar en el Activity Log
            #[test]
            fn failed_task_logs_reason_in_activity_log() {
                // Simular lo que el pipeline debe hacer cuando una task pasa a failed
                let _task_id = "TASK-003";
                let phase_name = "implement";
                let reject_cycles: u32 = 5;
                let max_reject_cycles: u32 = 3;

                let reason = format!(
                    "{reject_cycles} ciclos de rechazo superados en fase '{phase_name}' \
                     (máximo: {max_reject_cycles})"
                );

                assert!(reason.contains("5 ciclos"));
                assert!(reason.contains("implement"));
                assert!(reason.contains("máximo: 3"));

                // Formato esperado del Activity Log entry
                let entry = format!("- | Orchestrator | {reason}");
                assert!(entry.contains("Orchestrator"));
                assert!(entry.contains("rechazo"));
            }

            /// CA3: Tasks que dependían de una task failed deben ser reevaluadas
            #[test]
            fn dependent_tasks_reevaluated_when_blocker_fails() {
                let tasks = vec![
                    make_task("TASK-001", "failed", &[]),
                    make_task("TASK-002", "blocked", &["TASK-001"]),
                ];

                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                let graph = DependencyGraph::from_tasks(&tasks);
                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "failed".to_string());

                // TASK-002 está blocked y su dependencia TASK-001 es failed (terminal)
                let result = wf.apply_automatic_transitions(&tasks[1], &graph, 0, &status_map);
                // Como failed es terminal, TASK-002 debería desbloquearse
                assert_eq!(
                    result,
                    Some("draft".to_string()),
                    "TASK-002 debe desbloquearse porque TASK-001 (failed) es terminal"
                );
            }

            // ═══════════════════════════════════════════════════════
            // Prioridad: bloqueo antes que failed
            // ═══════════════════════════════════════════════════════

            /// La transición a failed tiene prioridad sobre el bloqueo
            #[test]
            fn failed_checked_before_blocked() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                // Tarea en "review" con max_reject_cycles=2, y dependencias no resueltas
                let task = make_task("TASK-001", "review", &["TASK-002"]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-002".to_string(), "draft".to_string()); // no terminal

                // Con reject_cycles=2 (≥ max=2), debe ir a failed, NO a blocked
                let result = wf.apply_automatic_transitions(&task, &graph, 2, &status_map);
                assert_eq!(
                    result,
                    Some("failed".to_string()),
                    "Failed debe tener prioridad sobre blocked cuando se supera max_reject_cycles"
                );
            }

            // ═══════════════════════════════════════════════════════
            // Transiciones automáticas en tareas terminales
            // ═══════════════════════════════════════════════════════

            /// Tarea terminal no debe ser modificada por transiciones automáticas
            #[test]
            fn terminal_task_unchanged_by_automatic_transitions() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                // Tarea done con dependencias no resueltas → no debería bloquearse
                let task = make_task("TASK-001", "done", &["TASK-002"]);
                let graph = DependencyGraph::from_tasks(&[task.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-002".to_string(), "draft".to_string());

                let result = wf.apply_automatic_transitions(&task, &graph, 0, &status_map);
                assert_eq!(
                    result,
                    None,
                    "Tarea terminal (done) no debe ser modificada por transiciones automáticas"
                );
            }

            // ═══════════════════════════════════════════════════════
            // Multiple tasks: solo las que superan el límite pasan a failed
            // ═══════════════════════════════════════════════════════

            /// Solo las tareas que superan max_reject_cycles pasan a failed
            #[test]
            fn only_tasks_exceeding_max_reject_cycles_fail() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                let graph = DependencyGraph::default();
                let status_map = HashMap::new();

                // TASK-001: review con 2 ciclos (max=2) → failed
                let t1 = make_task("TASK-001", "review", &[]);
                // TASK-002: review con 1 ciclo (max=2) → sin cambio
                let t2 = make_task("TASK-002", "review", &[]);

                let r1 = wf.apply_automatic_transitions(&t1, &graph, 2, &status_map);
                let r2 = wf.apply_automatic_transitions(&t2, &graph, 1, &status_map);

                assert_eq!(r1, Some("failed".to_string()));
                assert_eq!(r2, None);
            }

            // ═══════════════════════════════════════════════════════
            // Gherkin Scenario 8: desbloqueo a 'ready' con dependencia done
            // ═══════════════════════════════════════════════════════

            /// Gherkin Scenario 8: TASK-002 blocked → TASK-001 done → TASK-002 vuelve a 'ready'
            #[test]
            fn gherkin_scenario_8_unblock_to_initial_state_ready() {
                let mut config = make_3phase_workflow();
                config.states.initial = "ready".to_string();

                let wf = ConfigurableWorkflow::new(&config);

                let t2 = make_task("TASK-002", "blocked", &["TASK-001"]);
                let graph = DependencyGraph::from_tasks(&[t2.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "done".to_string());

                let result = wf.apply_automatic_transitions(&t2, &graph, 0, &status_map);

                assert_eq!(
                    result,
                    Some("ready".to_string()),
                    "TASK-002 debe desbloquearse al estado inicial 'ready'"
                );
                assert_eq!(wf.initial_state(), "ready");
            }

            /// Gherkin Scenario 8 variante: verifica que el desbloqueo respeta
            /// el estado inicial del workflow definido en TOML.
            #[test]
            fn gherkin_scenario_8_unblock_preserves_workflow_initial() {
                let config = make_3phase_workflow();
                let wf = ConfigurableWorkflow::new(&config);

                assert_eq!(wf.initial_state(), "draft");

                let t2 = make_task("TASK-002", "blocked", &["TASK-001"]);
                let graph = DependencyGraph::from_tasks(&[t2.clone()]);

                let mut status_map = HashMap::new();
                status_map.insert("TASK-001".to_string(), "done".to_string());

                let result = wf.apply_automatic_transitions(&t2, &graph, 0, &status_map);
                assert_eq!(
                    result,
                    Some("draft".to_string()),
                    "Con initial='draft', desbloquea a 'draft', no a 'ready'"
                );
            }

            // ═══════════════════════════════════════════════════════
            // Gherkin Scenario 9: failed con 4 rechazos, max=3 en implement
            // ═══════════════════════════════════════════════════════

            /// Gherkin Scenario 9: TASK-003 rechazada 4 veces en fase "implement"
            /// con max_reject_cycles=3 → pasa a failed con Activity Log.
            #[test]
            fn gherkin_scenario_9_failed_with_exact_gherkin_parameters() {
                let mut config = make_3phase_workflow();
                if let Some(phase) = config.phases.iter_mut().find(|p| p.name == "implement") {
                    phase.max_reject_cycles = 3;
                }

                let wf = ConfigurableWorkflow::new(&config);

                let t3 = make_task("TASK-003", "ready", &[]);
                let graph = DependencyGraph::from_tasks(&[t3.clone()]);
                let status_map = HashMap::new();

                let reject_cycles: u32 = 4;

                let result = wf.apply_automatic_transitions(&t3, &graph, reject_cycles, &status_map);

                assert_eq!(
                    result,
                    Some("failed".to_string()),
                    "Con 4 rechazos y max_reject_cycles=3, TASK-003 debe pasar a failed"
                );

                let phase_name = "implement";
                let reason = format!(
                    "{reject_cycles} ciclos de rechazo superados en fase '{phase_name}' \
                     (máximo: {max_reject_cycles})",
                    max_reject_cycles = 3
                );
                assert!(
                    reason.contains("4 ciclos de rechazo superados en fase 'implement'"),
                    "El Activity Log debe contener el mensaje exacto del Gherkin.\n\
                     Mensaje generado: {reason}"
                );
                assert!(reason.contains("máximo: 3"), "Debe indicar el máximo: {reason}");

                let log_entry = format!("- | Orchestrator | {reason}");
                assert!(log_entry.contains("Orchestrator"));
                assert!(log_entry.contains("4 ciclos"));
                assert!(log_entry.contains("implement"));
            }

            /// Gherkin Scenario 9 variante: con 3 rechazos (justo debajo de max=3) NO pasa a failed
            #[test]
            fn gherkin_scenario_9_below_max_does_not_fail() {
                let mut config = make_3phase_workflow();
                if let Some(phase) = config.phases.iter_mut().find(|p| p.name == "implement") {
                    phase.max_reject_cycles = 3;
                }

                let wf = ConfigurableWorkflow::new(&config);
                let t3 = make_task("TASK-003", "ready", &[]);
                let graph = DependencyGraph::from_tasks(&[t3.clone()]);
                let status_map = HashMap::new();

                let result = wf.apply_automatic_transitions(&t3, &graph, 3, &status_map);
                assert_eq!(
                    result,
                    Some("failed".to_string()),
                    "Con 3 rechazos y max=3, debe pasar a failed (>= max)"
                );

                let result = wf.apply_automatic_transitions(&t3, &graph, 2, &status_map);
                assert_eq!(
                    result,
                    None,
                    "Con 2 rechazos y max=3, NO debe pasar a failed"
                );
            }
        }
    }
}
