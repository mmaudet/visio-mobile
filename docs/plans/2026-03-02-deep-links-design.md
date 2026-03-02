# Deep Links Design

## Goal

Allow users to tap a `visio://` URL and have the app open with the room pre-filled, ready to join.

## URL Format

```
visio://meet.numerique.gouv.fr/abc-defg-hij
       └── host ──────────────┘ └── slug ──┘
```

Parsing (all platforms):
1. Extract host from URI authority
2. Extract slug from URI path (strip leading `/`)
3. Validate host against configured Meet instances list
4. If valid: reconstruct `https://{host}/{slug}`, pre-fill HomeScreen
5. If unknown host: show error message ("Unknown instance: {host}")

## Behavior

**Pre-fill + confirmation**: the app opens on HomeScreen with the room URL pre-filled. The user sees their display name, mic/camera settings, and clicks "Join" to connect. No auto-join.

## Settings — Meet Instances

New field in `Settings` struct (visio-core):

```rust
pub meet_instances: Vec<String>  // default: ["meet.numerique.gouv.fr"]
```

New SettingsStore methods:
- `get_meet_instances() -> Vec<String>`
- `set_meet_instances(instances: Vec<String>)`

Exposed via UniFFI UDL + Tauri command.

UI in Settings screen (all platforms):
- Section "Meet instances"
- List of domains, each with a delete button
- Text field + "Add" button at the bottom
- `meet.numerique.gouv.fr` pre-populated, deletable

No DNS validation on add — validation happens at join time via existing `validate_room`.

## Platform Integration

### Android

- `AndroidManifest.xml`: add `<intent-filter>` on MainActivity:
  ```xml
  <intent-filter>
      <action android:name="android.intent.action.VIEW" />
      <category android:name="android.intent.category.DEFAULT" />
      <category android:name="android.intent.category.BROWSABLE" />
      <data android:scheme="visio" />
  </intent-filter>
  ```
- `MainActivity.onCreate()`: parse `intent.data` URI, extract host+slug, pass to AppNavigation
- `onNewIntent()`: same parsing (app already open)
- `HomeScreen`: new optional `deepLinkUrl: String?` param, pre-fills room URL field if non-null

### iOS

- `Info.plist`: add `CFBundleURLTypes` with scheme `visio`
- `VisioMobileApp.swift`: add `.onOpenURL { url in ... }` on WindowGroup
- Parse URL, store in `@Published var deepLinkUrl: String?` on VisioManager
- `HomeView`: observe `manager.deepLinkUrl`, pre-fill room URL field, reset to nil after consumption

### Desktop (Tauri)

- `tauri.conf.json`: add `deep-link` plugin with scheme `visio`
- `Cargo.toml` (visio-desktop): add `tauri-plugin-deep-link` dependency
- `main.rs`: register deep-link plugin
- `App.tsx`: listen to `deep-link://new-url` event, parse URL, pre-fill HomeView
- Handles cold start (URL passed as first event after mount) and warm start (app already open)

### Host Validation (all platforms)

At parse time, load `get_meet_instances()` from settings and check host membership. If not found, display i18n error `deep_link_unknown_instance` with the host name.

## i18n

4 new keys (added to all 6 JSON files):

| Key | English |
|-----|---------|
| `settings_meet_instances` | Meet instances |
| `settings_add_instance` | Add instance |
| `settings_instance_placeholder` | meet.example.com |
| `deep_link_unknown_instance` | Unknown instance: {host} |

Total: 96 + 4 = 100 keys.

## Universal Links / App Links (future)

Not implemented in this phase — requires server-side configuration by the Meet instance admin.

Documentation added to README explaining how to set up:
- **Android App Links**: `.well-known/assetlinks.json` on the Meet server with the app's SHA-256 fingerprint
- **iOS Universal Links**: `.well-known/apple-app-site-association` on the Meet server with the app's team ID and bundle ID

These would allow `https://meet.numerique.gouv.fr/slug` links to open the app directly, in addition to the `visio://` scheme.
