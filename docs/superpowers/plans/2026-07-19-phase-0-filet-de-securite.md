# Phase 0 — Filet de sécurité (CI + E2E + i18n + ESLint) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rendre la détection automatique fiable : suite Playwright 100 % verte et lancée en CI, check-i18n couvrant les clés utilisées, ESLint à 0 warning, docs à jour.

**Architecture:** Réparer l'infrastructure de test existante (mock Tauri + specs) sans changer le comportement de l'app — les seules modifications côté app sont 2 `data-testid` et du code mort supprimé. Spec de référence : `docs/superpowers/specs/2026-07-19-desktop-dogfooding-fiabilisation-design.md`.

**Tech Stack:** Playwright + mock `__TAURI_INTERNALS__` (e2e/tauri-mock.ts), React/TS strict, ESLint flat + Prettier, GitHub Actions, bash + python3 (check-i18n.sh).

## Global Constraints

- Aucun changement de comportement app. Modifications `src/App.tsx` limitées à : ajout de `data-testid="prejoin-join-button"`, ajout de `data-testid="settings-close-button"`, suppression de code mort vérifié, typage (aucune logique modifiée sauf Tasks 6/8 listées explicitement).
- Style Prettier (vérifié par `npm run format:check`) : 2 espaces, simple quote, pas de point-virgule, trailing commas.
- Messages de commit conventionnels (gitlint), ex. `fix(e2e): ...`, `ci: ...`, `i18n: ...`.
- `CHANGELOG.md` : lignes < 80 caractères (CI `lint-changelog`).
- TDD : chaque réparation de spec commence par constater l'échec actuel (déjà documenté dans chaque tâche, le revérifier avant de modifier).
- Toutes les commandes frontend se lancent depuis `crates/visio-desktop/frontend/`.

### État initial constaté (2026-07-19)

- `npx playwright test` : 12 passent, 24+ échouent. Cause racine unique pour 28 échecs : l'app a gagné un écran PreJoin ; `joinMockRoom` ne le traverse pas et le mock n'émet jamais `connection-state-changed`.
- `npx eslint src/` : 40 warnings (9 no-unused-vars, 18 no-explicit-any, 6 react-hooks/exhaustive-deps, 7 no-empty), tous dans `src/App.tsx`.
- `scripts/check-i18n.sh` : OK mais ne vérifie que la parité entre locales ; 4 clés utilisées manquent partout : `action.add`, `action.back`, `action.close`, `action.remove` (utilisées dans `App.tsx:2152,2290,2298,2342,3160,3239,4011,4037`).
- Non commité au moment de l'écriture du plan : correctif auth (`start_oidc_auth`), 3 clés i18n (`home.signIn`…), mock réparé, `e2e/auth-required.spec.ts`, spec de design. À commiter en premier (voir Task 0).

---

### Task 0: Commiter le travail de la session (correctif auth + spec)

Le correctif auth du 2026-07-19 est vérifié (2 tests verts, tsc/eslint/prettier/i18n OK) mais non commité.

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx` (déjà modifié)
- Modify: `crates/visio-desktop/frontend/e2e/tauri-mock.ts` (déjà modifié)
- Create: `crates/visio-desktop/frontend/e2e/auth-required.spec.ts` (déjà créé)
- Modify: `i18n/{en,fr,de,es,it,nl}.json` (déjà modifiés)
- Create: `docs/superpowers/specs/2026-07-19-desktop-dogfooding-fiabilisation-design.md`

- [ ] **Step 1: Vérifier l'état**

Run: `git status --short`
Expected: les fichiers ci-dessus en modifié/non-suivi, rien d'autre.

- [ ] **Step 2: Commit (demander confirmation utilisateur avant)**

```bash
git add crates/visio-desktop/frontend/src/App.tsx \
  crates/visio-desktop/frontend/e2e/tauri-mock.ts \
  crates/visio-desktop/frontend/e2e/auth-required.spec.ts \
  i18n/ docs/superpowers/specs/2026-07-19-desktop-dogfooding-fiabilisation-design.md
git commit -m "fix(desktop): repair OIDC auth flow from join screen

handleAuth invoked a nonexistent Tauri command (start_oidc_auth),
broken since 860ea4c. Use launch_oidc_browser via onLaunchOidc and
re-validate the room after the auth-callback deep link completes.

Also: add 3 missing i18n keys (home.signIn, home.room.authRequired,
home.room.authenticating) in 6 locales, deduplicate room-status JSX
blocks, repair E2E mock crash (get_visio_history/upcoming_meetings),
add mock invoke log + event emitter helpers + auth-required spec."
```

---

### Task 1: Mock E2E — traversée PreJoin + événement de connexion

Répare d'un coup ~28 tests (chat 10, device-selection 8, screen-share 5, call-controls 5+).

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx:4722` (ajout testid)
- Modify: `crates/visio-desktop/frontend/e2e/tauri-mock.ts` (case `connect` + `joinMockRoom`)
- Test: `crates/visio-desktop/frontend/e2e/{chat,device-selection,call-controls,screen-share}.spec.ts`

**Interfaces:**
- Consumes: `__emitTauriEvent(event, payload)` déjà exposé par le mock sur `window`.
- Produces: `data-testid="prejoin-join-button"` (utilisé par `joinMockRoom` et les futurs specs) ; le mock émet `connection-state-changed` payload `'connected'` après `connect`.

- [ ] **Step 1: Constater le RED (baseline)**

Run: `cd crates/visio-desktop/frontend && npx playwright test e2e/chat.spec.ts 2>&1 | tail -5`
Expected: 10 failed, tous sur `waiting for getByTestId('call-mic-button')` dans `joinMockRoom` (tauri-mock.ts).

- [ ] **Step 2: Ajouter le testid sur le bouton « Join now » (App.tsx:4722)**

```tsx
// AVANT
        <button className="btn btn-primary" onClick={handleJoinNow}>
          {t('prejoin.joinNow')}
        </button>

// APRÈS
        <button
          className="btn btn-primary"
          data-testid="prejoin-join-button"
          onClick={handleJoinNow}
        >
          {t('prejoin.joinNow')}
        </button>
```

- [ ] **Step 3: Le mock émet `connection-state-changed` après `connect` (tauri-mock.ts)**

```ts
// AVANT
          case 'connect':
            return;

// APRÈS
          case 'connect':
            // The real backend emits "connection-state-changed: connected"
            // once the LiveKit connection succeeds; PreJoinScreen only opens
            // the CallView after receiving it. Listeners are registered
            // before connect() is invoked, so a 0ms delay is enough.
            setTimeout(() => {
              (window as any).__emitTauriEvent(
                'connection-state-changed',
                'connected',
              );
            }, 0);
            return;
```

- [ ] **Step 4: `joinMockRoom` traverse l'écran PreJoin (tauri-mock.ts)**

```ts
// AVANT
  await page.getByTestId('home-join-button').click();

  // Wait for call view to render
  await page.getByTestId('call-mic-button').waitFor({ timeout: 5000 });

// APRÈS
  await page.getByTestId('home-join-button').click();

  // The app shows a pre-join (lobby) screen before entering the call
  await page.getByTestId('prejoin-join-button').click();

  // Wait for call view to render
  await page.getByTestId('call-mic-button').waitFor({ timeout: 5000 });
```

- [ ] **Step 5: Vérifier le GREEN**

Run: `npx playwright test e2e/chat.spec.ts e2e/device-selection.spec.ts e2e/call-controls.spec.ts e2e/screen-share.spec.ts 2>&1 | tail -8`
Expected: seuls échecs restants = `screen-share.spec.ts` test « source picker shows dimensions for sources » (traité en Task 3). Tout le reste passe.

- [ ] **Step 6: Commit (demander confirmation utilisateur)**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/e2e/tauri-mock.ts
git commit -m "fix(e2e): traverse pre-join screen in joinMockRoom

The app gained a PreJoinScreen step; the mock never emitted
connection-state-changed and the helper never clicked Join now,
breaking 28 tests. Add prejoin-join-button testid."
```

---

### Task 2: Rétablir le testid `settings-close-button` (home.spec)

Le passage de Settings en pleine page (commit `b2b0a13`) a perdu le testid du bouton retour.

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx:3921`
- Test: `crates/visio-desktop/frontend/e2e/home.spec.ts`

**Interfaces:**
- Produces: `data-testid="settings-close-button"` sur le bouton retour de SettingsView (consommé par `home.spec.ts:38`).

- [ ] **Step 1: Constater le RED**

Run: `npx playwright test e2e/home.spec.ts 2>&1 | tail -5`
Expected: 1 failed — « settings modal opens and closes » timeout sur `settings-close-button`.

- [ ] **Step 2: Ajouter le testid (App.tsx:3921)**

```tsx
// AVANT
        <button className="settings-back-btn" onClick={onClose}>
          <RiArrowLeftSLine size={22} />
        </button>

// APRÈS
        <button
          className="settings-back-btn"
          data-testid="settings-close-button"
          onClick={onClose}
        >
          <RiArrowLeftSLine size={22} />
        </button>
```

- [ ] **Step 3: Vérifier le GREEN**

Run: `npx playwright test e2e/home.spec.ts 2>&1 | tail -3`
Expected: 6 passed.

- [ ] **Step 4: Commit (demander confirmation utilisateur)**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "fix(desktop): restore settings-close-button testid lost in full-page refactor"
```

---

### Task 3: Supprimer le test « dimensions » obsolète (screen-share.spec)

Le test attend l'affichage de `1920`/`720` dans le picker, fonctionnalité **jamais implémentée** (`git log -S "1920"` sur App.tsx : vide ; `SourcePickerModal` n'affiche que thumbnail + nom). Ce n'est pas une régression app : c'est une attente pour une feature inexistante. La couverture des sources est déjà assurée par les tests 2 et 3 du même spec. (Si l'utilisateur préfère implémenter l'affichage des dimensions, sortir ce point du plan et le traiter comme une feature séparée.)

**Files:**
- Modify: `crates/visio-desktop/frontend/e2e/screen-share.spec.ts:61-67`

- [ ] **Step 1: Constater l'échec résiduel**

Run: `npx playwright test e2e/screen-share.spec.ts 2>&1 | tail -6`
Expected: 1 failed — « source picker shows dimensions for sources ».

- [ ] **Step 2: Supprimer le test (screen-share.spec.ts:61-67)**

```ts
// SUPPRIMER ce bloc entier :
test('source picker shows dimensions for sources', async ({ page }) => {
  await page.getByTestId('call-screen-share-button').click();
  const picker = page.getByTestId('screen-share-source-picker');
  await expect(picker.getByText('1920')).toBeVisible();
  await expect(picker.getByText('720')).toBeVisible();
});
```

(Le texte exact peut varier légèrement : supprimer le `test(...)` dont le nom est « source picker shows dimensions for sources ».)

- [ ] **Step 3: Vérifier le GREEN**

Run: `npx playwright test e2e/screen-share.spec.ts 2>&1 | tail -3`
Expected: 5 passed.

- [ ] **Step 4: Commit (demander confirmation utilisateur)**

```bash
git add crates/visio-desktop/frontend/e2e/screen-share.spec.ts
git commit -m "test(e2e): drop screen-share dimensions test for a feature that never existed"
```

---

### Task 4: Job CI Playwright

La suite tourne en local ; elle n'est jamais lancée en CI. Ajouter un job `test-frontend` calqué sur `lint-frontend` (`.github/workflows/ci.yml:86-105`). Le `webServer` de `playwright.config.ts` démarre `npm run dev` tout seul.

**Files:**
- Modify: `.github/workflows/ci.yml` (après le job `lint-frontend`, ligne 105)

**Interfaces:**
- Consumes: suite verte après Tasks 1-3 (prérequis : merger ce job après les Tasks 1-3, sinon la CI est rouge).
- Produces: job `test-frontend` visible dans les checks de PR.

- [ ] **Step 1: Ajouter le job (ci.yml, après `lint-frontend`)**

```yaml
  test-frontend:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: crates/visio-desktop/frontend
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6
      - name: Set up Node.js
        uses: actions/setup-node@53b83947a5a98c8d113130e565377fae1a50d02f # v6
        with:
          node-version: "22"
          cache: "npm"
          cache-dependency-path: >
            crates/visio-desktop/frontend/package-lock.json
      - name: Install dependencies
        run: npm ci
      - name: Install Playwright browsers
        run: npx playwright install --with-deps chromium
      - name: Run Playwright tests
        run: npx playwright test
      - name: Upload test results on failure
        if: failure()
        uses: actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f # v7
        # même SHA épinglé que les 4 autres workflows du repo (build-*.yml)
        with:
          name: playwright-test-results
          path: crates/visio-desktop/frontend/test-results/
          retention-days: 7
```

- [ ] **Step 2: Valider le YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('YAML OK')"`
Expected: `YAML OK`

- [ ] **Step 3: Vérifier localement la séquence CI**

Run: `cd crates/visio-desktop/frontend && npx playwright test 2>&1 | tail -3`
Expected: suite 100 % verte (le job CI exécute exactement cette commande).

- [ ] **Step 4: Commit (demander confirmation utilisateur)**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run Playwright E2E suite on PRs (test-frontend job)"
```

---

### Task 5: check-i18n renforcé + 4 clés `action.*`

`scripts/check-i18n.sh` ne vérifie que la parité entre locales. Les clés `action.add`, `action.back`, `action.close`, `action.remove` sont utilisées dans `App.tsx` (aria-labels) mais absentes des 6 locales — les aria-labels affichent la clé brute.

**Files:**
- Modify: `scripts/check-i18n.sh`
- Modify: `i18n/{en,fr,de,es,it,nl}.json`

**Interfaces:**
- Consumes: clés littérales `t('...')` dans `crates/visio-desktop/frontend/src/**/*.{ts,tsx}`.
- Produces: check-i18n échoue si une clé utilisée (littérale) manque dans `en.json` ; les 4 clés `action.*` présentes dans les 6 locales.

- [ ] **Step 1: Ajouter la vérification des clés utilisées (check-i18n.sh)**

Insérer avant la boucle `for locale_file` (ligne 21) :

```bash
# Check that all literal t('...') keys used in the desktop frontend exist
FRONTEND_SRC="$REPO_ROOT/crates/visio-desktop/frontend/src"
USED_KEYS=$(grep -rhoE "t\('[a-zA-Z0-9_.]+'" "$FRONTEND_SRC" \
  | sed "s/^t('//; s/'$//" | sort -u || true)
MISSING_USED=$(comm -23 <(echo "$USED_KEYS") <(echo "$SOURCE_KEYS"))
if [ -n "$MISSING_USED" ]; then
    echo "USED in frontend but MISSING in en.json:"
    echo "$MISSING_USED" | sed 's/^/  - /'
    MISSING=1
fi
```

- [ ] **Step 2: Constater le RED**

Run: `bash scripts/check-i18n.sh`
Expected: échec listant `action.add`, `action.back`, `action.close`, `action.remove` (et rien d'autre — les 3 clés `home.signIn`… ont été ajoutées dans Task 0).

- [ ] **Step 3: Ajouter les 4 clés dans les 6 locales**

Dans chaque fichier, après la clé `"home.signIn"` (ajoutée en Task 0) :

| Clé | en | fr | de | es | it | nl |
|---|---|---|---|---|---|---|
| `action.close` | Close | Fermer | Schließen | Cerrar | Chiudi | Sluiten |
| `action.back` | Back | Retour | Zurück | Volver | Indietro | Terug |
| `action.remove` | Remove | Supprimer | Entfernen | Eliminar | Rimuovi | Verwijderen |
| `action.add` | Add | Ajouter | Hinzufügen | Añadir | Aggiungi | Toevoegen |

Exemple pour `i18n/en.json` :

```json
  "home.signIn": "Sign in",
  "action.close": "Close",
  "action.back": "Back",
  "action.remove": "Remove",
  "action.add": "Add",
```

- [ ] **Step 4: Vérifier le GREEN**

Run: `bash scripts/check-i18n.sh`
Expected: `All locales have all keys from en.json` (exit 0).

- [ ] **Step 5: Commit (demander confirmation utilisateur)**

```bash
git add scripts/check-i18n.sh i18n/
git commit -m "i18n: check used keys against en.json + add missing action.* keys"
```

---

### Task 6: Suppression du code mort (9 warnings no-unused-vars)

Suppressions vérifiées (grep à l'appui) dans `crates/visio-desktop/frontend/src/App.tsx`. Les numéros de ligne sont indicatifs — chercher par symbole, pas par ligne.

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Interfaces:**
- Consumes: rien (premier chantier ESLint).
- Produces: `npm run lint` passe de 40 à ~31 warnings.

- [ ] **Step 1: Constater le RED (baseline)**

Run: `npx eslint src/ 2>&1 | grep -c 'no-unused-vars'`
Expected: 9 occurrences.

- [ ] **Step 2: Supprimer les codes morts**

1. **Composant `VisioLogo`** (~lignes 238-326) : supprimer le composant entier (de `function VisioLogo(` à sa dernière accolade). Vérification préalable : `grep -n '<VisioLogo' src/App.tsx` → aucune occurrence.
2. **`ParticipantTile` (~lignes 440-441)** : supprimer les deux lignes de calcul inutilisées `const initials = ...` et `const hue = ...` (les fonctions `getInitials`/`getHue` restent utilisées ailleurs — ne pas les supprimer).
3. **`MeetingsView` state `calendarUrl` (~ligne 641)** : supprimer `const [calendarUrl, setCalendarUrl] = useState<...>(...)` et l'appel `setCalendarUrl(...)` dans l'effet (~ligne 660) ; la variable locale `url` garde la logique.
4. **`HomeView` state `roomDisplayName` (~ligne 919)** : supprimer le `useState` et le bloc mort qui le lit dans `handleJoin` (~lignes 1144-1148 : le `if (trimmedDisplayName)` qui ajoute `?visio=` à l'URL — le champ a été retiré du formulaire, voir commentaire ligne ~1430).
5. **`CreateRoomView` state `searching` (~ligne 1595)** : supprimer le `useState` et les 2 appels `setSearching(...)` (~lignes 1612, 1625).
6. **`App` state `pendingOidcInstance` (~ligne 4856)** : supprimer le `useState` et les 3 appels `setPendingOidcInstance(...)` (handler deep-link ~ligne 4931, `onLaunchOidc` ~lignes 5605 et 5610). `pendingOidcRef` (source de vérité) reste inchangé.
7. **2 `catch (_)` (~lignes 5738, 5743)** : remplacer `} catch (_) {` par `} catch {` (optional catch binding, ES2020).

- [ ] **Step 3: Vérifier type-check + lint**

Run: `npx tsc --noEmit && npx eslint src/ 2>&1 | grep -c 'no-unused-vars'`
Expected: tsc OK ; 0 occurrence de `no-unused-vars` (les warnings restants sont any/exhaustive-deps/no-empty, traités en Tasks 7-8).

- [ ] **Step 4: Vérifier la suite (pas de régression)**

Run: `npx playwright test 2>&1 | tail -3`
Expected: tout passe (la suppression est du code mort — aucun test ne doit bouger).

- [ ] **Step 5: Commit (demander confirmation utilisateur)**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "chore(desktop): remove dead code flagged by eslint no-unused-vars"
```

---

### Task 7: Typer les 18 warnings no-explicit-any

Les types exacts existent côté Rust (`crates/visio-core/src/access.rs:8-22`) ; serde sérialise `None` en `null`.

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Interfaces:**
- Produces (déclarées près de `VisioHistoryEntry`, ~ligne 167) :

```ts
interface UserSearchResult {
  id: string
  email: string
  full_name: string | null
  short_name: string | null
}
interface RoomAccess {
  id: string
  user: UserSearchResult
  resource: string
  role: string
}
```

- [ ] **Step 1: Constater le RED**

Run: `npx eslint src/ 2>&1 | grep -c 'no-explicit-any'`
Expected: 18 occurrences.

- [ ] **Step 2: Déclarer les 2 interfaces puis typer**

Après `interface VisioHistoryEntry { ... }` (~ligne 167-170), ajouter les 2 interfaces ci-dessus. Puis dans `CreateRoomView` et le panneau d'accès :

| Emplacement (indicatif) | Avant | Après |
|---|---|---|
| ~1593 | `useState<any[]>([])` (searchResults) | `useState<UserSearchResult[]>([])` |
| ~1594 | `useState<any[]>([])` (invitedUsers) | `useState<UserSearchResult[]>([])` |
| ~1614 | `invoke<any[]>('search_users')` | `invoke<UserSearchResult[]>('search_users')` |
| ~1619, 1819, 1840, 1848 | `(u: any)`, `(inv: any)`, `(user: any)` | supprimer les annotations `: any` (inférence) |
| ~2071 | `useState<any[]>([])` (roomAccesses) | `useState<RoomAccess[]>([])` |
| ~2073 | `useState<any[]>([])` (memberResults) | `useState<UserSearchResult[]>([])` |
| ~2094, 2225 | `invoke<any[]>('list_accesses')` | `invoke<RoomAccess[]>('list_accesses')` |
| ~2111 | `invoke<any[]>('search_users')` | `invoke<UserSearchResult[]>('search_users')` |
| ~2116, 2217, 2244, 2259 | `(u: any)`, `(a: any)`, `(user: any)`, `(access: any)` | supprimer les annotations `: any` |

- [ ] **Step 3: Vérifier**

Run: `npx tsc --noEmit && npx eslint src/ 2>&1 | grep -c 'no-explicit-any'`
Expected: tsc OK ; 0 occurrence.

- [ ] **Step 4: Commit (demander confirmation utilisateur)**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "chore(desktop): type room access API results (UserSearchResult, RoomAccess)"
```

---

### Task 8: exhaustive-deps (6) + no-empty (7) + CI lint aligné

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`
- Modify: `.github/workflows/ci.yml:102-105`

**Interfaces:**
- Consumes: Tasks 6-7 (seuls restent 13 warnings : 6 exhaustive-deps + 7 no-empty).
- Produces: `npm run lint` vert (0 warning) ; CI `lint-frontend` utilise `npm run lint` (équivalent local/CI).

- [ ] **Step 1: Corriger les 6 exhaustive-deps**

1. **MeetingsView listeners (~ligne 726, vrai bug de closure périmée)** : le listener `calendar-error` lit `meetings.length` figé au mount. Ajouter un ref miroir et corriger les deps :

```tsx
// Ajouter près de la déclaration de `meetings` dans MeetingsView :
const meetingsRef = useRef<Meeting[]>([])
useEffect(() => {
  meetingsRef.current = meetings
}, [meetings])

// Dans le listener calendar-error (~ligne 715), remplacer :
//   if (meetings.length === 0) setStatus('error')
// par :
    if (meetingsRef.current.length === 0) setStatus('error')

// Et deps de l'effet (~ligne 726) : `}, [])` → `}, [t])`
```

2. **HomeView listener `meeting-reminder` (~ligne 1014)** : `}, [])` → `}, [t])`.
3. **HomeView effet deepLinkUrl (~ligne 1036)** : côté App (~ligne 5597), stabiliser la prop :
   `onDeepLinkConsumed={useCallback(() => setDeepLinkUrl(null), [])}` — extraire en `const handleDeepLinkConsumed = useCallback(() => setDeepLinkUrl(null), [])` au niveau d'App et passer `onDeepLinkConsumed={handleDeepLinkConsumed}` ; côté HomeView, deps `[deepLinkUrl]` → `[deepLinkUrl, onDeepLinkConsumed]`.
4. **HomeView validation debounce (~ligne 1136)** : deps `[meetUrl]` → `[meetUrl, displayName, meetInstances]`.
5. **PreJoinScreen effet mount (~ligne 4278)** : destructurer la fonction stable du hook — remplacer l'appel `devices.enumerate()` par `enumerate()` où `enumerate` est destructuré du retour de `useDeviceEnumeration(...)` ; deps `[]` → `[enumerate]`. (Vérifié : `enumerate` est un `useCallback` stable, `useDeviceEnumeration.ts:99` — aucune boucle possible.)
6. **App listener `onOpenUrl` (~ligne 5022)** : conserver tel quel (ré-abonnement à chaque frappe du displayName indésirable) — ajouter juste au-dessus de la ligne des deps :

```tsx
    // eslint-disable-next-line react-hooks/exhaustive-deps -- listener global
    // enregistré une fois ; relire displayName/t à chaque frappe forcerait un
    // ré-abonnement permanent pour un cas d'usage rare (deep link).
```

- [ ] **Step 2: Justifier les 7 no-empty**

Dans chaque `catch {}` listé (~lignes 5051, 5062, 5070, 5078, 5086 scénario auto-connect E2E ; ~5454, 5464 push-to-talk), ajouter le commentaire `// best-effort` dans le bloc :

```ts
    } catch {
      // best-effort: un échec ne doit pas casser le scénario/handler clavier
    }
```

(La règle `no-empty` ignore les blocs contenant un commentaire.)

- [ ] **Step 3: Aligner le job CI lint-frontend sur les scripts npm**

Dans `.github/workflows/ci.yml`, remplacer :

```yaml
      - name: ESLint
        run: npx eslint src/
      - name: Prettier check
        run: npx prettier --check "src/**/*.{ts,tsx,css}"
```

par :

```yaml
      - name: ESLint
        run: npm run lint
      - name: Prettier check
        run: npm run format:check
```

- [ ] **Step 4: Vérifier le GREEN complet**

Run: `npm run lint && npm run format:check && npx tsc --noEmit`
Expected: tout passe, 0 warning ESLint.

- [ ] **Step 5: Vérifier la suite E2E**

Run: `npx playwright test 2>&1 | tail -3`
Expected: tout passe.

- [ ] **Step 6: Commit (demander confirmation utilisateur)**

```bash
git add crates/visio-desktop/frontend/src/App.tsx .github/workflows/ci.yml
git commit -m "chore(desktop): zero eslint warnings (deps, empty catch) + align CI on npm scripts"
```

---

### Task 9: AGENTS.md + CHANGELOG

**Files:**
- Modify: `AGENTS.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Mettre à jour AGENTS.md**

- Section « TypeScript » : remplacer « No ESLint or Prettier config exists » par les commandes réelles (`npm run lint`, `npm run format:check`) ; préciser que `--max-warnings 0` est en vigueur.
- Section « Test Commands › TypeScript » : préciser que la suite tourne en CI.
- Table « CI Summary » : ajouter la ligne `test-frontend` (Playwright) et corriger `lint-frontend` (eslint + prettier via npm scripts).

- [ ] **Step 2: Entrée CHANGELOG (lignes < 80 chars)**

Ajouter une section `## [Unreleased]` en haut (après le préambule, avant `## [0.9.0]`) :

```markdown
## [Unreleased]

### Fixed

- Desktop: OIDC sign-in from the join screen called a nonexistent
  Tauri command (start_oidc_auth) — broken since 0.8.x
- Desktop: missing i18n keys shown raw (home.signIn,
  home.room.authRequired/authenticating, action.*)
- E2E: Playwright suite repaired (mock crash, pre-join traversal,
  stale selectors) and now enforced by a CI job
- Desktop: dead code removal and zero-eslint-warning cleanup
```

- [ ] **Step 3: Vérifier la longueur des lignes**

Run: `max=$(grep -Ev "^\[.*\]: https://github.com" CHANGELOG.md | wc -L); echo "$max"; [ "$max" -lt 80 ] && echo OK`
Expected: `OK`.

- [ ] **Step 4: Commit (demander confirmation utilisateur)**

```bash
git add AGENTS.md CHANGELOG.md
git commit -m "docs: update AGENTS.md CI/testing sections + CHANGELOG for phase 0"
```

---

### Task 10: Vérification globale (gate de sortie Phase 0)

- [ ] **Step 1: Suite complète**

Run: `cd crates/visio-desktop/frontend && npx playwright test 2>&1 | tail -3`
Expected: 100 % passed (0 failed).

- [ ] **Step 2: Lint + format + types**

Run: `npm run lint && npm run format:check && npx tsc --noEmit`
Expected: tout passe.

- [ ] **Step 3: i18n**

Run: `bash scripts/check-i18n.sh`
Expected: `All locales have all keys from en.json`.

- [ ] **Step 4: Rust non impacté (sanity)**

Run: `cargo fmt -p visio-core -- --check && cargo clippy -p visio-core -- -D warnings && cargo test -p visio-core --lib 2>&1 | tail -2`
Expected: tout passe (aucun fichier Rust touché par ce plan — vérification de non-régression).

- [ ] **Step 5: Bilan git**

Run: `git log --oneline -12 && git status --short`
Expected: les commits des Tasks 0-9, arbre propre.
