# language: es
@app @decomposition @STORY-V10-018

Feature: Fase de descomposición — generar tareas desde input vía run --plan-only
  Como usuario que quiere arrancar un pipeline desde cero,
  quiero ejecutar regista run --plan-only spec.md para generar tareas desde un input,
  para revisar el backlog antes de lanzar el pipeline completo.

  Background:
    Given un proyecto sin tareas en .regista/tasks/
    And el workflow define una fase de descomposición con from="_init_" y to="draft"
    And el task_format define id_pattern="TASK-\\d+" y campos "description", "priority"
    And un mock LlmProvider configurado para el rol de la fase de descomposición

  Scenario: run --plan-only genera tareas en el formato del task_format
    Given el mock devuelve contenido markdown con 3 tareas en formato TASK-NNN
    When se ejecuta regista run --plan-only spec.md
    Then se crean 3 archivos .md en .regista/tasks/: TASK-001.md, TASK-002.md, TASK-003.md
    And cada archivo cumple el id_pattern del task_format
    And cada archivo contiene las secciones definidas en section_markers
    And el pipeline se detiene sin ejecutar el resto de fases

  Scenario: run sin --plan-only encadena descomposición + pipeline
    Given el mock genera 2 tareas en la fase de descomposición
    And el mock del pipeline responde [STATUS: done] para cada tarea
    When se ejecuta regista run spec.md
    Then se generan 2 tareas y se ejecuta el pipeline sobre ellas
    And ambas tareas terminan en estado "done"

  Scenario: Bucle de validación corrige dependencias incorrectas
    Given el mock genera TASK-003 con dependencia "TASK-999" (inexistente)
    When se ejecuta el bucle de validación (plan_max_iterations=3)
    Then en la iteración 2, el prompt incluye feedback: "TASK-003 depende de TASK-999 que no existe"
    And en la iteración 3, TASK-003 ya no tiene la dependencia rota (o se ha creado TASK-999)
