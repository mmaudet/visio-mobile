# Contributing to Visio Mobile

We love contributions of any kind, big and small!

## Ways to contribute

- **Open a PR** — See [building locally](#building-locally) below
- **Submit a [feature request](https://github.com/mmaudet/visio-mobile/issues/new?labels=enhancement)**
- **Report a [bug](https://github.com/mmaudet/visio-mobile/issues/new?labels=bug)**
- **Vote on features** in our [issues](https://github.com/mmaudet/visio-mobile/issues)

## Building locally

Follow the build instructions in the [README](README.md#building) for your platform:

- [Desktop (macOS / Linux / Windows)](README.md#desktop-macos--linux--windows)
- [Android](README.md#android)
- [iOS](README.md#ios)

## Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description

feat(android): add audio route picker
fix(core): handle reconnection timeout
refactor(desktop): extract settings component
docs: update build instructions
```

Types: `feat`, `fix`, `refactor`, `docs`, `style`, `test`, `chore`

## Code quality

All PRs must pass CI checks before merging.
See [docs/ci-pipeline.md](docs/ci-pipeline.md) for a full
description of each check.

- **lint-rust** — `cargo clippy` + `cargo fmt`
- **lint-kotlin** — ktlint
- **lint-frontend** — ESLint + Prettier
- **test-rust** — `cargo test -p visio-core`
- **check-changelog** — CHANGELOG.md must be updated
- **lint-git** — Commit messages must follow conventions
- **SonarCloud** — Code quality and maintainability
- **GitGuardian** — Secret detection
- **Trivy** — Dependency vulnerability scanning

Run locally before pushing:

```bash
# Rust
cargo fmt && cargo clippy -p visio-core -p visio-ffi -p visio-desktop

# Android
cd android && ./gradlew ktlintFormat

# Desktop frontend
cd crates/visio-desktop/frontend && npx prettier --write src/
```

## Pull requests

1. Fork the repo and create your branch from `main`
2. Make your changes with tests if applicable
3. Update `CHANGELOG.md` under `[Unreleased]`
4. Ensure CI passes
5. Open a PR with a clear description

## License

By contributing, you agree that your contributions will be licensed under the [AGPL-3.0 License](LICENSE).
