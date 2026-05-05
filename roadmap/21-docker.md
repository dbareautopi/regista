# 21 — Dockerización de regista

> 💡 **IDEA** — pendiente de refinamiento y diseño detallado.

## 🎯 Objetivo

Que un proyecto nuevo pueda arrancar regista en Docker con un solo comando,
sin instalar nada localmente. El usuario hace `regista init --docker`, elige
provider, y tras un `docker compose up` el pipeline está corriendo.

## ❓ Problema actual

1. Hay que instalar `regista` localmente (Rust + `cargo install` o binario).
2. Hay que instalar el provider CLI también (pi, claude, codex, opencode).
3. Cada provider tiene sus propias dependencias de sistema (Node, Python, etc.).
4. CI/CD requiere configurar el entorno manualmente en cada runner.
5. Diferentes máquinas = diferentes entornos = comportamientos inconsistentes.

## 💡 Idea general

```bash
regista init --docker --provider pi
```

Además del scaffolding normal (`.regista/config.toml`, skills, etc.), genera:

```
mi-proyecto/
├── .regista/
│   └── ...
├── Dockerfile                     ← Imagen con regista + provider
├── docker-compose.yml             ← Orquestación lista para usar
├── .dockerignore                  ← Excluir target/, node_modules/, etc.
└── .env.example                   ← Variables de entorno necesarias (API keys)
```

El usuario luego hace:

```bash
docker compose up
```

Y el pipeline arranca dentro del container.

## 🔧 Diseño preliminar

### Dockerfile (una imagen por provider)

La imagen debe contener:
1. El binario de `regista`
2. El provider CLI correspondiente (`pi`, `claude`, `codex`, `opencode`)
3. Las dependencias de sistema necesarias para ese provider

```
# Ejemplo conceptual para --provider pi
FROM node:22-alpine AS pi
RUN npm install -g @mariozechner/pi-coding-agent

FROM alpine:3.20
COPY --from=pi /usr/local/lib/node_modules /usr/local/lib/node_modules
COPY --from=pi /usr/local/bin/pi /usr/local/bin/pi
COPY regista /usr/local/bin/regista
ENTRYPOINT ["regista"]
```

### docker-compose.yml

```yaml
version: "3.8"
services:
  regista:
    build: .
    volumes:
      - .:/workspace           # Código + historias
      - ./.regista:/workspace/.regista
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    working_dir: /workspace
    command: run               # O plan + run, configurable
```

### Provider por provider

| Provider | Dependencias | Imagen base sugerida | Peso estimado |
|----------|-------------|---------------------|---------------|
| `pi` | Node.js 22+ | `node:22-alpine` | ~200 MB |
| `claude` | Node.js 22+ | `node:22-alpine` | ~200 MB |
| `codex` | Node.js 22+ | `node:22-alpine` | ~200 MB |
| `opencode` | Rust / binary | `alpine:3.20` | ~80 MB |

> ⚠️ Todos los providers basados en Node comparten dependencias — se podría
> hacer una imagen multi-provider si el usuario necesita varios.

## ❓ Decisiones pendientes

Estas preguntas necesitan respuesta antes de implementar. Por ahora quedan
documentadas como **decisiones a tomar en la fase de refinamiento**.

### 1. ¿Imagen pre-built o compilación en el momento?

| Opción A: Imagen oficial | Opción B: Multi-stage build |
|---|---|
| `FROM ghcr.io/.../regista:latest` | `FROM rust → cargo build` dentro del Dockerfile |
| Necesita CI que publique la imagen | Autosuficiente, no depende de registry externo |
| Rápido (solo descarga) | Lento la primera vez (compila todo) |
| Requiere mantener un workflow de publicación | Siempre build reproducible |

### 2. ¿Un Dockerfile genérico parametrizable o uno por provider?

- **ARG + target**: `docker build --build-arg PROVIDER=pi` — más DRY, un solo archivo.
- **Un Dockerfile por provider**: `Dockerfile.pi`, `Dockerfile.claude` — más simple,
  cada uno auto-contenido y legible.
- El `docker-compose.yml` puede referenciar el correcto según config.

### 3. ¿Qué comando ejecuta el container al arrancar?

- `regista run` — pipeline con historias ya existentes.
- `regista plan spec.md && regista run` — flujo completo desde spec.
- `regista run --resume` — continuar desde checkpoint.
- **Configurable** vía `command:` en docker-compose o variable `REGISTA_CMD`.

### 4. ¿Cómo se manejan las credenciales?

- Variables de entorno en `.env` (`.env.example` generado, `.env` en `.gitignore`).
- Volumen para montar archivos de credenciales (`~/.pi/credentials`, etc.).
- ¿Soportar secretos de Docker Swarm / Kubernetes?

### 5. ¿Comportamiento con `regista plan`?

Si el usuario quiere el flujo completo (plan + run):
- ¿El container ejecuta `plan` y luego `run` secuencialmente?
- ¿Son dos servicios separados en docker-compose?
- ¿Un script entrypoint que decide según estado de `stories/`?

### 6. ¿Multi-provider en el mismo proyecto?

Un usuario puede querer PO con Claude y Dev con pi. ¿La imagen generada
incluye ambos providers? ¿O solo el especificado en `--provider` y luego
el usuario extiende el Dockerfile?

### 7. ¿Modo daemon en Docker?

Actualmente `--detach` spawnea un proceso hijo. En Docker, el propio container
es el daemon. ¿Adaptar `--detach` para entorno Docker o no es necesario?

### 8. ¿Qué pasa con los hooks?

Los hooks (`post_qa`, `post_dev`, `post_reviewer`) ejecutan comandos del stack
(`cargo build`, `npm test`, etc.). En Docker:
- ¿El container necesita el toolchain completo (Rust, Node, Python)?
- ¿O los hooks se ejecutan en el host y el container solo orquesta agentes?
- Esto es **crítico**: si el container necesita compilar el proyecto, la imagen
  puede crecer mucho.

### 9. ¿Persistencia de decisiones y logs?

Con checkpoint/resume funcionando, si el container se detiene:
- ¿Se pierde el estado? → Volumen para `.regista/`
- ¿Las decisiones en `decisions/` sobreviven? → Ya están en volumen (código)

## 📐 Esfuerzo estimado

| Tarea | Esfuerzo |
|-------|----------|
| Diseño detallado (responder preguntas ↑) | Medio (1-2 sesiones) |
| Templates de Dockerfile (1 por provider, 4 total) | Bajo (~40 líneas cada uno) |
| Template de docker-compose.yml | Bajo (~20 líneas) |
| `.dockerignore` + `.env.example` | Trivial (~10 líneas) |
| Integración en `init.rs` (flag `--docker`) | Bajo (~30 líneas) |
| Tests | Bajo (~5 tests) |
| Documentación en README | Bajo |
| **Total** | **Medio-bajo** (~150-200 líneas) |

## 🔗 Relacionado con

- [`06-init-scaffold.md`](./06-init-scaffold.md) — el flag `--docker` se integra aquí.
- [`20-multi-provider.md`](./20-multi-provider.md) — cada provider tiene su propio Dockerfile.
- [`07-checkpoint-resume.md`](./07-checkpoint-resume.md) — el estado debe persistir vía volúmenes.
- [`04-workflow-configurable.md`](./04-workflow-configurable.md) — si hay workflow custom, docker-compose debería reflejarlo.
- [`specs/spec-logs-transparentes.md`](../specs/spec-logs-transparentes.md) — logs del container visibles para el usuario.

## 📅 Estado

- **2026-05-05**: Idea documentada. Pendiente de refinamiento. No asignada a fase.
