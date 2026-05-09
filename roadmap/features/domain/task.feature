# language: es
@domain @task @STORY-V10-006

Feature: Task genérico con parseo configurable
  Como orquestador de tareas,
  quiero poder parsear archivos .md con formato configurable,
  para que regista soporte cualquier dominio sin hardcodear STORY-NNN.

  Background:
    Given que existe un archivo "TASK-001.md" en el directorio de tareas
    And el workflow define task_format con:
      | campo             | valor                        |
      | id_pattern        | TASK-\d+                     |
      | section_markers   | {"status":"## Status"}       |
      | dependency_marker | Bloqueado por:               |

  Scenario: Parsear campos definidos en section_markers
    Given el archivo "TASK-001.md" contiene "## Status\n**pending**"
    When se invoca Task::load() con el task_format configurado
    Then el Task resultante tiene id "TASK-001"
    And task.fields["status"] es "pending"

  Scenario: Extraer dependencias con dependency_marker configurable
    Given el archivo "TASK-001.md" contiene "## Dependencias\n- Bloqueado por: TASK-002, TASK-003"
    When se invoca Task::load()
    Then task.blockers contiene ["TASK-002", "TASK-003"]

  Scenario: Escribir estado preservando el resto del contenido
    Given un Task cargado desde "TASK-001.md" con status "pending"
    When se invoca Task::set_status("in_progress")
    Then el archivo en disco tiene status "in_progress"
    And el resto del contenido del archivo permanece idéntico
    And si el parseo post-escritura falla, el archivo se restaura desde .bak
