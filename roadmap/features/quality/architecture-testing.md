# 🧪 Plan de testing de arquitectura — regista v1.0

> **Objetivo**: Garantizar que las reglas de dependencia R1-R5 se cumplen durante
> y después del rework, con tests automáticos que fallen en CI ante cualquier violación.

---

## 1. Diagnóstico del test actual

**Archivo**: `tests/architecture.rs` (~510 líneas)
**Mecanismo**: Un test `architecture_layers_are_respected` que:

| Paso | Qué hace |
|------|----------|
| Mapeo de archivos | `file_layer(path)` determina la capa por prefijo de directorio (`/cli/`, `/app/`, `/domain/`, `/infra/`) |
| Extracción de imports | `collect_imports(source)` parsea `use crate::X::...` ignorando bloques `#[cfg(test)]` |
| Resolución de capas | `build_module_layer_map()` construye `HashMap<module_name, Layer>` |
| Detección | Por cada `use crate::M` en un archivo de capa L, si M ∉ `L.allowed_imports()` y M ≠ L → violación |
| Reporte | Agrupa por R1/R2/R3 y muestra archivo, línea y el import ofensivo |

**Fortalezas**: Auto-detecta nuevos módulos por path, ignora tests, reporta con precisión.
**Debilidades**: Un solo test monolítico, el mapeo `root_file_layer()` tiene hardcodeados nombres de módulos v0.x que desaparecerán.

---

## 2. Cambios necesarios al test existente

### 2.1 Limpiar `root_file_layer()`

Los módulos que **desaparecen** en el rework deben eliminarse del mapeo:

```rust
fn root_file_layer(module: &str) -> Layer {
    match module {
        // Domain (v1.0)
        "state" | "deadlock" | "graph" | "templates" => Layer::Domain,
        "task" | "workflow" => Layer::Domain,

        // Infrastructure (v1.0)
        "daemon" | "checkpoint" | "git" | "hooks" => Layer::Infra,

        // Application (v1.0)
        "pipeline" | "plan" | "board" | "init" | "validate"
        | "health" | "update" => Layer::App,

        // Config
        "config" => Layer::Config,
        "main"  => Layer::Main,
        _       => Layer::Cli,
    }
}
```

**Eliminados**: `story`, `dependency_graph` (pasó a `graph`), `prompts` (pasó a `templates`), `providers`, `agent`, `orchestrator` (pasó a `pipeline`), `validator` (pasó a `validate`).

### 2.2 Añadir submódulos al `build_module_layer_map()`

Los paths como `infra/llm/openai.rs` generan nombres de módulo `infra::llm::openai`. El mapa necesita entradas para estos:

```rust
// Añadir al build_module_layer_map():
map.entry("infra::llm".to_string()).or_insert(Layer::Infra);
map.entry("app::presets".to_string()).or_insert(Layer::App);
```

### 2.3 Verificar que `file_layer()` maneja los nuevos paths

Los paths ya contienen `/infra/` y `/app/`, así que `infra/llm/openai.rs` → `Layer::Infra` automáticamente. ✅ Sin cambios.

### 2.4 `config.rs` — verificar R5 estrictamente

Actualmente el test dice que `config` no puede importar nada del crate. Con el rework, `config.rs` **debe** seguir sin imports del crate. Si el test detecta `use crate::infra::...` o `use crate::domain::...` en `config.rs`, debe fallar.

Verificación: en v0.x `config.rs` importa `crate::providers` → violación R5. En v1.0, al eliminar `providers`, esta violación desaparece. ✅

---

## 3. Nuevos tests específicos (a añadir)

Además del mega-test, se añaden tests focalizados que verifican políticas concretas:

### Test A: `infra_llm_does_not_import_domain`

```rust
#[test]
fn infra_llm_does_not_import_domain() {
    // Verifica que ningún archivo en infra/llm/ tenga use crate::domain::
}
```

**Motivo**: `infra/llm/types.rs` define `Message`, `ChatResponse`. Podría ser tentador importar `domain::task::Task` para serializarlo. Eso rompería R2.

### Test B: `domain_does_not_import_anyhow`

```rust
#[test]
fn domain_does_not_import_anyhow() {
    // Verifica que ningún archivo en domain/ tenga use anyhow::
}
```

**Motivo**: `anyhow` es una dependencia de infraestructura. El dominio debe usar errores tipados o `std::error::Error`. Si decidimos mantener `anyhow` en domain, este test no se añade.

### Test C: `config_does_not_import_crate_modules`

```rust
#[test]
fn config_does_not_import_crate_modules() {
    // Verifica que config.rs solo importa std + serde + toml
    // Ningún use crate::...
}
```

**Motivo**: R5 estricto. `config.rs` es datos puros. Este test es más específico que el mega-test porque busca explícitamente cualquier `use crate::`.

### Test D: `app_presets_do_not_import_infra`

```rust
#[test]
fn app_presets_do_not_import_infra() {
    // Los presets son datos (WorkflowConfig). No deberían tocar infra/llm/.
}
```

**Motivo**: Los presets definen configuración de workflow. Si un preset intenta instanciar un `LlmProvider`, está mezclando responsabilidades.

### Test E: `layers_are_not_circular`

```rust
#[test]
fn layers_are_not_circular() {
    // Verifica que no hay ciclos: A→B y B→A para ningún par de capas
}
```

**Motivo**: El mega-test detecta violaciones individuales, pero no ciclos. Si `app/pipeline.rs` importa `domain` y `domain/workflow.rs` importa `app` (por error), el mega-test detecta la segunda pero no la relación circular.

---

## 4. Estrategia de implementación por fase

| Fase del rework | Acción en tests de arquitectura |
|-----------------|--------------------------------|
| **Fase 1** (LLM nativo) | Añadir `infra/llm` al mapa de módulos. Ejecutar mega-test → debe pasar (aún no hay imports cruzados). Añadir Test A. |
| **Fase 2** (Dominio genérico) | Actualizar `root_file_layer()`: quitar `story`, `prompts`; añadir `task`, `templates`. Ejecutar → deben pasar. Añadir Test B si se usan errores tipados. |
| **Fase 3** (Pipeline genérico) | Quitar `orchestrator`, añadir `pipeline`. Sin tests nuevos. |
| **Fase 4** (Presets + CLI) | Añadir `app::presets` al mapa. Añadir Test D. |
| **Fase 5** (Limpieza) | Quitar `providers`, `agent`, `dependency_graph`, `validator` del mapeo. Añadir Test C. Ejecutar → 0 violaciones es el objetivo. |
| **Fase 6** (Tests) | Añadir Test E (ciclos). Revisión final: todos los tests pasan. |

---

## 5. Matriz de cobertura

| Regla | Qué verifica | Mega-test | Test específico |
|-------|-------------|-----------|-----------------|
| R1 | domain/ no importa infra, app, cli, config | ✅ | Test B (anyhow) |
| R2 | infra/ no importa domain, app, cli | ✅ | Test A (llm→domain) |
| R3 | app/ no importa cli | ✅ | Test D (presets→infra) |
| R4 | cli/ puede importar todo | ✅ | — (sin restricciones) |
| R5 | config no importa crate | ✅ | Test C (use crate::) |

---

## 6. Cómo añadir un test sin tocar el mega-test

Patrón para tests específicos:

```rust
#[test]
fn infra_llm_does_not_import_domain() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let llm_dir = src_dir.join("infra").join("llm");

    if !llm_dir.exists() {
        return; // skip si el módulo aún no existe (fase temprana)
    }

    let mut violations = Vec::new();

    for entry in fs::read_dir(&llm_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(true, |e| e != "rs") {
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        for (line_no, line) in source.lines().enumerate() {
            if line.trim().starts_with("use crate::domain") {
                violations.push(format!(
                    "{}:{} — {}\n  → infra/llm/ imports domain, viola R2",
                    path.file_name().unwrap().to_string_lossy(),
                    line_no + 1,
                    line.trim(),
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "infra/llm/ imports domain:\n{}",
        violations.join("\n")
    );
}
```

---

## 7. Ejecución en CI

```yaml
# .github/workflows/ci.yml (fragmento relevante)
- name: Architecture tests
  run: cargo test --test architecture
```

Los tests de arquitectura son `#[test]` normales dentro de `tests/architecture.rs`. El comando `cargo test --test architecture` los ejecuta todos.

**Exit code**: 0 si todas las reglas se cumplen, 1 si hay violaciones. Bloquea el merge.

---

## 8. Resumen

| Ítem | Cantidad |
|------|----------|
| Tests existentes (mega-test) | 1 a actualizar |
| Tests específicos nuevos | 5 (A, B, C, D, E) |
| Tests helper (parseo, mapeo) | 7 (ya existen, se actualizan) |
| **Total** | **13 tests** |
| Fases donde se toca el test | Las 6 fases |
| Coste de mantenimiento | Bajo: añadir/quitar entradas en `root_file_layer()` |
