# language: es
@domain @templates @STORY-V10-008

Feature: Sistema de templates para prompts
  Como definidor de fases de workflow,
  quiero usar {{variables}} en los prompts que se sustituyen en runtime,
  para que los prompts referencien dinámicamente campos de la task sin hardcodearlos.

  Background:
    Given un Task con:
      | id          | TASK-005                          |
      | status      | pending                           |
      | fields      | {"priority": "high", "topic": "IA generativa", "effort": "5"} |
      | blockers    | ["TASK-002", "TASK-003"]          |
    And una entrada en activity_log: "2026-05-08 | reviewer | rechazado: tests no compilan"
    And un context con {"fecha_limite": "2026-06-01", "asignado_a": "Alice"}

  # ── Capa: Dominio (domain/templates.rs) — render_template() ──────────

  # CA1: Sustitución de variables básicas
  Scenario: Sustituir {{task_id}} y {{task_status}}
    Given el template "Procesa {{task_id}} con estado {{task_status}}"
    When se invoca render_template(template, task, context)
    Then el resultado es "Procesa TASK-005 con estado pending"

  Scenario: Sustituir {{last_rejection}}
    Given la última entrada del activity_log contiene "rechazado"
    And el template es "Corrige: {{last_rejection}}"
    When se invoca render_template()
    Then el resultado contiene "rechazado: tests no compilan"

  Scenario: {{last_rejection}} sin rechazos previos
    Given el activity_log no contiene ninguna línea con "rechaz"
    And el template es "{{last_rejection}}"
    When se invoca render_template()
    Then el resultado es "(sin rechazos previos)"

  Scenario: Sustituir {{blockers}} como lista
    Given el template es "Depende de:\n{{blockers}}"
    When se invoca render_template()
    Then el resultado contiene "- TASK-002"
    And el resultado contiene "- TASK-003"
    And cada blocker aparece en una línea separada con prefijo "- "

  Scenario: {{blockers}} cuando no hay dependencias
    Given task.blockers es un vector vacío
    And el template es "{{blockers}}"
    When se invoca render_template()
    Then el resultado es "(sin dependencias)"

  # CA1: Sustitución de campos dinámicos
  Scenario: Sustituir campo concreto con {{task_fields.<campo>}}
    Given el template es "Prioridad: {{task_fields.priority}}"
    When se invoca render_template()
    Then el resultado es "Prioridad: high"

  Scenario: Sustituir otro campo concreto
    Given el template es "Tema: {{task_fields.topic}} — Esfuerzo: {{task_fields.effort}} días"
    When se invoca render_template()
    Then el resultado es "Tema: IA generativa — Esfuerzo: 5 días"

  Scenario: Sustituir campos con {{task_fields.*}} como bullet list
    Given el template es "Campos de la tarea:\n{{task_fields.*}}"
    When se invoca render_template()
    Then el resultado contiene "- priority: high"
    And el resultado contiene "- topic: IA generativa"
    And el resultado contiene "- effort: 5"
    And los campos aparecen en orden alfabético

  Scenario: {{task_fields.*}} con task sin campos extra
    Given task.fields es un HashMap vacío
    And el template es "{{task_fields.*}}"
    When se invoca render_template()
    Then el resultado es "(sin campos adicionales)"

  # CA1: Sustitución de contexto
  Scenario: Sustituir {{context.<clave>}}
    Given el template es "Fecha límite: {{context.fecha_limite}} — Asignado a: {{context.asignado_a}}"
    When se invoca render_template()
    Then el resultado es "Fecha límite: 2026-06-01 — Asignado a: Alice"

  Scenario: {{context.<clave>}} con clave no definida
    Given el template es "{{context.inexistente}}"
    When se invoca render_template()
    Then el resultado es "(no definido)"
    And se emite un warning: "variable de contexto 'inexistente' no encontrada"

  Scenario: Combinación de todas las variables en un solo template
    Given el template es:
      """
      Tarea: {{task_id}} [{{task_status}}]
      Prioridad: {{task_fields.priority}}
      Bloqueada por: {{blockers}}
      Último rechazo: {{last_rejection}}
      Deadline: {{context.fecha_limite}}
      """
    When se invoca render_template()
    Then el resultado contiene "Tarea: TASK-005 [pending]"
    And el resultado contiene "Prioridad: high"
    And el resultado contiene "TASK-002"
    And el resultado contiene "rechazado: tests no compilan"
    And el resultado contiene "Deadline: 2026-06-01"

  # CA3: Variables no definidas no rompen el renderizado
  Scenario: Campo no definido se sustituye por placeholder
    Given el template es "{{task_fields.priority}} {{task_fields.inexistente}}"
    When se invoca render_template()
    Then el resultado es "high (no definido)"
    And se emite un warning: "campo 'inexistente' no encontrado en la task"

  Scenario: Variable inventada fuera de las conocidas
    Given el template es "{{variable_inventada}}"
    When se invoca render_template()
    Then el resultado es "(no definido)"
    And se emite un warning con el nombre de la variable no reconocida

  Scenario: Template sin variables se devuelve intacto
    Given el template es "Este prompt no tiene variables"
    When se invoca render_template()
    Then el resultado es exactamente "Este prompt no tiene variables"
    And no se emite ningún warning

  Scenario: Template con {{ repetido pero no cerrado
    Given el template es "{{task_id y más texto sin cerrar"
    When se invoca render_template()
    Then el texto "{{task_id" se conserva literal (no se interpreta como variable)
    And se emite un warning: "variable '{{task_id' no tiene cierre }}"

  # CA2: Renderizado de system prompt desde RoleConfig
  Scenario: System prompt de rol se renderiza con variables de task
    Given un RoleConfig con system_prompt = "Eres {{role_name}}. Procesa {{task_id}}"
    And el role_name es "QA Engineer"
    When se renderiza el system prompt con la task del Background
    Then el resultado es "Eres QA Engineer. Procesa TASK-005"

  Scenario: System prompt soporta task_fields en el renderizado
    Given un RoleConfig con system_prompt = "Rol: {{role_name}}. Prioridad de la tarea: {{task_fields.priority}}"
    And el role_name es "Developer"
    When se renderiza el system prompt
    Then el resultado contiene "Rol: Developer"
    And el resultado contiene "Prioridad de la tarea: high"

  Scenario: System prompt usa {{role_name}} sin task
    Given un RoleConfig con system_prompt = "Eres {{role_name}}"
    And el role_name es "Reviewer"
    When se renderiza el system prompt (sin task asociada)
    Then el resultado es "Eres Reviewer"
    And las variables de task ({{task_id}}, etc.) se sustituyen por "(sin tarea)"

  Scenario: System prompt sin variables se devuelve intacto
    Given un RoleConfig con system_prompt = "Eres un asistente útil."
    When se renderiza el system prompt
    Then el resultado es "Eres un asistente útil."

  # ── Integridad cross-layer ──────────────────────────────────────────

  Scenario: domain/templates.rs no importa otras capas del crate
    Given el código fuente de domain/templates.rs
    Then no contiene "use crate::app::"
    And no contiene "use crate::infra::"
    And no contiene "use crate::cli::"
    And no contiene "use crate::config::"
    And render_template() recibe &Task y &HashMap<String,String> sin conocer su origen

  Scenario: render_template es una función pura
    Given los mismos parámetros (template, task, context)
    When se invoca render_template() múltiples veces
    Then devuelve exactamente el mismo resultado cada vez
    And no tiene efectos secundarios (no escribe archivos, no llama a red)
