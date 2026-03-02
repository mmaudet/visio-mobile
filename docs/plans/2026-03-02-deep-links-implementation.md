# Deep Links Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `visio://host/slug` deep link support so tapping a link opens the app with the room pre-filled.

**Architecture:** Custom URL scheme `visio://` handled natively on each platform. Settings store gains a `meet_instances` list for host validation. Deep link parsing extracts host+slug, validates host against the list, and pre-fills the HomeScreen. No auto-join — user confirms before connecting.

**Tech Stack:** Rust (visio-core settings), UniFFI UDL, Tauri deep-link plugin, Android intent-filter, iOS CFBundleURLTypes, React/Compose/SwiftUI.

---

### Task 1: Add `meet_instances` to Settings (Rust core)

**Files:**
- Modify: `crates/visio-core/src/settings.rs`

**Step 1: Write failing tests**

Add these tests at the end of the `tests` module in `crates/visio-core/src/settings.rs`:

```rust
#[test]
fn test_default_meet_instances() {
    let s = Settings::default();
    assert_eq!(s.meet_instances, vec!["meet.numerique.gouv.fr".to_string()]);
}

#[test]
fn test_set_meet_instances_persists() {
    let dir = temp_dir();
    let path = dir.path().to_str().unwrap();
    {
        let store = SettingsStore::new(path);
        store.set_meet_instances(vec![
            "meet.numerique.gouv.fr".to_string(),
            "meet.example.com".to_string(),
        ]);
    }
    let store = SettingsStore::new(path);
    assert_eq!(store.get().meet_instances, vec![
        "meet.numerique.gouv.fr".to_string(),
        "meet.example.com".to_string(),
    ]);
}

#[test]
fn test_partial_json_defaults_meet_instances() {
    let dir = temp_dir();
    let path = dir.path().to_str().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"display_name":"Eve"}"#,
    ).unwrap();
    let store = SettingsStore::new(path);
    assert_eq!(store.get().meet_instances, vec!["meet.numerique.gouv.fr".to_string()]);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p visio-core -- test_default_meet_instances test_set_meet_instances_persists test_partial_json_defaults_meet_instances`
Expected: FAIL — `meet_instances` field doesn't exist yet.

**Step 3: Implement**

In `crates/visio-core/src/settings.rs`:

1. Add a default function:
```rust
fn default_meet_instances() -> Vec<String> {
    vec!["meet.numerique.gouv.fr".to_string()]
}
```

2. Add the field to `Settings`:
```rust
#[serde(default = "default_meet_instances")]
pub meet_instances: Vec<String>,
```

3. Add the field to `Default` impl:
```rust
meet_instances: default_meet_instances(),
```

4. Add methods to `SettingsStore`:
```rust
pub fn get_meet_instances(&self) -> Vec<String> {
    self.settings.lock().unwrap().meet_instances.clone()
}

pub fn set_meet_instances(&self, instances: Vec<String>) {
    self.settings.lock().unwrap().meet_instances = instances;
    self.save();
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p visio-core`
Expected: ALL pass (including existing tests — serde defaults handle missing field).

**Step 5: Commit**

```bash
git add crates/visio-core/src/settings.rs
git commit -m "feat(core): add meet_instances to Settings with default [meet.numerique.gouv.fr]"
```

---

### Task 2: Expose `meet_instances` via FFI

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl`
- Modify: `crates/visio-ffi/src/lib.rs`

**Step 1: Update UDL**

In `crates/visio-ffi/src/visio.udl`, add `meet_instances` to the `Settings` dictionary:

```
dictionary Settings {
    string? display_name;
    string? language;
    boolean mic_enabled_on_join;
    boolean camera_enabled_on_join;
    string theme;
    sequence<string> meet_instances;
};
```

Add two new methods to the `VisioClient` interface, after `set_theme`:

```
sequence<string> get_meet_instances();
void set_meet_instances(sequence<string> instances);
```

**Step 2: Update lib.rs**

In `crates/visio-ffi/src/lib.rs`, find the FFI `Settings` struct and add:
```rust
pub meet_instances: Vec<String>,
```

In the `get_settings` method, add:
```rust
meet_instances: s.meet_instances,
```

Add two new methods to `VisioClient` impl:
```rust
pub fn get_meet_instances(&self) -> Vec<String> {
    self.settings.get_meet_instances()
}

pub fn set_meet_instances(&self, instances: Vec<String>) {
    self.settings.set_meet_instances(instances);
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p visio-ffi`
Expected: compiles without errors.

**Step 4: Commit**

```bash
git add crates/visio-ffi/src/visio.udl crates/visio-ffi/src/lib.rs
git commit -m "feat(ffi): expose meet_instances via UniFFI (get/set)"
```

---

### Task 3: Add Tauri commands for `meet_instances`

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs`

**Step 1: Add two new Tauri commands**

After `set_theme` command in `crates/visio-desktop/src/lib.rs`:

```rust
#[tauri::command]
fn get_meet_instances(state: tauri::State<'_, VisioState>) -> Result<Vec<String>, String> {
    Ok(state.settings.get_meet_instances())
}

#[tauri::command]
fn set_meet_instances(state: tauri::State<'_, VisioState>, instances: Vec<String>) {
    state.settings.set_meet_instances(instances);
}
```

**Step 2: Register commands in invoke_handler**

Add `get_meet_instances` and `set_meet_instances` to the `generate_handler!` macro call.

**Step 3: Verify it compiles**

Run: `cargo build -p visio-desktop`
Expected: compiles without errors.

**Step 4: Commit**

```bash
git add crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): add get/set_meet_instances Tauri commands"
```

---

### Task 4: Add i18n keys

**Files:**
- Modify: `i18n/en.json`, `i18n/fr.json`, `i18n/de.json`, `i18n/es.json`, `i18n/it.json`, `i18n/nl.json`

**Step 1: Add 4 new keys to each file**

At the end of each JSON file (before the closing `}`), add:

**en.json:**
```json
"settings.meetInstances": "Meet instances",
"settings.addInstance": "Add instance",
"settings.instancePlaceholder": "meet.example.com",
"deepLink.unknownInstance": "Unknown instance: {host}"
```

**fr.json:**
```json
"settings.meetInstances": "Instances Meet",
"settings.addInstance": "Ajouter une instance",
"settings.instancePlaceholder": "meet.example.com",
"deepLink.unknownInstance": "Instance inconnue : {host}"
```

**de.json:**
```json
"settings.meetInstances": "Meet-Instanzen",
"settings.addInstance": "Instanz hinzufügen",
"settings.instancePlaceholder": "meet.example.com",
"deepLink.unknownInstance": "Unbekannte Instanz: {host}"
```

**es.json:**
```json
"settings.meetInstances": "Instancias Meet",
"settings.addInstance": "Añadir instancia",
"settings.instancePlaceholder": "meet.example.com",
"deepLink.unknownInstance": "Instancia desconocida: {host}"
```

**it.json:**
```json
"settings.meetInstances": "Istanze Meet",
"settings.addInstance": "Aggiungi istanza",
"settings.instancePlaceholder": "meet.example.com",
"deepLink.unknownInstance": "Istanza sconosciuta: {host}"
```

**nl.json:**
```json
"settings.meetInstances": "Meet-instanties",
"settings.addInstance": "Instantie toevoegen",
"settings.instancePlaceholder": "meet.example.com",
"deepLink.unknownInstance": "Onbekende instantie: {host}"
```

**Step 2: Commit**

```bash
git add i18n/*.json
git commit -m "feat(i18n): add deep link and meet instances keys in 6 languages"
```

---

### Task 5: Desktop — Meet instances UI in Settings

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Add state and load/save for meet_instances**

In the `App` component, add state:
```typescript
const [meetInstances, setMeetInstances] = useState<string[]>(["meet.numerique.gouv.fr"]);
```

In the `useEffect` that calls `get_settings`, also load instances:
```typescript
invoke<string[]>("get_meet_instances").then(setMeetInstances).catch(() => {});
```

**Step 2: Add MeetInstances section to SettingsModal**

Find the SettingsModal component. Add a new section after the language section:

```tsx
<div className="settings-section">
  <h3>{t("settings.meetInstances")}</h3>
  {meetInstances.map((inst, i) => (
    <div key={i} className="instance-row">
      <span>{inst}</span>
      <button className="btn-icon" onClick={() => {
        const next = meetInstances.filter((_, j) => j !== i);
        setMeetInstances(next);
        invoke("set_meet_instances", { instances: next });
      }}><RiCloseLine size={16} /></button>
    </div>
  ))}
  <div className="instance-add-row">
    <input
      id="newInstance"
      type="text"
      placeholder={t("settings.instancePlaceholder")}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          const val = (e.target as HTMLInputElement).value.trim().toLowerCase();
          if (val && !meetInstances.includes(val)) {
            const next = [...meetInstances, val];
            setMeetInstances(next);
            invoke("set_meet_instances", { instances: next });
            (e.target as HTMLInputElement).value = "";
          }
        }
      }}
    />
  </div>
</div>
```

**Step 3: Add CSS for instance rows**

In `App.css`, add:
```css
.instance-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  background: var(--surface);
  border-radius: 8px;
  margin-bottom: 4px;
}
.instance-add-row {
  margin-top: 8px;
}
.instance-add-row input {
  width: 100%;
}
```

**Step 4: Verify**

Run: `cd crates/visio-desktop/frontend && npm run build`
Expected: builds without errors.

**Step 5: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): add Meet instances management in Settings"
```

---

### Task 6: Desktop — Deep link via Tauri plugin

**Files:**
- Modify: `crates/visio-desktop/Cargo.toml`
- Modify: `crates/visio-desktop/tauri.conf.json`
- Modify: `crates/visio-desktop/src/lib.rs`
- Modify: `crates/visio-desktop/frontend/src/App.tsx`
- Modify: `crates/visio-desktop/frontend/package.json` (add `@tauri-apps/plugin-deep-link`)

**Step 1: Add Tauri deep-link plugin dependency**

In `crates/visio-desktop/Cargo.toml`, add to `[dependencies]`:
```toml
tauri-plugin-deep-link = "2"
```

In `crates/visio-desktop/frontend/`, run:
```bash
npm install @tauri-apps/plugin-deep-link
```

**Step 2: Configure tauri.conf.json**

In `crates/visio-desktop/tauri.conf.json`, add inside the top-level object:
```json
"plugins": {
  "deep-link": {
    "desktop": {
      "schemes": ["visio"]
    }
  }
}
```

Also add `"deep-link"` to `app.security.capabilities` if needed, or ensure the `"core:default"` permission covers it.

**Step 3: Register plugin in Rust**

In `crates/visio-desktop/src/lib.rs`, in the `run()` function, add before `.invoke_handler(...)`:
```rust
.plugin(tauri_plugin_deep_link::init())
```

**Step 4: Listen for deep links in React**

In `crates/visio-desktop/frontend/src/App.tsx`, add import:
```typescript
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
```

In the `App` component, add a `useEffect` that listens for deep links:
```typescript
useEffect(() => {
  const unlisten = onOpenUrl((urls: string[]) => {
    if (urls.length === 0) return;
    const url = urls[0];
    // Parse visio://host/slug
    try {
      const parsed = new URL(url);
      if (parsed.protocol !== "visio:") return;
      const host = parsed.hostname;
      const slug = parsed.pathname.replace(/^\//, "");
      if (!host || !slug) return;

      // Validate host against meet_instances
      invoke<string[]>("get_meet_instances").then((instances) => {
        if (instances.includes(host)) {
          setView("home");
          // Set meetUrl will be handled by passing it down
          setDeepLinkUrl(`https://${host}/${slug}`);
        } else {
          // Show error — unknown instance
          setDeepLinkError(t("deepLink.unknownInstance").replace("{host}", host));
        }
      });
    } catch { /* ignore malformed URLs */ }
  });
  return () => { unlisten.then((fn) => fn()); };
}, []);
```

Add state for deep link:
```typescript
const [deepLinkUrl, setDeepLinkUrl] = useState<string | null>(null);
const [deepLinkError, setDeepLinkError] = useState<string | null>(null);
```

Pass `deepLinkUrl` to `HomeView` as a prop. In `HomeView`, consume it:
```typescript
useEffect(() => {
  if (deepLinkUrl) {
    setMeetUrl(deepLinkUrl);
    onDeepLinkConsumed(); // clears deepLinkUrl in parent
  }
}, [deepLinkUrl]);
```

Show `deepLinkError` as a toast/banner on the home view if non-null, with a dismiss.

**Step 5: Verify**

Run: `cd crates/visio-desktop && cargo build`
Run: `cd crates/visio-desktop/frontend && npm run build`
Expected: both compile without errors.

**Step 6: Commit**

```bash
git add crates/visio-desktop/Cargo.toml crates/visio-desktop/tauri.conf.json \
  crates/visio-desktop/src/lib.rs crates/visio-desktop/frontend/src/App.tsx \
  crates/visio-desktop/frontend/package.json crates/visio-desktop/frontend/package-lock.json
git commit -m "feat(desktop): add visio:// deep link support via Tauri plugin"
```

---

### Task 7: Android — Deep link intent filter + parsing

**Files:**
- Modify: `android/app/src/main/AndroidManifest.xml`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/navigation/AppNavigation.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`

**Step 1: Add intent filter to AndroidManifest.xml**

In `android/app/src/main/AndroidManifest.xml`, inside the `<activity>` tag, add a second `<intent-filter>` after the LAUNCHER one:

```xml
<intent-filter>
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="visio" />
</intent-filter>
```

Also add `android:launchMode="singleTask"` to the `<activity>` tag so `onNewIntent` fires when the app is already running.

**Step 2: Parse deep link in MainActivity**

In `MainActivity.kt`, add a helper to parse the intent:

```kotlin
private fun parseDeepLink(intent: Intent?): String? {
    val uri = intent?.data ?: return null
    if (uri.scheme != "visio") return null
    val host = uri.host ?: return null
    val slug = uri.path?.trimStart('/') ?: return null
    if (host.isBlank() || slug.isBlank()) return null

    val instances = VisioManager.client.getMeetInstances()
    return if (instances.contains(host)) {
        "https://$host/$slug"
    } else {
        null // unknown instance — could set error state
    }
}
```

In `onCreate`, after `enableEdgeToEdge()`:
```kotlin
val deepLinkUrl = parseDeepLink(intent)
if (deepLinkUrl != null) {
    VisioManager.pendingDeepLink = deepLinkUrl
}
```

Override `onNewIntent`:
```kotlin
override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    val deepLinkUrl = parseDeepLink(intent)
    if (deepLinkUrl != null) {
        VisioManager.pendingDeepLink = deepLinkUrl
    }
}
```

**Step 3: Add pendingDeepLink state to VisioManager**

In `VisioManager.kt`, add:
```kotlin
var pendingDeepLink: String? by mutableStateOf(null)
```

**Step 4: Consume deep link in HomeScreen**

In `HomeScreen.kt`, add a `LaunchedEffect` that watches `VisioManager.pendingDeepLink`:

```kotlin
LaunchedEffect(VisioManager.pendingDeepLink) {
    val link = VisioManager.pendingDeepLink
    if (link != null) {
        roomUrl = link
        VisioManager.pendingDeepLink = null
    }
}
```

This sets `roomUrl` which triggers the existing validation `LaunchedEffect(roomUrl)`.

**Step 5: Verify**

Run: `cd android && ./gradlew assembleDebug`
Expected: builds without errors.

Test manually: `adb shell am start -a android.intent.action.VIEW -d "visio://meet.numerique.gouv.fr/abc-defg-hij"`
Expected: app opens, HomeScreen shows `https://meet.numerique.gouv.fr/abc-defg-hij` in room URL field.

**Step 6: Commit**

```bash
git add android/
git commit -m "feat(android): add visio:// deep link support with intent filter"
```

---

### Task 8: iOS — Deep link via URL scheme

**Files:**
- Modify: `ios/VisioMobile/Info.plist`
- Modify: `ios/VisioMobile/VisioMobileApp.swift`
- Modify: `ios/VisioMobile/VisioManager.swift`
- Modify: `ios/VisioMobile/Views/HomeView.swift`

**Step 1: Register URL scheme in Info.plist**

In `ios/VisioMobile/Info.plist`, add inside the top-level `<dict>`:

```xml
<key>CFBundleURLTypes</key>
<array>
    <dict>
        <key>CFBundleURLName</key>
        <string>io.visio.mobile</string>
        <key>CFBundleURLSchemes</key>
        <array>
            <string>visio</string>
        </array>
    </dict>
</array>
```

**Step 2: Add deepLinkUrl to VisioManager**

In `ios/VisioMobile/VisioManager.swift`, add a published property:
```swift
@Published var pendingDeepLink: String? = nil
```

**Step 3: Handle onOpenURL in VisioMobileApp**

In `ios/VisioMobile/VisioMobileApp.swift`, add `.onOpenURL` to the `WindowGroup`:

```swift
WindowGroup {
    NavigationStack {
        HomeView()
    }
    .environmentObject(manager)
    .preferredColorScheme(manager.currentTheme == "dark" ? .dark : .light)
    .onOpenURL { url in
        guard url.scheme == "visio",
              let host = url.host,
              let slug = url.path.split(separator: "/").first
        else { return }

        let instances = manager.client.getMeetInstances()
        if instances.contains(host) {
            manager.pendingDeepLink = "https://\(host)/\(slug)"
        }
    }
}
```

**Step 4: Consume deep link in HomeView**

In `ios/VisioMobile/Views/HomeView.swift`, add an `.onChange` modifier that watches `manager.pendingDeepLink`:

```swift
.onChange(of: manager.pendingDeepLink) { newValue in
    if let link = newValue {
        roomURL = link
        manager.pendingDeepLink = nil
    }
}
```

This sets `roomURL` which triggers the existing `.task(id: roomURL)` validation.

**Step 5: Verify**

Build in Xcode. Test with:
```bash
xcrun simctl openurl booted "visio://meet.numerique.gouv.fr/abc-defg-hij"
```
Expected: app opens, HomeView shows `https://meet.numerique.gouv.fr/abc-defg-hij` in room URL field.

**Step 6: Commit**

```bash
git add ios/
git commit -m "feat(ios): add visio:// deep link support via CFBundleURLTypes"
```

---

### Task 9: Android — Meet instances UI in Settings

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/SettingsScreen.kt`

**Step 1: Add instances state**

In `SettingsScreen.kt`, add state:
```kotlin
var meetInstances by remember { mutableStateOf(listOf("meet.numerique.gouv.fr")) }
var newInstance by remember { mutableStateOf("") }
```

In the `LaunchedEffect(Unit)` that loads settings, add:
```kotlin
meetInstances = VisioManager.client.getMeetInstances()
```

**Step 2: Add Meet instances section**

After the Language section and before the Save button, add:

```kotlin
SectionHeader(Strings.t("settings.meetInstances", lang), isDark)
meetInstances.forEachIndexed { index, instance ->
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                if (isDark) VisioColors.PrimaryDark100 else VisioColors.LightSurfaceVariant,
                RoundedCornerShape(12.dp)
            )
            .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = instance,
            style = MaterialTheme.typography.bodyLarge,
            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
            modifier = Modifier.weight(1f)
        )
        IconButton(onClick = {
            meetInstances = meetInstances.filterIndexed { i, _ -> i != index }
        }) {
            Icon(
                painter = painterResource(R.drawable.ri_close_line),
                contentDescription = "Remove",
                tint = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary
            )
        }
    }
}
TextField(
    value = newInstance,
    onValueChange = { newInstance = it },
    placeholder = { Text(Strings.t("settings.instancePlaceholder", lang), color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary) },
    singleLine = true,
    modifier = Modifier.fillMaxWidth(),
    trailingIcon = {
        if (newInstance.isNotBlank()) {
            IconButton(onClick = {
                val trimmed = newInstance.trim().lowercase()
                if (trimmed.isNotBlank() && trimmed !in meetInstances) {
                    meetInstances = meetInstances + trimmed
                    newInstance = ""
                }
            }) {
                Icon(
                    painter = painterResource(R.drawable.ri_add_line),
                    contentDescription = Strings.t("settings.addInstance", lang),
                    tint = VisioColors.Primary500
                )
            }
        }
    },
    colors = TextFieldDefaults.colors(
        focusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
        unfocusedContainerColor = MaterialTheme.colorScheme.surfaceVariant,
        cursorColor = VisioColors.Primary500,
        focusedTextColor = MaterialTheme.colorScheme.onSurface,
        unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent
    ),
    shape = RoundedCornerShape(12.dp)
)
```

**Step 3: Save instances in the Save button handler**

In the `onClick` of the Save button, add:
```kotlin
VisioManager.client.setMeetInstances(meetInstances)
```

**Step 4: Add ri_add_line icon**

Check if `ri_add_line.xml` exists in `android/app/src/main/res/drawable/`. If not, create it (standard Remixicon add-line SVG converted to Android vector drawable):

```xml
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:fillColor="#000000"
        android:pathData="M11,11V5h2v6h6v2h-6v6h-2v-6H5v-2z"/>
</vector>
```

**Step 5: Verify**

Run: `cd android && ./gradlew assembleDebug`
Expected: builds.

**Step 6: Commit**

```bash
git add android/
git commit -m "feat(android): add Meet instances management in Settings"
```

---

### Task 10: iOS — Meet instances UI in Settings

**Files:**
- Modify: `ios/VisioMobile/Views/SettingsView.swift`

**Step 1: Add state**

In `SettingsView.swift`, add:
```swift
@State private var meetInstances: [String] = ["meet.numerique.gouv.fr"]
@State private var newInstance: String = ""
```

In the `load()` function, add:
```swift
meetInstances = manager.client.getMeetInstances()
```

**Step 2: Add section to Form**

After the Language section, add:

```swift
Section(Strings.t("settings.meetInstances", lang: lang)) {
    ForEach(meetInstances, id: \.self) { instance in
        HStack {
            Text(instance)
            Spacer()
            Button {
                meetInstances.removeAll { $0 == instance }
            } label: {
                Image(systemName: "minus.circle.fill")
                    .foregroundStyle(.red)
            }
        }
    }
    HStack {
        TextField(Strings.t("settings.instancePlaceholder", lang: lang), text: $newInstance)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(.URL)
        Button {
            let trimmed = newInstance.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if !trimmed.isEmpty && !meetInstances.contains(trimmed) {
                meetInstances.append(trimmed)
                newInstance = ""
            }
        } label: {
            Image(systemName: "plus.circle.fill")
                .foregroundStyle(VisioColors.primary500)
        }
        .disabled(newInstance.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }
}
```

**Step 3: Save in save() function**

In the `save()` function, add:
```swift
manager.client.setMeetInstances(instances: meetInstances)
```

**Step 4: Verify**

Build in Xcode.

**Step 5: Commit**

```bash
git add ios/
git commit -m "feat(ios): add Meet instances management in Settings"
```

---

### Task 11: Update README with Deep Links section

**Files:**
- Modify: `README.md`

**Step 1: Add Deep Links section**

After the "Internationalization" section and before "Running tests", add:

```markdown
## Deep Links

The app registers the `visio://` URL scheme on all platforms. Tapping a `visio://` link opens the app with the room pre-filled on the home screen.

**Format:** `visio://host/slug` — for example: `visio://meet.numerique.gouv.fr/abc-defg-hij`

The host must match one of the configured Meet instances (managed in Settings). By default, `meet.numerique.gouv.fr` is pre-configured. Unknown hosts are rejected with an error message.

**Testing deep links:**
- **Android:** `adb shell am start -a android.intent.action.VIEW -d "visio://meet.numerique.gouv.fr/abc-defg-hij"`
- **iOS:** `xcrun simctl openurl booted "visio://meet.numerique.gouv.fr/abc-defg-hij"`
- **Desktop:** `open "visio://meet.numerique.gouv.fr/abc-defg-hij"` (macOS)

### Universal Links / App Links (optional, server-side)

For HTTPS links (e.g., `https://meet.numerique.gouv.fr/slug`) to open the app directly instead of the browser, the Meet server admin must host verification files:

**Android App Links** — create `https://meet.example.com/.well-known/assetlinks.json`:
```json
[{
  "relation": ["delegate_permission/common.handle_all_urls"],
  "target": {
    "namespace": "android_app",
    "package_name": "io.visio.mobile",
    "sha256_cert_fingerprints": ["<YOUR_APP_SHA256>"]
  }
}]
```

**iOS Universal Links** — create `https://meet.example.com/.well-known/apple-app-site-association`:
```json
{
  "applinks": {
    "apps": [],
    "details": [{
      "appID": "<TEAM_ID>.io.visio.mobile",
      "paths": ["/*"]
    }]
  }
}
```

These are not required for the `visio://` scheme to work — they enable the additional HTTPS link interception.
```

**Step 2: Update "What works" section**

Add to the Core section:
```
- Deep links: `visio://host/slug` opens the app with room pre-filled (all platforms)
- Configurable Meet instances list in Settings
```

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add Deep Links section to README with Universal Links guidance"
```
