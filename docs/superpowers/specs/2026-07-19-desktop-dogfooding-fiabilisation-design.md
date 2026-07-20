# Design — Fiabilisation desktop pour le dogfooding quotidien

Date : 2026-07-19
Statut : proposé
Approche retenue : **A — Fiabilisation dirigée par l'usage** (vs B « fondations d'abord », vs C « qualité d'appel d'abord »)

## Contexte et objectif

Rendre l'application **desktop** (Tauri 2, `crates/visio-desktop`) utilisable **au quotidien
sans contournement** (dogfooding). Périmètre : desktop d'abord ; mobile traité dans un
chantier ultérieur. Les quatre axes comptent : fiabilité des parcours, qualité en appel,
dette structurelle, outillage & garde-fous — ordonnés pour des gains visibles chaque semaine.

### Déclencheur

Le bug `start_oidc_auth` (commande Tauri inexistante appelée par `handleAuth`,
`App.tsx:1166`, cassée depuis le 2026-03-07 sans être détectée) a révélé l'absence de
filet : suite Playwright 100 % cassée sur `main` (crash au démarrage : le mock renvoyait
`null` pour `get_visio_history`/`get_upcoming_meetings`), aucun job CI Playwright,
clés i18n utilisées mais absentes (`home.signIn`, `home.room.authRequired`,
`home.room.authenticating` affichés en brut), 40 warnings ESLint masquant du code mort.

Correctifs déjà appliqués dans cette session : flux `auth_required` réparé
(`handleAuth` → `onLaunchOidc` + re-validation post-callback via `registerPostAuthAction`),
3 clés i18n ajoutées (6 locales), mock E2E réparé (crash + `launch_oidc_browser`/
`exchange_oidc_code`/`__invokeLog`/`__emitTauriEvent`), blocs de statut JSX dédupliqués,
2 tests de régression (`e2e/auth-required.spec.ts`). État de la suite après réparation du
mock : 12 passent, 24 échouent sur des sélecteurs obsolètes pré-existants (chat,
device-selection, screen-share, home modal).

## Analyse d'architecture

### Solide (à préserver)

- `visio-core` : logique métier centralisée, ~310 tests, zéro TODO, patterns cohérents
  (`EventEmitter`, services, machines à états).
- Backend Tauri : délégation propre au core (~90 commandes), capture native par OS.
- Mobile : rendu vidéo natif zero-copy (hors périmètre de ce chantier).

### Fragile (cible du chantier)

- **Frontend monolithe** : `App.tsx` ~5 800 lignes ; synchronisation hybride polling 1 s +
  événements ; aucune protection test effective jusqu'ici.
- **Rendu vidéo desktop dégradé** : JPEG base64 ~10 fps (vs natif mobile), coûteux en CPU.
- **Garde-fous absents** : pas de job CI Playwright ; `check-i18n.sh` ne vérifie que la
  parité entre locales, pas les clés utilisées ; ESLint `--max-warnings 0` violé sur
  `main` (CI `lint-frontend` en échec ignoré) ; `AGENTS.md` obsolète (ESLint/Prettier).
- **Duplication par OS** : commandes devices en 3 variantes `cfg` ; deux runtimes tokio
  temporaires dans `run()` (`lib.rs:2028,2049`) ; FFI `catch_unwind` partiel (mobile).

### Constat transversal

Chaque classe de bug rencontrée est détectable automatiquement. Le problème principal
n'est pas le code mais l'absence de filet : la phase 0 vient en premier pour cette raison.

## Phasage

### Phase 0 — Le filet de sécurité (2-3 jours)

Objectif : CI rouge = vrai problème.

1. **Job CI Playwright** dans `.github/workflows/ci.yml` : `npm ci` +
   `playwright install --with-deps chromium` + `npx playwright test` depuis
   `crates/visio-desktop/frontend/`, sur PR vers `main` comme `lint-frontend`.
2. **Réparer les 24 sélecteurs E2E obsolètes** : `chat.spec.ts` (9),
   `device-selection.spec.ts` (8), `screen-share.spec.ts` (6), `home.spec.ts` (1).
   Aligner les testids avec `App.tsx` (ou rétablir les testids perdus côté app). Si un
   sélecteur révèle une régression réelle → fix app, pas fix du test.
3. **i18n renforcé** : étendre `scripts/check-i18n.sh` pour extraire les clés `t('...')`
   utilisées dans les sources et échouer si absentes d'`en.json`. Ajouter les 6 clés
   restantes (`action.add`, `action.back`, `action.close`, `action.remove`, `code`,
   `visio`) × 6 locales, après vérification de leurs usages réels.
4. **Assainissement ESLint** : corriger les vrais warnings (variables inutilisées =
   code mort, `no-empty` sur `catch {}` suspects, `react-hooks/exhaustive-deps`).
   Sortie : `npm run lint` à 0 warning.
5. **`AGENTS.md` à jour** : table CI (job Playwright), section TypeScript (ESLint +
   Prettier existent), commandes E2E.

Critère de sortie : CI verte avec le job Playwright, suite 100 % verte, lint 0 warning,
check-i18n couvrant les clés utilisées.

### Phase 1 — Parcours quotidien sans accroc (1-2 semaines)

Règle : **chaque fix embarque son garde-fou**. Pour chaque flux de la checklist :
scénario E2E de verrouillage écrit d'abord (mock Tauri) ; bug trouvé → test RED →
fix → GREEN. Aucun fix de parcours sans test.

Checklist des flux à auditer et verrouiller :

1. Lancement et état initial (home, historique, meetings)
2. Auth OIDC — fait dans cette session (`auth-required.spec.ts`)
3. Join par URL complète, par slug (multi-instances), par alias, par deep link
4. Lobby (attente, admission, refus, timeout, annulation)
5. Appel : mic/cam/devices (sélection, défaut, changement à chaud)
6. Partage d'écran (picker, démarrage, arrêt, source disparue)
7. Chat (envoi, réception, unread count, réactions)
8. Reconnexion et perte réseau (cache token, états d'erreur, messages utilisateur)
9. Settings et historique (persistance, langue, thème, instances)

Critère de sortie : une journée complète d'usage réel sans contournement.

### Phase 2 — Confort en appel (~1 semaine)

- Remplacer le polling 1 s par les événements core existants (`get_messages`,
  `get_participants`, unread count) — latence chat et CPU. Petits pas, test à chaque pas.
- Qualité vidéo desktop : mesurer fps/CPU actuels, puis évaluer montée en fps et
  allègement du encodage base64. Pas d'optimisation sans mesure avant/après.

### Phase 3 — Structure ciblée (en continu, seulement sur flux verrouillés)

- Extraction de `CallView`, `HomeView`, `SettingsView` de `App.tsx` en modules dédiés.
- Suppression du code mort (ex. getter `pendingOidcInstance`).
- Déduplication des commandes devices `cfg` OS (3 variantes → impl partagée).

## Hors scope explicite (YAGNI)

- Pipeline vidéo desktop natif (réévalué après phase 2, sur mesures).
- Mobile (Android/iOS) : chantier suivant du programme.
- Toute nouvelle fonctionnalité.

## Tests et risques

- **Couverture** : unit (core, déjà bon) / E2E mocké (frontend, verrouillage parcours) /
  E2E réel (framework TS + visio-bot + LiveKit local) pour les flux réseau que le mock
  ne couvre pas (reconnexion, perte de qualité).
- **Risques** : les 24 sélecteurs peuvent révéler de vraies régressions (temps
  supplémentaire) ; le passage polling→événements touche la sync UI (petits pas) ;
  Windows/Linux non vérifiables localement — s'appuyer sur `build-desktop.yml`
  (matrice, workflow_dispatch).

## Critère de succès global

Dogfooding quotidien desktop : une journée d'usage sans contournement, CI verte avec
Playwright, lint 0 warning, suite E2E fiable.
