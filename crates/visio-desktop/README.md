# visio-desktop

Tauri 2 desktop shell for Visio Mobile.

## Cargo features

| feature           | default | what it gates                                                 |
|-------------------|---------|---------------------------------------------------------------|
| `oidc`            | yes     | OIDC / ProConnect login flow (PKCE, token refresh, account-scoped commands, room creation, access management). |
| `pipewire-camera` | no      | Linux PipeWire camera capture (xdg-portal).                   |

## Build variants

### OIDC-enabled (default)

The shipping build with ProConnect / OIDC login, "Nouvelle réunion" room
creation from the authenticated account, and access management:

```sh
cargo tauri build
# or, for a quick syntactic check:
cargo check
```

### Anonymous-only (classic Jitsi-style)

Builds without the OIDC code paths. The frontend hides the "Nouvelle réunion"
sidebar button and the "Gérer le compte" Settings button; the home anonymous
"join by code" flow is the only entry point. `is_oidc_enabled` returns
`false` and `get_session_state` always returns `{ "state": "anonymous" }` so
the existing frontend keeps working unchanged.

```sh
cargo tauri build --no-default-features
# or, for a quick syntactic check:
cargo check --no-default-features
```

The following Tauri commands are not registered in the no-OIDC build (calling
them from the frontend will fail; the UI already hides the call sites):

- `launch_oidc_browser`, `exchange_pkce_code`, `refresh_tokens`, `logout_session`
- `create_room`
- `search_users`, `list_accesses`, `add_access`, `remove_access`

The `is_oidc_enabled` and `get_session_state` commands remain available in
both builds so the frontend can detect the mode at runtime.
