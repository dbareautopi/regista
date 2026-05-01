---
name: release
description: Automates the release workflow for regista — version bump, changelog update, git tag, merge to main, and cargo publish. Use when asked to publish a new version of regista.
---

# Release Skill for regista

Eres un desarrollador trabajando en el repo `regista`
(`github.com/dbareautopi/regista`). Esta skill describe el flujo completo
para subir cambios y publicar una nueva versión.

---

## Ramas

| Rama | Propósito |
|------|-----------|
| `develop` | Desarrollo activo. Todos los cambios empiezan aquí. |
| `main` | Producción. Solo se actualiza vía merge desde `develop`. |

---

## Flujo completo de release

Cuando se te pida publicar una nueva versión, recibirás el número de versión
en el prompt (ej: `0.3.3`, `0.4.0`, `1.0.0`). Sigue estos pasos **en orden**:

### 1. Verificar estado

```bash
git branch --show-current   # debe ser develop
git status --short           # debe estar limpio o con cambios a commitear
```

### 2. Hacer los cambios necesarios

Implementa el fix/feature en el código fuente según corresponda.

### 3. Actualizar `Cargo.toml`

```toml
version = "X.Y.Z"    # la versión indicada en el prompt
```

### 4. Actualizar `CHANGELOG.md`

Añadir entrada al principio del archivo con este formato exacto:

```markdown
## [X.Y.Z] — YYYY-MM-DD

### Added / Changed / Fixed
- Descripción concisa del cambio
```

Y añadir el link de comparación al final:

```markdown
[X.Y.Z]: https://github.com/dbareautopi/regista/compare/vX.Y.(Z-1)...vX.Y.Z
```

### 5. Commit en `develop`

```bash
git add -A
git commit -m "tipo: descripción breve del cambio"
```

Tipos de commit válidos: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.

### 6. Push de `develop`

```bash
git push origin develop
```

### 7. Merge a `main`

```bash
git checkout main
git merge develop
```

### 8. Crear tag anotado

```bash
git tag -a vX.Y.Z -m "vX.Y.Z: descripción breve"
```

### 9. Push de `main` + tags

```bash
git push origin main --tags
```

Si el tag ya existe en el remote y necesita reubicarse:

```bash
git push origin :vX.Y.Z    # borrar tag remoto
git push origin vX.Y.Z     # pushear el nuevo
```

### 10. Publicar en crates.io

```bash
cargo publish
```

Si `Cargo.lock` tiene cambios sin commitear, haz commit antes:

```bash
git add Cargo.lock
git commit -m "chore: update Cargo.lock for X.Y.Z"
git push origin main
git tag -d vX.Y.Z && git tag -a vX.Y.Z -m "vX.Y.Z: descripción"
git push origin :vX.Y.Z && git push origin vX.Y.Z
cargo publish
```

### 11. Volver a `develop`

```bash
git checkout develop
```

---

## Tests y calidad

Antes de publicar, verifica **siempre**:

```bash
cargo test              # 128 tests, 0 fallos
cargo clippy -- -D warnings  # 0 warnings
cargo fmt -- --check    # código formateado
```

Si hay fallos, corrígelos antes de continuar con el release.

---

## Notas importantes

- **Nunca** hagas commit directo en `main`. Siempre vía merge desde `develop`.
- **Nunca** publiques sin haber pasado tests + clippy.
- El tag **siempre** es anotado (`-a`), no ligero.
- La versión en `Cargo.toml` y el tag deben coincidir exactamente.
- Si `cargo publish` falla por `Cargo.lock` sucio, commitea el lock y reubica el tag al nuevo commit.
- Si el tag ya existe remotamente, bórralo antes de pushear el nuevo.
