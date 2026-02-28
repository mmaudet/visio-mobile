# Visio Mobile

Open-source native video conferencing client for [La Suite Meet](https://visio.numerique.gouv.fr), built on the [LiveKit Rust SDK](https://github.com/livekit/rust-sdks).

## Platforms

- **Android** — Kotlin + Jetpack Compose
- **iOS** — Swift + SwiftUI
- **Desktop** — Tauri 2.x

## Architecture

Shared Rust core (`visio-core`) handles room management, chat, participants, and media controls. Platform-specific UI shells consume the core via UniFFI bindings (control plane) and raw C FFI (video rendering).

See `docs/plans/2026-03-01-v2-rewrite-design.md` for full architecture documentation.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
