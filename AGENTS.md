# AGENTS.md — Visio Mobile Codebase Guide

Cross-platform video conferencing client (Android, iOS, Linux, macOS, Windows) built with
Rust + Tauri 2 (desktop) and Rust + UniFFI (mobile). The TypeScript/React frontend lives
exclusively in `crates/visio-desktop/frontend/`.

---

## Repository Structure

```
crates/
  visio-core/       — Pure Rust business logic (no platform deps)
  visio-desktop/    — Tauri 2 app (Rust backend + React/TS frontend)
  visio-ffi/        — UniFFI bindings for Android/iOS
  visio-video/      — Platform video rendering
e2e/visio-bot/      — Integration test bot (Rust)
android/            — Android Kotlin app
ios/                — iOS Swift app
i18n/               — Translation JSON files (en, fr, de, es, it, nl)
```

---

## Build & Run Commands

### Rust (all platforms)

```sh
# Build the entire workspace
cargo build

# Build a specific crate
cargo build -p visio-core
cargo build -p visio-desktop

# Run the Tauri desktop app (dev mode)
cargo tauri dev          # from crates/visio-desktop/
# or
cargo run -p visio-desktop
```

### Frontend (TypeScript/React)

```sh
# Working directory: crates/visio-desktop/frontend/
npm install
npm run dev       # Vite dev server
npm run build     # tsc + vite build (production)
npm run preview   # Preview production build
```

### Android

```sh
./gradlew assembleDebug          # Build APK
./gradlew connectedAndroidTest   # Run instrumented tests on device/emulator
```

---

## Test Commands

### Rust — Unit Tests

```sh
# Run all unit tests for a single crate (fastest, no integration deps)
cargo test -p visio-core --lib

# Run a single test by name (supports substring matching)
cargo test -p visio-core --lib test_name

# Run all workspace tests
cargo test

# Run tests with output shown
cargo test -p visio-core --lib -- --nocapture
```

### TypeScript — E2E Tests (Playwright)

```sh
# Working directory: crates/visio-desktop/frontend/
npx playwright test                  # Run all E2E tests (headless)
npx playwright test --ui             # Interactive Playwright UI
npx playwright test --headed         # Run with browser visible
npx playwright test path/to/test.ts  # Run a single test file
```

> There are no unit tests for the TypeScript frontend. All TS tests are Playwright E2E.
> The suite runs in CI on every PR to `main` (job `test-frontend`).

---

## Lint & Format

### Rust

```sh
# Format (check only — used in CI)
cargo fmt -p visio-core -- --check

# Format (apply)
cargo fmt

# Lint (CI uses -D warnings — all warnings are errors)
cargo clippy -p visio-core -- -D warnings

# Lint entire workspace
cargo clippy -- -D warnings
```

### Kotlin (Android)

```sh
./gradlew ktlintCheck   # Lint
./gradlew ktlintFormat  # Auto-fix
```

### TypeScript

```sh
# Working directory: crates/visio-desktop/frontend/
npm run lint          # ESLint — any warning fails the run (--max-warnings 0)
npm run format:check  # Prettier check
npx tsc --noEmit      # Type-check
```

---

## Code Style Guidelines

### Rust

**Formatting**
- `rustfmt` with `edition = "2024"` (see `rustfmt.toml`). Always run `cargo fmt` before committing.
- `clippy -D warnings` must pass. Address all clippy lints — do not `#[allow(...)]` unless truly necessary with a comment explaining why.

**Naming**
- `snake_case` for variables, functions, modules, fields.
- `PascalCase` for structs, enums, traits, type aliases.
- `SCREAMING_SNAKE_CASE` for constants and statics.
- Module filenames match the module name exactly.

**Error Handling**
- All public errors derive from `thiserror::Error` in `crates/visio-core/src/errors.rs`.
- Propagate with `?`; convert foreign errors with `.map_err(|e| VisioError::Variant(e.to_string()))`.
- FFI entry points (`visio-ffi`) must wrap all calls in `std::panic::catch_unwind` — panics must never cross the FFI boundary.
- Never use `.unwrap()` in production paths. Use `?`, `unwrap_or_else`, or explicit match.
- For poisoned sync `Mutex`: recover with `.unwrap_or_else(|p| p.into_inner())`.

**Async & Concurrency**
- Use `tokio` for all async work (`rt-multi-thread`, `macros`, `sync`, `time` features).
- Shared mutable state: `Arc<tokio::sync::Mutex<T>>` for async contexts; `Arc<std::sync::Mutex<T>>` for sync contexts.
- FFI must be synchronous — wrap async calls with `block_on(runtime.spawn(...))`.
- Background tasks: `tokio::spawn` with explicit `AbortHandle` stored for cleanup.

**Modules & Visibility**
- Declare submodules in `lib.rs` with `pub mod name;`.
- Re-export public API with `pub use name::Type;` from `lib.rs`.
- Keep `pub(crate)` for internal-only items.

**Tests**
- Unit tests live inline: `#[cfg(test)] mod tests { ... }` at the bottom of each file.
- Integration tests go in `tests/` directories or `e2e/`.
- Use descriptive test names: `test_<what>_<condition>_<expected>`.

**Workspace Dependencies**
- Declare shared dependencies once in the root `[workspace.dependencies]` table.
- Reference them in crate `Cargo.toml` with `dep = { workspace = true }`.
- Do not duplicate version pins across crates.

---

### TypeScript / React (Frontend — `crates/visio-desktop/frontend/`)

**Compiler**
- `strict: true` in `tsconfig.json` — no implicit `any`, strict null checks, etc.
- Target `ES2020`, module resolution `"bundler"` (Vite).
- `noEmit: true` — Vite handles output; `tsc` is type-check only.

**Naming**
- `camelCase` for variables, functions, hooks.
- `PascalCase` for React components, interfaces, and type aliases.
- `UPPER_SNAKE_CASE` for module-level constants.
- Hook names must start with `use`.

**Imports**
- Named imports only — no default-import-everything.
- No barrel (`index.ts`) files.
- Import order (manual, no linter): React/external packages → Tauri APIs → local modules → CSS.
- Translation JSON files are imported directly: `import en from "../../../../i18n/en.json"`.
- Icons from `@remixicon/react` imported individually by name.

**Components & State**
- Function components only — no class components.
- React hooks for all state (`useState`, `useRef`, `useEffect`, `useCallback`).
- No external state management library (Redux, Zustand, etc.) — keep state local or in context.
- Prefer `useCallback` for event handlers passed as props.
- Clean up `listen()` subscriptions by calling the returned `UnlistenFn` in `useEffect` cleanup.

**Tauri IPC**
- Frontend → backend: `invoke("command_name", { arg1, arg2 })` from `@tauri-apps/api/core`.
- Backend → frontend: `listen("event-name", handler)` from `@tauri-apps/api/event`.
- Command names and event names use `snake_case` strings matching the Rust `#[tauri::command]` functions.

**Error Handling**
- User-visible errors: `catch (e) { setError(String(e)); }` — always convert to string.
- Non-critical operations (e.g., clipboard): empty `catch {}` is acceptable; add a comment.
- Never silently swallow errors that affect user-visible state.

**i18n**
- All user-facing strings must go through the `useT()` hook (returns a `TFunction`).
- Add new keys to all locale files in `i18n/` simultaneously.

**Formatting**
- No formatter is configured. Follow the existing style: 2-space indentation, double quotes for JSX attributes, trailing commas in multi-line structures.
- Keep lines under ~100 characters where practical.

---

## CI Summary (`.github/workflows/ci.yml`)

| Job | Command | Trigger |
|---|---|---|
| `lint-git` | `gitlint` on PR commits | PR to `main` |
| `check-changelog` / `lint-changelog` | CHANGELOG updated, lines < 80 | PR to `main` |
| `lint-rust` | `cargo fmt --check` + `cargo clippy -D warnings` | PR to `main` |
| `lint-kotlin` | `./gradlew ktlintCheck` | PR to `main` |
| `lint-frontend` | `npm run lint` + `npm run format:check` | PR to `main` |
| `check-i18n` | `./scripts/check-i18n.sh` | PR to `main` |
| `test-rust` | `cargo test -p visio-core --lib` (+ LiveKit integration) | PR to `main` |
| `test-frontend` | `npx playwright test` | PR to `main` |
| `test-android-ui` | `./gradlew connectedAndroidTest` | `workflow_dispatch` only |

All CI jobs must pass before merging to `main`.
