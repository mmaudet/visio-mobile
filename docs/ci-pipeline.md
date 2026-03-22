# CI/CD Pipeline

Visio Mobile uses GitHub Actions to enforce code quality, security,
and correctness on every pull request and push to `main`.

> This pipeline is heavily inspired by the CI/CD practices of
> [La Suite Meet](https://github.com/suitenumerique/meet) by
> [DINUM](https://www.numerique.gouv.fr/), the French government's
> open-source video conferencing platform. We share the same
> commitment to code quality and open-source best practices.
> See their [contributing guide](https://github.com/suitenumerique/meet/blob/main/docs/developping_locally.md)
> and [CI workflows](https://github.com/suitenumerique/meet/tree/main/.github/workflows)
> for reference.

## Pipeline overview

```
PR opened / push to main
    |
    +-- CI workflow (.github/workflows/ci.yml)
    |     +-- lint-git          Commit message conventions
    |     +-- check-changelog   CHANGELOG.md updated
    |     +-- lint-changelog    CHANGELOG line length <= 80
    |     +-- lint-rust         cargo fmt + clippy
    |     +-- lint-kotlin       ktlint
    |     +-- lint-frontend     ESLint + Prettier
    |     +-- test-rust         Unit + integration tests
    |     +-- test-android-ui   Android emulator tests (manual)
    |
    +-- SonarCloud (.github/workflows/sonarcloud.yml)
    |     +-- Code analysis     Bugs, smells, complexity, coverage
    |
    +-- Security (.github/workflows/security.yml)
          +-- GitGuardian       Secret detection
          +-- Trivy (fs)        Rust dependency vulnerabilities
          +-- Trivy (Docker)    Container image scan (push only)
```

## CI checks

### lint-git

**Tool:** [gitlint](https://jorisroovers.com/gitlint/)

Validates commit messages follow conventions:
- Title <= 72 characters (T1)
- Body is present (B6)
- Body lines <= 80 characters (B1)
- Conventional commit format: `type(scope): description`

*Runs on: pull requests only*

### check-changelog

Verifies that `CHANGELOG.md` was modified in the PR. This
ensures every user-facing change is documented.

Skip by adding the `noChangeLog` label to the PR.

*Runs on: pull requests only*

### lint-changelog

Checks that no line in `CHANGELOG.md` exceeds 80 characters
(excluding URL reference links). Keeps the changelog readable
in terminals and code review tools.

### lint-rust

**Tools:** [rustfmt](https://github.com/rust-lang/rustfmt) +
[Clippy](https://github.com/rust-lang/rust-clippy)

- `cargo fmt -p visio-core -- --check` — Enforces standard
  Rust formatting
- `cargo clippy -p visio-core -- -D warnings` — Static
  analysis catching common mistakes, performance issues,
  and idiomatic Rust violations. All warnings are errors.

### lint-kotlin

**Tool:** [ktlint](https://pinterest.github.io/ktlint/)
(via Gradle plugin `org.jlleitschuh.gradle.ktlint`)

Enforces Kotlin coding style in the Android app. Checks
indentation, spacing, import ordering, and naming conventions.

Run locally: `cd android && ./gradlew ktlintFormat`

### lint-frontend

**Tools:** [ESLint](https://eslint.org/) +
[Prettier](https://prettier.io/)

Two-step check for the Desktop React/TypeScript frontend:
1. **ESLint** — Catches bugs, accessibility issues (S6847,
   S6853), unused variables, and React anti-patterns
2. **Prettier** — Enforces consistent code formatting
   (indentation, quotes, line length)

Run locally:
```bash
cd crates/visio-desktop/frontend
npx eslint src/
npx prettier --write "src/**/*.{ts,tsx,css}"
```

### test-rust

**Tool:** `cargo test`

Runs the full Rust test suite against a local LiveKit server:
1. **Unit tests** (`cargo test -p visio-core --lib`) — 148+
   tests covering room management, auth, chat, settings,
   event handling, and adaptive mode logic
2. **Integration tests** (`cargo test -p visio-core --test
   integration_livekit`) — End-to-end tests against a real
   LiveKit server (installed automatically in CI)

The CI step installs `livekit-server --dev` and runs it in
the background before executing tests.

### test-android-ui

**Tool:** Android emulator +
[Gradle connectedAndroidTest](https://developer.android.com/studio/test)

Runs instrumented UI tests on an Android emulator (API 34,
x86_64). Currently triggered only on `workflow_dispatch`
(manual) due to emulator instability in CI.

## Security checks

### GitGuardian

**Tool:** [GitGuardian](https://www.gitguardian.com/)

Scans every commit for accidentally leaked secrets:
API keys, tokens, passwords, private keys, and credentials.
Runs on both PRs and pushes. A detection blocks the PR.

### Trivy (Rust dependencies)

**Tool:** [Trivy](https://trivy.dev/) by Aqua Security

Filesystem scan of all Rust dependencies (`Cargo.lock`)
for known vulnerabilities (CVEs). Only `CRITICAL` and `HIGH`
severity issues cause a failure. Unfixed vulnerabilities are
ignored to avoid blocking on upstream issues.

### Trivy (Docker images)

**Tool:** [Trivy](https://trivy.dev/)

Builds Docker images for Android and Desktop, then scans
them for OS-level and dependency vulnerabilities. Runs on
push to `main` only (not on PRs).

## SonarCloud

**Tool:** [SonarCloud](https://sonarcloud.io/)

Continuous code quality inspection covering:
- **Reliability** — Bugs and runtime errors
- **Maintainability** — Code smells, cognitive complexity,
  duplication
- **Security** — Vulnerability detection

SonarCloud runs on every PR and push. Results are visible at:
https://sonarcloud.io/summary/overall?id=mmaudet_visio-mobile

### Quality gate

The quality gate checks new code for:
- No new bugs
- No new vulnerabilities
- Maintainability rating A
- No new security hotspots

> SonarCloud is configured with `continue-on-error: true`
> so it does not block merges, but quality gate failures
> are visible in the PR checks.

## Running checks locally

```bash
# Rust formatting + linting
cargo fmt -p visio-core
cargo clippy -p visio-core -- -D warnings

# Rust tests (requires livekit-server --dev running)
cargo test -p visio-core

# Kotlin formatting
cd android && ./gradlew ktlintFormat

# Frontend formatting + linting
cd crates/visio-desktop/frontend
npx eslint src/
npx prettier --write "src/**/*.{ts,tsx,css}"

# Changelog line length
awk 'length > 80' CHANGELOG.md
```
