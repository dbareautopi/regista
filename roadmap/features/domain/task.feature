# language: es
@domain @task @STORY-V10-006

Feature: Task genérico con parseo configurable
  Como orquestador de tareas,
  quiero poder parsear archivos .md con formato configurable,
  para que regista soporte cualquier dominio sin hardcodear STORY-NNN.

  Background:
    Given que existe un archivo "TASK-001.md" en el directorio de tareas
    And el workflow define task_format con:
      | campo             | valor                                      |
      | id_pattern        | TASK-\d+                                   |
      | section_markers   | {"status": "## Status", "priority": "## Priority", "description": "## Descripción"} |
      | dependency_marker | Bloqueado por:                             |

  # ── Capa: Dominio (domain/task.rs) ───────────────────────────────────

  # CA1: Task::load extrae ID usando id_pattern configurable
  Scenario: Extraer ID con patrón configurable TASK-NNN
    Given el archivo "TASK-001.md" existe
    And el task_format.id_pattern es "TASK-\d+"
    When se invoca Task::load(path, task_format)
    Then el Task resultante tiene id "TASK-001"
    And el campo id no contiene el prefijo del path ni la extensión .md

  Scenario: Extraer ID con patrón arbitrario ISSUE-NNN
    Given el archivo "ISSUE-042.md" existe
    And el task_format.id_pattern es "ISSUE-\d+"
    When se invoca Task::load(path, task_format)
    Then el Task resultante tiene id "ISSUE-042"

  Scenario: Fallar si el ID no cumple el patrón configurado
    Given el archivo "mytask.md" existe (sin número)
    And el task_format.id_pattern es "TASK-\d+"
    When se invoca Task::load(path, task_format)
    Then devuelve Err con mensaje que contiene "id_pattern" y el nombre del archivo

  # CA1: Parsear múltiples section_markers simultáneamente
  Scenario: Parsear varios campos definidos en section_markers
    Given el archivo "TASK-001.md" contiene:
      """
      ## Status
      **pending**

      ## Priority
      high

      ## Descripción
      Implementar el parser genérico de tareas
      """
    When se invoca Task::load() con el task_format configurado
    Then task.fields["status"] es "pending"
    And task.fields["priority"] es "high"
    And task.fields["description"] contiene "parser genérico"
    And task.id es "TASK-001"

  Scenario: Campos no presentes en el archivo se omiten de fields
    Given el archivo "TASK-002.md" solo contiene "## Status\n**draft**"
    And el section_markers incluye "status", "priority" y "description"
    When se invoca Task::load()
    Then task.fields["status"] es "draft"
    And task.fields no contiene la clave "priority"
    And task.fields no contiene la clave "description"

  Scenario: raw_content preserva el contenido completo del archivo
    Given el archivo "TASK-001.md" tiene 3 secciones y texto libre
    When se invoca Task::load()
    Then task.raw_content contiene el texto íntegro del archivo original
    And task.raw_content incluye tanto las secciones parseadas como cualquier texto entre ellas

  # CA2: Extraer dependencias con dependency_marker configurable
  Scenario: Extraer dependencias con dependency_marker "Bloqueado por:"
    Given el archivo "TASK-001.md" contiene:
      """
      ## Dependencias
      - Bloqueado por: TASK-002, TASK-003
      """
    And el task_format.dependency_marker es "Bloqueado por:"
    When se invoca Task::load()
    Then task.blockers contiene ["TASK-002", "TASK-003"]

  Scenario: Extraer dependencias con dependency_marker personalizado
    Given el archivo "ISSUE-005.md" contiene:
      """
      ## Relations
      - Depends on: ISSUE-001, ISSUE-002
      """
    And el task_format.dependency_marker es "Depends on:"
    When se invoca Task::load()
    Then task.blockers contiene ["ISSUE-001", "ISSUE-002"]

  Scenario: Sin dependencias devuelve blockers vacío
    Given el archivo "TASK-001.md" no contiene ninguna línea con el dependency_marker
    When se invoca Task::load()
    Then task.blockers es un vector vacío

  # CA2: Parsear activity_log desde "## Activity Log"
  Scenario: Parsear activity_log con entradas fecha | actor | descripción
    Given el archivo "TASK-001.md" contiene:
      """
      ## Activity Log
      - 2026-05-08 | PO | Historia creada
      - 2026-05-09 | Dev | Implementación iniciada
      - 2026-05-10 | Reviewer | RECHAZADO: falta cobertura de tests
      """
    When se invoca Task::load()
    Then task.activity_log tiene 3 entradas
    And la primera entrada tiene fecha="2026-05-08", actor="PO", descripción="Historia creada"
    And la última entrada tiene actor="Reviewer" y contiene "RECHAZADO"

  Scenario: Activity Log vacío produce vector vacío
    Given el archivo "TASK-001.md" contiene "## Activity Log\n"
    When se invoca Task::load()
    Then task.activity_log es un vector vacío

  Scenario: Sin sección Activity Log produce vector vacío sin error
    Given el archivo "TASK-001.md" no contiene "## Activity Log"
    When se invoca Task::load()
    Then task.activity_log es un vector vacío
    And la carga no falla (es exitosa)

  # CA3: Task::set_status escribe preservando el resto del contenido
  Scenario: Escribir nuevo estado preservando el contenido intacto
    Given un Task cargado desde "TASK-001.md" con status "pending"
    And el contenido original tiene 50 líneas con las secciones Status, Priority y Descripción
    When se invoca Task::set_status("in_progress", section_markers)
    Then el archivo en disco tiene status "**in_progress**"
    And el archivo conserva exactamente las mismas líneas salvo la del status
    And el número total de líneas del archivo es 50

  Scenario: Restaurar desde .bak si el parseo post-escritura falla
    Given un Task cargado desde "TASK-001.md"
    When se invoca Task::set_status() pero la escritura produce un archivo corrupto
    Then el archivo original se restaura desde "TASK-001.md.bak"
    And devuelve Err con mensaje descriptivo
    And el archivo .bak se elimina tras la restauración

  Scenario: Eliminar .bak tras escritura exitosa
    Given un Task cargado desde "TASK-001.md"
    When se invoca Task::set_status() exitosamente
    Then el archivo "TASK-001.md.bak" no existe
    And el status en memoria del Task coincide con el escrito a disco

  Scenario: set_status falla si el section_marker de status no existe en el archivo
    Given un Task cargado desde "TASK-001.md" sin sección "## Status"
    And el section_markers mapea "status" → "## Status"
    When se invoca Task::set_status("done", section_markers)
    Then devuelve Err con mensaje que contiene "no se encontró la sección"

  # ── Escenario de integridad cross-layer (domain puro) ────────────────

  Scenario: El módulo domain/task.rs no importa ninguna otra capa del crate
    Given el código fuente de domain/task.rs
    Then no contiene "use crate::app::"
    And no contiene "use crate::infra::"
    And no contiene "use crate::cli::"
    And no contiene "use crate::config::"
    And solo importa std, crates externos (regex, serde) y otros módulos de domain/
