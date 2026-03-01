# Participant List Panel — Design

## Overview

Add a participant list bottom sheet to Android (Kotlin/Compose) and iOS (SwiftUI).
Accessible via a new button in the call control bar. Pure UI work — no Rust changes needed.

## Trigger

- New button in control bar between chat and hangup
- Android: `ri_group_line` icon, iOS: `person.2` SF Symbol
- Badge showing participant count (e.g. "3")
- Tap toggles the bottom sheet open/closed

## Bottom Sheet

- **Android**: `ModalBottomSheet` (Material3), ~60% screen height
- **iOS**: `.sheet` with `presentationDetents([.medium, .large])`, starts at `.medium`
- Header: "Participants (N)" + close button (X)
- Scrollable list below header

## Participant Row

```
┌──────────────────────────────────────────┐
│  [AV]  Nom du participant    🎤 📷 ✋ ▮▮▮│
└──────────────────────────────────────────┘
```

- **Avatar**: 40dp circle with colored initials (same hash algorithm as ParticipantTile)
- **Name**: `name ?? identity`, single line with ellipsis. Local participant appends "(Vous)"
- **Status icons** (trailing):
  - Mic: `ri_mic_off_line` / `mic.slash` in red if muted, hidden otherwise
  - Camera: `ri_video_off_line` / `video.slash` in red if off, hidden otherwise
  - Hand raised: yellow pill with queue position (1, 2, 3…), hidden if not raised
  - Connection quality: 3-bar indicator (green/yellow/orange/red)

## Sort Order

1. Local participant first
2. Hand raised participants (sorted by queue position ascending)
3. Remaining participants alphabetically by name/identity

## Data Sources (already available)

- `VisioManager.participants` — list of ParticipantInfo (sid, name, isMuted, hasVideo, connectionQuality)
- `VisioManager.activeSpeakers` — list of active speaker sids
- `VisioManager.handRaisedMap` — map of sid to queue position
- Local participant identified by comparing sid with `client.localParticipantSid()`

## Files

- `android/app/src/main/kotlin/io/visio/mobile/ui/ParticipantListSheet.kt` (new)
- `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt` (add button + sheet state)
- `ios/VisioMobile/Views/ParticipantListSheet.swift` (new)
- `ios/VisioMobile/Views/CallView.swift` (add button + sheet state)
