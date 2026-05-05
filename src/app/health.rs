//! Health & metrics endpoint para monitorizar el pipeline en ejecución.
//!
//! Expone métricas clave (iteraciones/hora, tiempo medio por agente,
//! tasa de rechazo, throughput, coste estimado) y las vuelca a
//! `.regista/health.json` de forma atómica cada N iteraciones.
//!
//! Consumido por el TUI/dashboard (#11) y cost tracking (#12).
//!
//! Nota: Los items públicos están marcados #[allow(dead_code)] porque
//! la integración en el pipeline ocurrirá en historias posteriores.
//! Los tests validan el comportamiento correcto del módulo.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── HealthReport ──────────────────────────────────────────────────────────

/// Métricas agregadas del pipeline en un instante dado.
///
/// Todos los campos son calculados; no llevan lógica de negocio interna.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Iteraciones del loop principal por hora de pared.
    pub iterations_per_hour: f64,
    /// Tiempo medio en segundos que tarda una invocación de agente.
    pub mean_agent_time_seconds: f64,
    /// Proporción de transiciones que fueron rechazos (rechazos / total).
    pub rejection_rate: f64,
    /// Historias completadas (Done) por hora de pared.
    pub stories_per_hour: f64,
    /// Coste estimado acumulado en USD (modelo naive: tokens × precio / 1M).
    pub estimated_cost_usd: f64,
    /// Iteración actual del loop del orquestador.
    pub current_iteration: u32,
    /// Número de historias en estado Done.
    pub stories_done: u32,
    /// Número de historias en estado Failed.
    pub stories_failed: u32,
    /// Número de historias activas (ni Done ni Failed).
    pub stories_active: u32,
    /// Segundos transcurridos de pared desde el inicio del pipeline.
    pub elapsed_wall_time_seconds: u64,
}

// ── Funciones públicas ────────────────────────────────────────────────────

/// Calcula un `HealthReport` a partir de los datos crudos del orchestrator.
///
/// El caller (orquestador) recolecta las estadísticas del estado compartido,
/// las historias, y el wall-clock, y las pasa aquí para el cálculo.
#[allow(clippy::too_many_arguments)]
pub fn generate_report(
    current_iteration: u32,
    elapsed_wall_time_seconds: u64,
    stories_done: u32,
    stories_failed: u32,
    stories_active: u32,
    total_agent_time_seconds: f64,
    total_agent_invocations: u64,
    total_rejected_transitions: u64,
    total_transitions: u64,
    estimated_cost_usd: f64,
) -> HealthReport {
    let hours = elapsed_wall_time_seconds as f64 / 3600.0;

    let iterations_per_hour = if hours > 0.0 {
        current_iteration as f64 / hours
    } else {
        0.0
    };

    let stories_per_hour = if hours > 0.0 {
        stories_done as f64 / hours
    } else {
        0.0
    };

    let mean_agent_time_seconds = if total_agent_invocations > 0 {
        total_agent_time_seconds / total_agent_invocations as f64
    } else {
        0.0
    };

    let rejection_rate = if total_transitions > 0 {
        total_rejected_transitions as f64 / total_transitions as f64
    } else {
        0.0
    };

    HealthReport {
        iterations_per_hour,
        mean_agent_time_seconds,
        rejection_rate,
        stories_per_hour,
        estimated_cost_usd,
        current_iteration,
        stories_done,
        stories_failed,
        stories_active,
        elapsed_wall_time_seconds,
    }
}

/// Determina si la iteración actual es un checkpoint de salud.
///
/// Por defecto cada 10 iteraciones (`interval = 10`), configurable.
/// La iteración 0 SIEMPRE es checkpoint (reporte inicial).
pub fn is_health_checkpoint(iteration: u32, interval: u32) -> bool {
    if iteration == 0 {
        return true;
    }
    if interval == 0 {
        return false;
    }
    iteration.is_multiple_of(interval)
}

/// Escribe el reporte a `.regista/health.json` de forma atómica.
///
/// Primero escribe a `.regista/health.json.tmp` y luego renombra,
/// garantizando que el archivo nunca está en estado parcial.
pub fn write_health_json(report: &HealthReport, project_root: &Path) -> anyhow::Result<()> {
    let regista_dir = project_root.join(".regista");
    std::fs::create_dir_all(&regista_dir)?;

    let tmp_path = regista_dir.join("health.json.tmp");
    let final_path = regista_dir.join("health.json");

    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&tmp_path, json)?;
    std::fs::rename(&tmp_path, &final_path)?;

    tracing::debug!(
        "💚 health.json actualizado (iteración {})",
        report.current_iteration
    );
    Ok(())
}

/// Escribe el reporte final cuando el pipeline termina (PipelineComplete).
///
/// Es idéntico a `write_health_json` pero usa un nombre fijo y loguea
/// el evento como cierre del pipeline.
pub fn write_final_health_report(report: &HealthReport, project_root: &Path) -> anyhow::Result<()> {
    tracing::info!("🏁 Pipeline completo — escribiendo health report final");
    write_health_json(report, project_root)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════════════
    // CA1: HealthReport struct con todos los campos requeridos
    // ═══════════════════════════════════════════════════════════════════════

    /// CA1: HealthReport se puede construir con todos los campos.
    #[test]
    fn healthreport_can_be_constructed_with_all_fields() {
        let report = HealthReport {
            iterations_per_hour: 42.0,
            mean_agent_time_seconds: 30.5,
            rejection_rate: 0.15,
            stories_per_hour: 2.5,
            estimated_cost_usd: 1.23,
            current_iteration: 100,
            stories_done: 5,
            stories_failed: 1,
            stories_active: 3,
            elapsed_wall_time_seconds: 3600,
        };

        assert_eq!(report.iterations_per_hour, 42.0);
        assert_eq!(report.mean_agent_time_seconds, 30.5);
        assert_eq!(report.rejection_rate, 0.15);
        assert_eq!(report.stories_per_hour, 2.5);
        assert_eq!(report.estimated_cost_usd, 1.23);
        assert_eq!(report.current_iteration, 100);
        assert_eq!(report.stories_done, 5);
        assert_eq!(report.stories_failed, 1);
        assert_eq!(report.stories_active, 3);
        assert_eq!(report.elapsed_wall_time_seconds, 3600);
    }

    /// CA1: Todos los campos tienen el tipo correcto.
    /// (Si los tipos no coincidieran, este test ni compilaría.)
    #[test]
    fn healthreport_fields_have_correct_types() {
        let report = HealthReport {
            iterations_per_hour: 0.0_f64,
            mean_agent_time_seconds: 0.0_f64,
            rejection_rate: 0.0_f64,
            stories_per_hour: 0.0_f64,
            estimated_cost_usd: 0.0_f64,
            current_iteration: 0_u32,
            stories_done: 0_u32,
            stories_failed: 0_u32,
            stories_active: 0_u32,
            elapsed_wall_time_seconds: 0_u64,
        };

        // Verificación de tipo en assignments (no compilaría si el tipo no es f64)
        let _iter: f64 = report.iterations_per_hour;
        let _mean: f64 = report.mean_agent_time_seconds;
        let _rej: f64 = report.rejection_rate;
        let _sph: f64 = report.stories_per_hour;
        let _cost: f64 = report.estimated_cost_usd;
        let _ci: u32 = report.current_iteration;
        let _done: u32 = report.stories_done;
        let _fail: u32 = report.stories_failed;
        let _active: u32 = report.stories_active;
        let _elapsed: u64 = report.elapsed_wall_time_seconds;
    }

    /// CA1: HealthReport implementa Clone.
    #[test]
    fn healthreport_is_clone() {
        let report = HealthReport {
            iterations_per_hour: 10.0,
            mean_agent_time_seconds: 25.0,
            rejection_rate: 0.2,
            stories_per_hour: 1.5,
            estimated_cost_usd: 0.75,
            current_iteration: 50,
            stories_done: 3,
            stories_failed: 0,
            stories_active: 7,
            elapsed_wall_time_seconds: 7200,
        };
        let cloned = report.clone();
        assert_eq!(cloned.iterations_per_hour, report.iterations_per_hour);
        assert_eq!(cloned.current_iteration, report.current_iteration);
        assert_eq!(cloned.stories_done, report.stories_done);
    }

    /// CA1: HealthReport implementa Debug.
    #[test]
    fn healthreport_is_debug() {
        let report = HealthReport {
            iterations_per_hour: 1.0,
            mean_agent_time_seconds: 1.0,
            rejection_rate: 0.0,
            stories_per_hour: 1.0,
            estimated_cost_usd: 0.0,
            current_iteration: 1,
            stories_done: 0,
            stories_failed: 0,
            stories_active: 1,
            elapsed_wall_time_seconds: 1,
        };
        // Solo verificamos que Debug no paniquea
        let _ = format!("{report:?}");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // CA2: generate_report calcula correctamente todas las métricas
    // ═══════════════════════════════════════════════════════════════════════

    /// CA2: Happy path — valores normales, todos los campos calculados.
    #[test]
    fn generate_report_happy_path() {
        let report = generate_report(
            120,    // current_iteration
            3600,   // elapsed_wall_time_seconds = 1 hora
            5,      // stories_done
            1,      // stories_failed
            4,      // stories_active
            3000.0, // total_agent_time_seconds
            100,    // total_agent_invocations
            20,     // total_rejected_transitions
            120,    // total_transitions
            2.50,   // estimated_cost_usd
        );

        // iterations_per_hour = 120 / 1.0h = 120
        assert!((report.iterations_per_hour - 120.0).abs() < 0.001);
        // mean_agent_time_seconds = 3000 / 100 = 30
        assert!((report.mean_agent_time_seconds - 30.0).abs() < 0.001);
        // rejection_rate = 20 / 120 ≈ 0.1667
        assert!((report.rejection_rate - 0.1666667).abs() < 0.001);
        // stories_per_hour = 5 / 1.0h = 5
        assert!((report.stories_per_hour - 5.0).abs() < 0.001);
        // estimated_cost_usd se pasa tal cual
        assert!((report.estimated_cost_usd - 2.50).abs() < 0.001);
        // Campos passthrough
        assert_eq!(report.current_iteration, 120);
        assert_eq!(report.stories_done, 5);
        assert_eq!(report.stories_failed, 1);
        assert_eq!(report.stories_active, 4);
        assert_eq!(report.elapsed_wall_time_seconds, 3600);
    }

    /// CA2: Media hora de ejecución (3600/2 = 1800s).
    #[test]
    fn generate_report_half_hour() {
        let report = generate_report(
            60,     // current_iteration
            1800,   // 0.5 horas
            3,      // stories_done
            0,      // stories_failed
            2,      // stories_active
            1500.0, // total_agent_time_seconds
            50,     // invocations
            5,      // rejected
            55,     // total_transitions
            1.00,   // cost
        );

        // iterations_per_hour = 60 / 0.5 = 120
        assert!((report.iterations_per_hour - 120.0).abs() < 0.001);
        // stories_per_hour = 3 / 0.5 = 6
        assert!((report.stories_per_hour - 6.0).abs() < 0.001);
        // mean_agent_time = 1500 / 50 = 30
        assert!((report.mean_agent_time_seconds - 30.0).abs() < 0.001);
        // rejection_rate = 5/55 ≈ 0.0909
        assert!((report.rejection_rate - 0.090909).abs() < 0.001);
    }

    /// CA2: Tiempo transcurrido cero → métricas por hora son 0 (no NaN).
    #[test]
    fn generate_report_zero_elapsed_time() {
        let report = generate_report(
            10,    // current_iteration
            0,     // elapsed_wall_time_seconds = 0
            1,     // stories_done
            0,     // stories_failed
            0,     // stories_active
            100.0, // total_agent_time_seconds
            5,     // invocations
            1,     // rejected
            5,     // total_transitions
            0.50,  // cost
        );

        // Sin tiempo transcurrido, métricas por hora = 0
        assert_eq!(report.iterations_per_hour, 0.0);
        assert_eq!(report.stories_per_hour, 0.0);
        // Pero mean_agent_time y rejection_rate sí se calculan
        assert!((report.mean_agent_time_seconds - 20.0).abs() < 0.001);
        assert!((report.rejection_rate - 0.2).abs() < 0.001);
        assert_eq!(report.elapsed_wall_time_seconds, 0);
    }

    /// CA2: Sin invocaciones de agente → mean_agent_time = 0.
    #[test]
    fn generate_report_zero_invocations() {
        let report = generate_report(
            1, 3600, 0, 0, 1, 0.0, // total_agent_time_seconds
            0,   // total_agent_invocations = 0
            0, 0, 0.0,
        );

        assert_eq!(report.mean_agent_time_seconds, 0.0);
    }

    /// CA2: Sin transiciones → rejection_rate = 0 (evita división por cero).
    #[test]
    fn generate_report_zero_transitions() {
        let report = generate_report(
            1, 3600, 0, 0, 1, 0.0, 0, 0, 0, // total_transitions = 0
            0.0,
        );

        assert_eq!(report.rejection_rate, 0.0);
    }

    /// CA2: 100% de rechazo.
    #[test]
    fn generate_report_full_rejection_rate() {
        let report = generate_report(
            10, 3600, 0, 0, 5, 500.0, 10, 10, // todos rechazos
            10, // total = rechazos
            0.0,
        );

        assert!((report.rejection_rate - 1.0).abs() < 0.001);
    }

    /// CA2: Todas las historias Done, ninguna activa.
    #[test]
    fn generate_report_all_done() {
        let report = generate_report(
            50, 7200, // 2 horas
            10,   // stories_done
            0, 0, // stories_active = 0
            2000.0, 80, 5, 85, 3.50,
        );

        assert_eq!(report.stories_done, 10);
        assert_eq!(report.stories_failed, 0);
        assert_eq!(report.stories_active, 0);
        // stories_per_hour = 10 / 2h = 5
        assert!((report.stories_per_hour - 5.0).abs() < 0.001);
    }

    /// CA2: Coste estimado se preserva exactamente.
    #[test]
    fn generate_report_preserves_cost() {
        let report = generate_report(1, 3600, 0, 0, 1, 0.0, 0, 0, 0, 42.42);
        assert!((report.estimated_cost_usd - 42.42).abs() < 0.0001);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // CA3: HealthReport implementa Serialize + checkpoint cada N iteraciones
    // ═══════════════════════════════════════════════════════════════════════

    /// CA3: HealthReport serializa a JSON correctamente.
    #[test]
    fn healthreport_serializes_to_json() {
        let report = HealthReport {
            iterations_per_hour: 120.0,
            mean_agent_time_seconds: 30.0,
            rejection_rate: 0.1667,
            stories_per_hour: 5.0,
            estimated_cost_usd: 2.50,
            current_iteration: 120,
            stories_done: 5,
            stories_failed: 1,
            stories_active: 4,
            elapsed_wall_time_seconds: 3600,
        };

        let json =
            serde_json::to_string_pretty(&report).expect("HealthReport debe serializar a JSON");

        // Verificar que todos los campos aparecen en el JSON
        assert!(json.contains("\"iterations_per_hour\""));
        assert!(json.contains("\"mean_agent_time_seconds\""));
        assert!(json.contains("\"rejection_rate\""));
        assert!(json.contains("\"stories_per_hour\""));
        assert!(json.contains("\"estimated_cost_usd\""));
        assert!(json.contains("\"current_iteration\""));
        assert!(json.contains("\"stories_done\""));
        assert!(json.contains("\"stories_failed\""));
        assert!(json.contains("\"stories_active\""));
        assert!(json.contains("\"elapsed_wall_time_seconds\""));

        // Verificar valores numéricos
        assert!(json.contains("120.0"));
        assert!(json.contains("30.0"));
        assert!(json.contains("5"));
        assert!(json.contains("3600"));
    }

    /// CA3: HealthReport deserializa desde JSON (roundtrip).
    #[test]
    fn healthreport_json_roundtrip() {
        let original = HealthReport {
            iterations_per_hour: 42.5,
            mean_agent_time_seconds: 18.3,
            rejection_rate: 0.1,
            stories_per_hour: 3.2,
            estimated_cost_usd: 1.11,
            current_iteration: 77,
            stories_done: 4,
            stories_failed: 2,
            stories_active: 6,
            elapsed_wall_time_seconds: 5400,
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: HealthReport =
            serde_json::from_str(&json).expect("HealthReport debe deserializar desde JSON");

        assert!((parsed.iterations_per_hour - original.iterations_per_hour).abs() < 0.001);
        assert!((parsed.mean_agent_time_seconds - original.mean_agent_time_seconds).abs() < 0.001);
        assert!((parsed.rejection_rate - original.rejection_rate).abs() < 0.001);
        assert!((parsed.stories_per_hour - original.stories_per_hour).abs() < 0.001);
        assert!((parsed.estimated_cost_usd - original.estimated_cost_usd).abs() < 0.001);
        assert_eq!(parsed.current_iteration, original.current_iteration);
        assert_eq!(parsed.stories_done, original.stories_done);
        assert_eq!(parsed.stories_failed, original.stories_failed);
        assert_eq!(parsed.stories_active, original.stories_active);
        assert_eq!(
            parsed.elapsed_wall_time_seconds,
            original.elapsed_wall_time_seconds
        );
    }

    /// CA3: Iteración 0 siempre es checkpoint (reporte inicial).
    #[test]
    fn is_health_checkpoint_iteration_zero_always_true() {
        assert!(is_health_checkpoint(0, 10));
        assert!(is_health_checkpoint(0, 5));
        assert!(is_health_checkpoint(0, 1));
        assert!(is_health_checkpoint(0, 100));
    }

    /// CA3: Intervalo por defecto (10): cada 10 iteraciones.
    #[test]
    fn is_health_checkpoint_default_interval_10() {
        let interval = 10;

        // Checkpoints esperados: 0, 10, 20, 30, ...
        assert!(is_health_checkpoint(0, interval));
        assert!(is_health_checkpoint(10, interval));
        assert!(is_health_checkpoint(20, interval));
        assert!(is_health_checkpoint(30, interval));
        assert!(is_health_checkpoint(100, interval));
        assert!(is_health_checkpoint(1000, interval));

        // No checkpoints
        assert!(!is_health_checkpoint(1, interval));
        assert!(!is_health_checkpoint(9, interval));
        assert!(!is_health_checkpoint(11, interval));
        assert!(!is_health_checkpoint(15, interval));
        assert!(!is_health_checkpoint(99, interval));
        assert!(!is_health_checkpoint(101, interval));
    }

    /// CA3: Intervalo configurable (ej: 5 iteraciones).
    #[test]
    fn is_health_checkpoint_custom_interval_5() {
        let interval = 5;

        assert!(is_health_checkpoint(0, interval));
        assert!(is_health_checkpoint(5, interval));
        assert!(is_health_checkpoint(10, interval));
        assert!(is_health_checkpoint(15, interval));
        assert!(is_health_checkpoint(50, interval));

        assert!(!is_health_checkpoint(1, interval));
        assert!(!is_health_checkpoint(4, interval));
        assert!(!is_health_checkpoint(6, interval));
        assert!(!is_health_checkpoint(11, interval));
    }

    /// CA3: Intervalo = 1 → cada iteración es checkpoint.
    #[test]
    fn is_health_checkpoint_interval_1_every_iteration() {
        let interval = 1;

        for i in 0..20 {
            assert!(
                is_health_checkpoint(i, interval),
                "Iteración {i} debería ser checkpoint con interval=1"
            );
        }
    }

    /// CA3: Intervalo = 0 → solo la iteración 0 es checkpoint.
    #[test]
    fn is_health_checkpoint_interval_zero_only_initial() {
        let interval = 0;

        assert!(is_health_checkpoint(0, interval));
        assert!(!is_health_checkpoint(1, interval));
        assert!(!is_health_checkpoint(10, interval));
        assert!(!is_health_checkpoint(100, interval));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // CA4: Escritura atómica de health.json (tmp → rename)
    // ═══════════════════════════════════════════════════════════════════════

    /// CA4: write_health_json crea el archivo en .regista/health.json.
    #[test]
    fn write_health_json_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let report = sample_report();

        write_health_json(&report, tmp.path()).unwrap();

        let health_path = tmp.path().join(".regista/health.json");
        assert!(health_path.exists(), "health.json debe existir");

        let content = std::fs::read_to_string(&health_path).unwrap();
        assert!(content.contains("\"iterations_per_hour\""));
        assert!(content.contains("\"current_iteration\""));
    }

    /// CA4: El contenido escrito coincide con el reporte (roundtrip).
    #[test]
    fn write_health_json_content_matches_report() {
        let tmp = tempfile::tempdir().unwrap();
        let report = HealthReport {
            iterations_per_hour: 77.7,
            mean_agent_time_seconds: 12.3,
            rejection_rate: 0.33,
            stories_per_hour: 4.4,
            estimated_cost_usd: 9.99,
            current_iteration: 42,
            stories_done: 7,
            stories_failed: 3,
            stories_active: 5,
            elapsed_wall_time_seconds: 9999,
        };

        write_health_json(&report, tmp.path()).unwrap();

        let health_path = tmp.path().join(".regista/health.json");
        let content = std::fs::read_to_string(&health_path).unwrap();
        let parsed: HealthReport =
            serde_json::from_str(&content).expect("Debe poder parsearse el JSON escrito");

        assert!((parsed.iterations_per_hour - 77.7).abs() < 0.001);
        assert!((parsed.mean_agent_time_seconds - 12.3).abs() < 0.001);
        assert!((parsed.rejection_rate - 0.33).abs() < 0.001);
        assert!((parsed.stories_per_hour - 4.4).abs() < 0.001);
        assert!((parsed.estimated_cost_usd - 9.99).abs() < 0.001);
        assert_eq!(parsed.current_iteration, 42);
        assert_eq!(parsed.stories_done, 7);
        assert_eq!(parsed.stories_failed, 3);
        assert_eq!(parsed.stories_active, 5);
        assert_eq!(parsed.elapsed_wall_time_seconds, 9999);
    }

    /// CA4: Escrituras sucesivas sobreescriben el archivo correctamente.
    #[test]
    fn write_health_json_overwrites() {
        let tmp = tempfile::tempdir().unwrap();

        let report1 = HealthReport {
            iterations_per_hour: 1.0,
            mean_agent_time_seconds: 1.0,
            rejection_rate: 0.0,
            stories_per_hour: 1.0,
            estimated_cost_usd: 0.0,
            current_iteration: 1,
            stories_done: 0,
            stories_failed: 0,
            stories_active: 1,
            elapsed_wall_time_seconds: 1,
        };

        let report2 = HealthReport {
            iterations_per_hour: 2.0,
            mean_agent_time_seconds: 2.0,
            rejection_rate: 0.5,
            stories_per_hour: 2.0,
            estimated_cost_usd: 10.0,
            current_iteration: 2,
            stories_done: 1,
            stories_failed: 0,
            stories_active: 2,
            elapsed_wall_time_seconds: 2,
        };

        write_health_json(&report1, tmp.path()).unwrap();
        write_health_json(&report2, tmp.path()).unwrap();

        let health_path = tmp.path().join(".regista/health.json");
        let content = std::fs::read_to_string(&health_path).unwrap();
        let parsed: HealthReport = serde_json::from_str(&content).unwrap();

        // Debe reflejar el segundo reporte, no el primero
        assert_eq!(parsed.current_iteration, 2);
        assert!((parsed.iterations_per_hour - 2.0).abs() < 0.001);
    }

    /// CA4: El archivo temporal (.tmp) NO debe persistir tras la escritura.
    #[test]
    fn write_health_json_no_temp_file_left_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let report = sample_report();

        write_health_json(&report, tmp.path()).unwrap();

        let tmp_path = tmp.path().join(".regista/health.json.tmp");
        assert!(
            !tmp_path.exists(),
            "El archivo temporal health.json.tmp NO debe quedar tras escritura exitosa"
        );
    }

    /// CA4: La escritura crea el directorio .regista si no existe.
    #[test]
    fn write_health_json_creates_regista_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let regista_dir = tmp.path().join(".regista");

        // Asegurar que no existe antes
        if regista_dir.exists() {
            std::fs::remove_dir_all(&regista_dir).unwrap();
        }
        assert!(!regista_dir.exists());

        let report = sample_report();
        write_health_json(&report, tmp.path()).unwrap();

        assert!(regista_dir.exists(), ".regista/ debe ser creado");
        assert!(
            regista_dir.join("health.json").exists(),
            "health.json debe existir dentro de .regista/"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // CA5: Reporte final en PipelineComplete
    // ═══════════════════════════════════════════════════════════════════════

    /// CA5: write_final_health_report escribe el reporte igual que write_health_json.
    #[test]
    fn write_final_health_report_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let report = HealthReport {
            iterations_per_hour: 50.0,
            mean_agent_time_seconds: 35.0,
            rejection_rate: 0.08,
            stories_per_hour: 2.0,
            estimated_cost_usd: 5.75,
            current_iteration: 200,
            stories_done: 10,
            stories_failed: 2,
            stories_active: 0, // Pipeline completo → 0 activas
            elapsed_wall_time_seconds: 14400,
        };

        write_final_health_report(&report, tmp.path()).unwrap();

        let health_path = tmp.path().join(".regista/health.json");
        assert!(
            health_path.exists(),
            "health.json debe existir tras reporte final"
        );

        let content = std::fs::read_to_string(&health_path).unwrap();
        let parsed: HealthReport = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.current_iteration, 200);
        assert_eq!(parsed.stories_done, 10);
        assert_eq!(parsed.stories_failed, 2);
        assert_eq!(parsed.stories_active, 0);
    }

    /// CA5: Reporte final con todas las historias en Done.
    #[test]
    fn write_final_report_all_done() {
        let tmp = tempfile::tempdir().unwrap();
        let report = HealthReport {
            iterations_per_hour: 30.0,
            mean_agent_time_seconds: 40.0,
            rejection_rate: 0.0,
            stories_per_hour: 1.0,
            estimated_cost_usd: 10.00,
            current_iteration: 300,
            stories_done: 5,
            stories_failed: 0,
            stories_active: 0,
            elapsed_wall_time_seconds: 18000,
        };

        write_final_health_report(&report, tmp.path()).unwrap();

        let health_path = tmp.path().join(".regista/health.json");
        let content = std::fs::read_to_string(&health_path).unwrap();
        let parsed: HealthReport = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.stories_done, 5);
        assert_eq!(parsed.stories_failed, 0);
        assert_eq!(parsed.stories_active, 0);
        assert_eq!(parsed.rejection_rate, 0.0);
    }

    /// CA5: Reporte final con pipeline que terminó con todas Failed.
    #[test]
    fn write_final_report_all_failed() {
        let tmp = tempfile::tempdir().unwrap();
        let report = HealthReport {
            iterations_per_hour: 10.0,
            mean_agent_time_seconds: 20.0,
            rejection_rate: 1.0,
            stories_per_hour: 0.0,
            estimated_cost_usd: 15.00,
            current_iteration: 80,
            stories_done: 0,
            stories_failed: 5,
            stories_active: 0,
            elapsed_wall_time_seconds: 3600,
        };

        write_final_health_report(&report, tmp.path()).unwrap();

        let health_path = tmp.path().join(".regista/health.json");
        let content = std::fs::read_to_string(&health_path).unwrap();
        let parsed: HealthReport = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.stories_done, 0);
        assert_eq!(parsed.stories_failed, 5);
        assert_eq!(parsed.stories_active, 0);
        assert!((parsed.rejection_rate - 1.0).abs() < 0.001);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Helper para los tests
    // ═══════════════════════════════════════════════════════════════════════

    fn sample_report() -> HealthReport {
        HealthReport {
            iterations_per_hour: 60.0,
            mean_agent_time_seconds: 45.0,
            rejection_rate: 0.12,
            stories_per_hour: 3.0,
            estimated_cost_usd: 2.50,
            current_iteration: 30,
            stories_done: 2,
            stories_failed: 1,
            stories_active: 4,
            elapsed_wall_time_seconds: 1800,
        }
    }
}
