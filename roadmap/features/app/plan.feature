# language: es
@app @plan @STORY-V10-018

Feature: Generación de backlog desde especificación con LLM nativo
  Como product owner,
  quiero ejecutar regista plan spec.md para generar tareas desde una especificación,
  para arrancar un pipeline nuevo sin escribir tareas manualmente.

  Background:
    Given un archivo spec.md con la descripción del proyecto
    And el workflow define task_format con id_pattern="TASK-\\d+" y campos "description", "priority"
    And un mock LlmProvider configurado para el rol "product_owner"

  Scenario: El plan genera tareas en el formato del task_format
    Given el mock devuelve contenido markdown con 3 tareas en formato TASK-NNN
    When se ejecuta regista plan spec.md
    Then se crean 3 archivos .md en .regista/tasks/: TASK-001.md, TASK-002.md, TASK-003.md
    And cada archivo cumple el id_pattern del task_format
    And cada archivo contiene las secciones definidas en section_markers

  Scenario: Bucle plan→validate corrige dependencias incorrectas
    Given el mock genera TASK-003 con dependencia "TASK-999" (inexistente)
    When se ejecuta el bucle de validación (plan_max_iterations=3)
    Then en la iteración 2, el prompt incluye feedback: "TASK-003 depende de TASK-999 que no existe"
    And en la iteración 3, TASK-003 ya no tiene la dependencia rota (o se ha creado TASK-999)

  Scenario: --max-stories limita el número de tareas generadas
    Given el mock intenta generar 10 tareas
    When se ejecuta regista plan spec.md --max-stories 5
    Then solo se crean 5 archivos de tarea
    And se ignora el contenido sobrante de la respuesta del LLM
