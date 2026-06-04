# Visio Mobile OAuth2 + PKCE Cross-Layer Security Audit

**Date:** 2026-06-04
**Scope:** end-to-end flow Mobile → Custom Tab → Meet `/authenticate/` → Keycloak → Meet `/callback/` → `visio://auth-callback` → Meet `/oauth/token/`. Findings focus on cross-layer integration; per-layer Android/Rust issues are deferred to the other agents.
**Audit type:** hostile / adversarial review on the user's own beta product (authorized).

---

## CRITICAL

### C1 — Authorization code TOCTOU race in `/oauth/token/` (Redis cache)
**Layers:** `meet/src/backend/core/authentication/views.py:182-191` + `meet/settings.py:311-323` (django-redis backend).

**Attack:**
`cache.get(cache_key)` + `cache.delete(cache_key)` are **two separate Redis round-trips**, not an atomic `GETDEL`. Two parallel `POST /api/v1.0/oauth/token/` with the same `code` (one from the legit mobile, one from a malicious app that intercepted the `visio://` deep link — see C2) can both read `code_data` before either deletes it. Each then independently computes its own `code_verifier`'s challenge; if the attacker has obtained or guessed nothing, they fail constant-time check and the legit one succeeds — fine. But the dangerous scenario is concurrent retries from the SAME client (network hiccup → retry storm) where two valid pairs of JWTs are issued for the same code, defeating single-use and silently doubling the refresh-token surface. The comment on line 189 ("invalidate BEFORE verification") only protects against verifier *guessing oracle*, not against duplicate issuance.

**Fix:**
Use atomic claim: `django-redis` exposes `cache.client.get_client().getdel(key)` (Redis 6.2+) or wrap in a Lua script / `WATCH`-`MULTI`. Minimum: replace lines 183-191 with a single GETDEL so the second concurrent request gets `None`.

**PoC:** `ab -n 2 -c 2 -p body.json http://meet/api/v1.0/oauth/token/` with `body.json={"code":"…","code_verifier":"…"}` → observe two `{"access":…,"refresh":…}` responses where one was expected.

---

## HIGH

### H1 — `MOBILE_DEEP_LINK_SCHEME` env var = unrestricted redirect / XSS-via-redirect / token exfiltration
**Layers:** `meet/src/backend/core/authentication/views.py:91-100,149-153` + `meet/settings.py:600-604`.

`MobileFriendlyRedirect.allowed_schemes` (line 99) does `settings.MOBILE_DEEP_LINK_SCHEME.split(":")[0]` and trusts it. The redirect target itself comes verbatim from `settings.MOBILE_DEEP_LINK_SCHEME`.

**Attacks (any one is sufficient):**
1. **Operator misconfig / env injection:** setting `MOBILE_DEEP_LINK_SCHEME=javascript://auth-callback` makes `javascript` an allowed scheme and the server returns `Location: javascript://auth-callback?code=…&state=…`. Many browsers will execute (Chrome strips `Location: javascript:` but some embeds and curl wrappers do not). Token in URL.
2. **Operator misconfig (lower bar):** `MOBILE_DEEP_LINK_SCHEME=https://attacker.example/x` — the verifier is still needed, but combined with C2 below this lets an attacker harvest codes from a victim instance.
3. **No validation that the scheme is actually a registered native-app scheme** (e.g. enforce regex `^[a-z][a-z0-9+\-.]{2,30}://[a-z0-9\-]+$` and reject `http`, `https`, `javascript`, `data`, `file`, `vbscript`, `chrome`, `about`).

**Fix:** validate `MOBILE_DEEP_LINK_SCHEME` at Django check-time. Reject any value whose scheme appears in `HttpResponseRedirect.allowed_schemes` (i.e. require it to be a *new* scheme not already known to the framework, and not in a denylist).

---

### H2 — Mobile `state` is never validated server-side (cross-PKCE replay window)
**Layers:** `views.py:69-87,149-152` (server stores+echoes state, never validates) + `OidcAuthManager.kt:130-133` (client-only check).

The PKCE-app `state` enters the server, is stored in session, and is echoed back into the deep link unmodified. It is **never compared on the server**. Combined with C2 (deep link hijack), this means:

**Attack chain:** malicious local app A also registers `visio://`. Victim taps "Login" in real Visio (V). V generates verifier_V, challenge_V, state_V; opens Custom Tab to Meet with challenge_V + state_V. Meet→Keycloak login succeeds; Meet redirects browser to `visio://auth-callback?code=X&state=state_V`. Android chooser launches A. A holds (code=X, state=state_V) but has no verifier_V. A separately runs its own PKCE flow (verifier_A, state_A) against the same Meet instance; gets its own code Y bound to challenge_A. A cannot mix them — server binds code↔challenge in cache. So the *code* is safe.

**But:** A can now silently relay the deep link back to Visio (`startActivity(Intent(visio://auth-callback?...))`) and Visio will exchange and obtain valid tokens *for the correct user*. From Visio's perspective everything is normal. A has now learned: (i) the user uses Visio, (ii) the timing of the login, (iii) potentially the user identity via traffic correlation. Worse, if A delays the relay arbitrarily long, V's verifier in EncryptedSharedPreferences may have been cleared by another aborted flow → user re-runs login, and A can now use the OLD code with the OLD verifier IF A also captured the verifier (it cannot from EncryptedSharedPreferences — verifier never leaves V). So actual token theft requires verifier access (out of scope here per Android agent).

**The genuine cross-layer concern:** because server doesn't bind state to code, a victim who installs malicious-Visio-clone-with-same-scheme can have its login attempts *transparently observed and replayed* (login UX still works, no warning). The 1-byte fix is to **bind state to code at cache level**:

```python
cache.set(key, {"user_id":…, "code_challenge":…, "state":pkce_data["state"]}, …)
# In token view: require POST to include state, compare in constant time.
```

Then mobile clients MUST send `state` with the token POST. This makes deep-link relay detectable (mismatched state) and aligns with RFC 6749 §10.12 / OAuth 2.1 §4.1.3.

---

### H3 — Keycloak `meet` client: PKCE NOT enforced, confidential client used as native-app backend
**Layers:** `docker/auth/realm.json:679-748` (no `pkce.code.challenge.method` attribute), contrast with `account-console` at line 568 and another client at 813 which DO set `"pkce.code.challenge.method":"S256"`.

**Construction:** Meet acts as the OIDC client to Keycloak using `client_secret` (line 689 `ThisIsAnExampleKeyForDevPurposeOnly`). Meet then layers its OWN app-level PKCE on top, binding mobile↔Meet. This is **defensible** (OAuth-for-Web-Backend pattern with a PKCE-protected resource ticket on top) — the app-level PKCE prevents the deep-link from being a usable bearer token.

But the missing Keycloak-side PKCE means: anyone who can read the Meet session cookie or Meet server logs can replay the Meet↔Keycloak code (no verifier protects that leg). And `redirectUris` (line 691-695) includes `http://localhost:*` with a wildcard — for a production Keycloak this is wildly permissive.

**Fixes:**
1. Add `"pkce.code.challenge.method":"S256"` to the `meet` client attributes (defense-in-depth for the Meet↔Keycloak leg).
2. Document explicitly in the deployment guide that **production must replace** `secret` (line 689), tighten `redirectUris` to exact production HTTPS URLs, and remove all `localhost`/`10.0.2.2` entries.
3. Set Django `OIDC_REDIRECT_REQUIRE_HTTPS=True` in production (currently default `False`, settings.py:559-561).

---

### H4 — JWT signing key fallback to `SECRET_KEY` with HS256 (symmetric) + weak default
**Layers:** `meet/settings.py:631-642` (`"ALGORITHM":"HS256"`, `"SIGNING_KEY": self.SIMPLE_JWT_SIGNING_KEY or self.SECRET_KEY`) + `settings.py:1181` (`Build.SECRET_KEY = values.Value("DummyKey")`).

**Cross-layer impact:** if an operator deploys with `SIMPLE_JWT_SIGNING_KEY` unset AND `SECRET_KEY` taken from a build image (`DummyKey`) or from env that leaked, **anyone with that key can forge any user's access and refresh JWTs**, bypassing the entire PKCE/Keycloak flow. HS256 means a single secret = mint+verify capability for the whole fleet of mobile clients. With RS256 (or EdDSA) the signing key would live on the auth server only.

**Fixes:**
1. Force `SIMPLE_JWT_SIGNING_KEY` to be set in `Production` (raise at startup if `None`).
2. Switch SimpleJWT to `"ALGORITHM":"RS256"` with a keypair; rotate via `SIGNING_KEY`/`VERIFYING_KEY` config.
3. Add a `django.core.checks` that fails deploy if `SECRET_KEY in {"DummyKey","CHANGE_ME"}` or < 50 chars.

---

## MEDIUM

### M1 — Concurrent refresh race under `BLACKLIST_AFTER_ROTATION=True`
**Layers:** `settings.py:640-641` + Rust `tokens.rs:144-184` (in-process serialized via `refresh_lock`).

The Rust side serializes refreshes *within one process*. But the same refresh token may exist on multiple installs (user switched device, restored backup) or across the network if a refresh fires concurrently from the Rust process and a stale background task. SimpleJWT's `BLACKLIST_AFTER_ROTATION` uses Django ORM with the DB's default isolation (PostgreSQL: READ COMMITTED). Two parallel refresh calls with the same token can both see it as un-blacklisted, both issue new pairs, and only one of the blacklist inserts will be the "winner" (the other may unique-constraint-fail and be 500). Net: legitimate users may get 500 on race; in some configurations both new pairs persist (token amplification).

**Fix:** wrap the refresh view in `select_for_update()` on the `OutstandingToken` row, or use SERIALIZABLE for that view. SimpleJWT 5.5.1 does not do this automatically.

### M2 — `AUTH_PKCE_CACHE_TTL_SECONDS=60` is too tight for some IdP flows
**Layer:** `settings.py:595-599` + Keycloak realm (MFA / step-up auth can take >60s; corporate IdPs even more).

60s is the wall-clock from when Meet caches the code (end of OIDC callback) to when the mobile app exchanges it. The Custom Tab → Android handoff back to the app is normally <1s, so this is fine in the happy path. But if the OS suspends the app (notification, low memory), the code expires and the user must re-login with no clear UX signal. Recommendation: 120-300s with cache jitter, document the choice.

### M3 — `/mobile-login` endpoint is publicly reachable as a 204 marker
**Layer:** `urls.py:55`. Anyone can `GET /mobile-login` and get a 204 — a noise-free fingerprint that a deployment has the PKCE mobile flow enabled. Minor info-disclosure; not exploitable on its own. **Fix:** restrict by referer / origin, or simply move it under `/api/v1.0/_internal/mobile-login` with a `Cache-Control: no-store`. Low priority.

### M4 — Logout is local-only (no server-side blacklist call)
**Layer:** `VisioManager.kt:398-419` clears local storage but never POSTs the refresh token to `/api/token/blacklist/`. Refresh tokens harvested from a rooted device, ADB backup, or pre-logout memory dump remain valid for up to 7 days. **Fix:** add a `POST /api/v1.0/oauth/token/blacklist/` (SimpleJWT's `TokenBlacklistView`) and call it from `logout()` before clearing locally. URLs config has no such route currently.

### M5 — Refresh trigger is reactive (on 401) only
**Layer:** Rust `tokens.rs:144-184` + `session.rs` — refresh fires only when an API call fails. For a 10-min access token in a 90-min meeting, every refresh window is a potential mid-call hiccup. UX/availability issue rather than security, but worth pairing with a proactive refresh at T-60s of `exp`.

---

## LOW / INFO

### L1 — Keycloak `meet` client secret committed to public repo
`realm.json:689` ships `ThisIsAnExampleKeyForDevPurposeOnly` — fine for dev, but the realm.json is imported on every Keycloak start. Production deployments must use a separate `realm.json` (not committed) or override via Keycloak admin API post-import. **Document this prominently in release notes** (not currently in the last 7 commits' messages).

### L2 — `OIDC_REDIRECT_REQUIRE_HTTPS` defaults to `False` (`settings.py:559-560`)
Should default `True` and be opt-out for dev only. Currently a production operator who forgets to flip it can accept `http://` returnTo.

### L3 — Mobile `state` and OIDC `state` are distinct, neither is bound to the user
The Meet↔Keycloak `state` is managed by lasuite/mozilla-django-oidc (session-bound, validated). The mobile-app `state` is mobile↔Meet only, client-validated. There's no cryptographic link between them. RFC 9700 (OAuth Security BCP) §4.1.3 recommends binding state to the user session — currently only the OIDC layer does this. Not actionable as a vulnerability by itself; recommend H2's fix (cache-bind state to code) which covers it.

### L4 — `pkceGenerate()` 32-byte verifier is the floor of RFC 7636 (43 chars base64url)
Already noted by the Rust agent; mentioned only because the mobile↔server contract (`schemas.py:23-26` requires 43-128 chars) and `pkce.rs:24` (`random_base64url(32)` → 43 chars) are at the lower bound. If a future Rust refactor reduces to 24 bytes, the server will silently reject every login. Pin with a test.

---

## What was NOT found (intellectual honesty)

- No path where the cached code can be reused with a different verifier (the binding at `views.py:131-137` is correct, and `cache.delete` happens before challenge computation — that part is solid).
- Constant-time comparison at `views.py:197` uses `secrets.compare_digest` — correct.
- Pydantic models (`schemas.py`) properly constrain code/verifier/state to base64url charset and lengths matching RFC 7636.
- `MobileFriendlyRedirect` correctly extends `allowed_schemes` (the issue is the unvalidated value, not the mechanism).

## Key files in scope

- `/home/mmaudet/work/meet/src/backend/core/authentication/views.py`
- `/home/mmaudet/work/meet/src/backend/core/authentication/schemas.py`
- `/home/mmaudet/work/meet/src/backend/core/urls.py`
- `/home/mmaudet/work/meet/src/backend/meet/settings.py` (lines 311-323, 472-478, 559-561, 590-642, 1181)
- `/home/mmaudet/work/meet/docker/auth/realm.json` (lines 677-748)
- `/home/mmaudet/work/visio-mobile/crates/visio-core/src/tokens.rs`
- `/home/mmaudet/work/visio-mobile/android/app/src/main/kotlin/io/visio/mobile/auth/OidcAuthManager.kt`
- `/home/mmaudet/work/visio-mobile/android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt` (lines 305-419)
- `/home/mmaudet/work/visio-mobile/android/app/src/main/AndroidManifest.xml`

**Priority recommendation:** fix C1 (atomic GETDEL) and H2 (bind state server-side) in the same PR — both touch `views.py` lines 129-200 and together harden the code↔verifier↔state triple. H1 and H4 are deployment-config hardening that should land before the first non-developer beta tester is onboarded.
