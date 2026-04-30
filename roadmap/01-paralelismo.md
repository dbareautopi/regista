# 01 — Paralelismo

## 🎯 Objetivo

Ejecutar múltiples historias **independientes** de forma simultánea, en lugar
del procesamiento secuencial actual (una historia por iteración).

## ❓ Problema actual

El loop principal en `orchestrator.rs` procesa **una sola historia por iteración**.
En un backlog de 20+ historias, con agentes LLM que pueden tardar de 2 a 10
minutos cada uno, el pipeline completo puede llevar **horas**. Muchas de esas
historias no tienen dependencias entre sí y podrían avanzar en paralelo.

## ✅ Solución propuesta

### Modelo de concurrencia

Usar `tokio` (o `std::thread`) para lanzar múltiples invocaciones de `pi`
simultáneamente, respetando:

1. **Grafo de dependencias**: solo se paralelizan historias sin relación de
   dependencia (ni directa ni transitiva).
2. **Límite configurable**: `max_concurrent_agents = 4` en `[limits]`.
3. **Recursos compartidos**: el filesystem es el estado compartido. Dos agentes
   no deben tocar la misma historia simultáneamente.

### Algoritmo

```
1. Construir grafo de dependencias
2. Encontrar todas las historias accionables
3. Agrupar por "componentes independientes" (particiones del grafo)
4. Para cada grupo independiente, lanzar un agente en paralelo
5. Esperar a que todos terminen (con timeout)
6. Re-evaluar estados y repetir
```

### Riesgos

- **Conflictos de archivos**: dos agentes podrían modificar el mismo archivo
  fuente. Mitigación: ejecutar en branches separados y mergear al final.
- **Rate limiting de LLM**: el proveedor puede rechazar llamadas simultáneas.
  El backoff exponencial ya existente ayuda.
- **Determinismo**: el orden de ejecución secuencial garantizaba
  reproducibilidad. Con paralelismo, dos ejecuciones pueden dar resultados
  distintos.

## 📝 Notas de implementación

- Agregar `tokio` como dependencia (feature `rt-multi-thread` + `process`).
- `agent::invoke_with_retry` debe ser `async`.
- `orchestrator::run` se convierte en `async` y usa `tokio::spawn` por historia.
- Nuevo campo en `LimitsConfig`: `max_concurrent_agents: u32` (default 1,
  manteniendo compatibilidad hacia atrás).
- El grafo de dependencias debe exponer un método `independent_groups()`
  que retorne Vec<Vec<Story>> con particiones procesables en paralelo.

## 🔗 Relacionado con

- [`07-checkpoint-resume.md`](./07-checkpoint-resume.md) — el checkpoint se
  vuelve más importante con concurrencia.
- [`12-cost-tracking.md`](./12-cost-tracking.md) — paralelismo = más gasto
  simultáneo.
