import SwiftUI
import AVFoundation
import visioFFI

// MARK: - Display Item Types

enum FocusSource: Equatable {
    case camera
    case screenShare
}

struct FocusItem: Equatable {
    let participantSid: String
    let source: FocusSource
}

struct DisplayItem: Identifiable, Equatable {
    let id: String
    let participant: ParticipantInfo
    let source: FocusSource
    let trackSid: String?
    let isScreenShare: Bool

    var label: String {
        participant.name ?? participant.identity
    }

    static func == (lhs: DisplayItem, rhs: DisplayItem) -> Bool {
        lhs.id == rhs.id
    }
}

func buildDisplayItems(_ participants: [ParticipantInfo]) -> [DisplayItem] {
    var items: [DisplayItem] = []
    for p in participants {
        items.append(DisplayItem(
            id: "\(p.sid)-camera",
            participant: p,
            source: .camera,
            trackSid: p.videoTrackSid,
            isScreenShare: false
        ))
        if p.hasScreenShare, let ssSid = p.screenShareTrackSid {
            items.append(DisplayItem(
                id: "\(p.sid)-screen",
                participant: p,
                source: .screenShare,
                trackSid: ssSid,
                isScreenShare: true
            ))
        }
    }
    return items
}

struct CallView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase

    let roomURL: String
    let displayName: String
    var roomDisplayName: String? = nil

    @State private var showChat: Bool = false
    @State private var showAudioDevices: Bool = false
    @State private var showInCallSettings: Bool = false
    @State private var inCallSettingsTab: Int = 0
    @State private var showParticipantList: Bool = false
    @State private var focusedItem: FocusItem? = nil
    // lobbyNotificationDismissTask removed — banner is now persistent while participants wait
    @State private var showOverflow: Bool = false
    @State private var showReactionPicker: Bool = false
    @State private var adaptiveModeOverride: AdaptiveMode? = nil
    @State private var showAdaptiveModePicker: Bool = false
    /// Tracks whether the user manually pinned a participant (disables active speaker auto-focus).
    @State private var userPinned: Bool = false
    @State private var layoutState = LayoutState()

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }
    /// When adaptive mode is disabled in settings, force Office mode everywhere.
    private var effectiveAdaptiveMode: AdaptiveMode {
        manager.client.isAdaptiveModeEnabled() ? manager.adaptiveMode : .office
    }
    private var isAdaptiveModeEnabled: Bool { manager.client.isAdaptiveModeEnabled() }

    var body: some View {
        ZStack {
            (effectiveAdaptiveMode == .office
                ? VisioColors.background(dark: isDark)
                : Color.black
            ).ignoresSafeArea()

            VStack(spacing: 0) {
                // Lobby join notification banner
                lobbyNotificationBanner

                // Connection state banner
                connectionBanner

                // Bandwidth degradation banner
                bandwidthBanner

                // Error banner
                if let error = manager.errorMessage {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.white)
                        .padding(8)
                        .frame(maxWidth: .infinity)
                        .background(VisioColors.error500)
                }

                // Main content area: video grid, waiting for host, or waiting for participants
                ZStack {
                    if case .waitingForHost = manager.connectionState {
                        VStack(spacing: 24) {
                            Spacer()
                            ProgressView()
                                .scaleEffect(1.5)
                                .tint(isDark ? .white : VisioColors.primary500)
                            Text(Strings.t("lobby.waiting", lang: lang))
                                .font(.title2)
                                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                            Text(Strings.t("lobby.waitingDesc", lang: lang))
                                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                            Button(action: {
                                manager.cancelLobby()
                                manager.disconnect()
                                CallKitManager.shared.reportCallEnded()
                                dismiss()
                            }) {
                                Text(Strings.t("lobby.cancel", lang: lang))
                            }
                            .buttonStyle(.bordered)
                            Spacer()
                        }
                        .padding()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    } else if manager.participants.isEmpty {
                        VStack(spacing: 12) {
                            Spacer()
                            ProgressView()
                                .tint(isDark ? .white : VisioColors.primary500)
                            Text(Strings.t("call.waiting", lang: lang))
                                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                            Spacer()
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    } else {
                        // Unified layout engine
                        let localSid = manager.participants.first?.sid ?? ""
                        let screenShareFocus: FocusItem? = manager.lastScreenShareParticipantSid.map {
                            FocusItem(participantSid: $0, source: .screenShare)
                        }
                        let pinnedFocus: FocusItem? = userPinned ? focusedItem : nil
                        let (layoutDecision, _) = computeLayout(
                            participants: manager.participants,
                            activeSpeakers: manager.activeSpeakers,
                            pinnedItem: pinnedFocus,
                            screenShare: screenShareFocus,
                            adaptiveMode: effectiveAdaptiveMode,
                            localParticipantSid: localSid,
                            previousState: layoutState,
                            nowMs: Date().timeIntervalSince1970 * 1000
                        )

                        Group {
                            if effectiveAdaptiveMode == .car {
                                carAudioOnlyView(speaker: layoutDecision.mainTile?.participant)
                            } else if effectiveAdaptiveMode == .pedestrian {
                                pedestrianSingleTile(speaker: layoutDecision.mainTile?.participant)
                            } else if layoutDecision.mode == .focus, let main = layoutDecision.mainTile {
                                focusLayout(focused: main, allItems: [main] + layoutDecision.secondaryTiles, pinnedIndicatorSid: layoutDecision.pinnedIndicatorSid, speakerIndicatorSid: layoutDecision.speakerIndicatorSid)
                            } else {
                                gridLayout
                            }
                        }
                        .accessibilityIdentifier("layout-mode:\(layoutDecision.mode == .focus ? "FOCUS" : "GRID")")
                    }

                    // Reaction overlay
                    ReactionOverlay(reactions: manager.reactions)

                    // Persistent adaptive mode indicator (tappable to override)
                    VStack {
                        HStack {
                            Spacer()
                            if isAdaptiveModeEnabled { VStack(alignment: .trailing, spacing: 4) {
                                Button {
                                    withAnimation(.easeInOut(duration: 0.2)) {
                                        showAdaptiveModePicker.toggle()
                                    }
                                } label: {
                                    HStack(spacing: 4) {
                                        Image(systemName: modeIcon(effectiveAdaptiveMode))
                                        Text(modeLabel(effectiveAdaptiveMode))
                                    }
                                    .font(.caption2)
                                    .foregroundColor(.white)
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(Color.black.opacity(0.5))
                                    .cornerRadius(12)
                                }
                                .padding(8)

                                if showAdaptiveModePicker {
                                    VStack(alignment: .leading, spacing: 6) {
                                        Text(Strings.t("adaptive.override", lang: lang))
                                            .font(.system(size: 11, weight: .medium))
                                            .foregroundStyle(.white)
                                        HStack(spacing: 6) {
                                            let modeOptions: [(AdaptiveMode?, String)] = [
                                                (nil, Strings.t("adaptive.auto", lang: lang)),
                                                (.office, Strings.t("adaptive.office", lang: lang)),
                                                (.pedestrian, Strings.t("adaptive.pedestrian", lang: lang)),
                                                (.car, Strings.t("adaptive.car", lang: lang)),
                                            ]
                                            ForEach(Array(modeOptions.enumerated()), id: \.offset) { _, option in
                                                let (mode, label) = option
                                                let isSelected = mode == adaptiveModeOverride
                                                Button {
                                                    adaptiveModeOverride = mode
                                                    manager.client.setAdaptiveModeOverride(mode: mode)
                                                    withAnimation(.easeInOut(duration: 0.2)) {
                                                        showAdaptiveModePicker = false
                                                    }
                                                } label: {
                                                    Text(label)
                                                        .font(.system(size: 11, weight: isSelected ? .bold : .regular))
                                                        .foregroundStyle(isSelected ? .black : .white)
                                                        .padding(.horizontal, 8)
                                                        .padding(.vertical, 5)
                                                        .background(isSelected ? VisioColors.primary500 : VisioColors.primaryDark100)
                                                        .clipShape(Capsule())
                                                }
                                            }
                                        }
                                    }
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 8)
                                    .background(Color.black.opacity(0.8))
                                    .clipShape(RoundedRectangle(cornerRadius: 12))
                                    .padding(.trailing, 8)
                                    .transition(.opacity.combined(with: .scale(scale: 0.9, anchor: .topTrailing)))
                                }
                            } }
                        }
                        Spacer()
                    }
                }

                // Control bar (hidden when waiting for host)
                if case .waitingForHost = manager.connectionState {
                    EmptyView()
                } else {
                    controlBar
                }
            }
        }
        .navigationTitle(roomDisplayName ?? Strings.t("call.title", lang: lang))
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(true)
        .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
        .toolbarBackground(VisioColors.surface(dark: isDark), for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        .fullScreenCover(isPresented: $showChat) {
            NavigationStack {
                ChatView()
                    .environmentObject(manager)
            }
            .onAppear { manager.setChatOpen(true) }
            .onDisappear { manager.setChatOpen(false) }
        }
        .sheet(isPresented: $showParticipantList) {
            ParticipantListSheet()
                .environmentObject(manager)
                .presentationDetents([.medium, .large])
        }
        .sheet(isPresented: $showAudioDevices) {
            AudioDeviceSheet()
                .environmentObject(manager)
                .presentationDetents([.medium])
        }
        .sheet(isPresented: $showInCallSettings) {
            InCallSettingsSheet(roomURL: roomURL, selectedTab: inCallSettingsTab, roomDisplayName: roomDisplayName)
                .environmentObject(manager)
                .presentationDetents([.medium, .large])
        }
        .onAppear {
            // Only connect if not already connected (PreJoinView already connects)
            if case .disconnected = manager.connectionState {
                let name = displayName.isEmpty ? nil : displayName
                var connectURL = roomURL
                if let rdName = roomDisplayName, !rdName.isEmpty {
                    var allowed = CharacterSet.urlQueryAllowed
                    allowed.remove(charactersIn: " +&=")
                    let encoded = rdName.addingPercentEncoding(withAllowedCharacters: allowed) ?? rdName
                    let separator = connectURL.contains("?") ? "&" : "?"
                    connectURL += "\(separator)visio=\(encoded)"
                }
                manager.connect(url: connectURL, username: name)
            }
            CallKitManager.shared.reportCallStarted(roomName: roomURL)
            UIApplication.shared.isIdleTimerDisabled = true
            PiPManager.shared.setup()
        }
        .onDisappear {
            manager.stopAudioPlayout()
            UIApplication.shared.isIdleTimerDisabled = false
            PiPManager.shared.tearDown()
        }
        .onChange(of: scenePhase) {
            if scenePhase == .background {
                if case .connected = manager.connectionState {
                    PiPManager.shared.startIfNeeded()
                }
            } else if scenePhase == .active {
                PiPManager.shared.stop()
            }
        }
        .onChange(of: manager.lobbyDenied) {
            if manager.lobbyDenied {
                manager.lobbyDenied = false
                manager.disconnect()
                CallKitManager.shared.reportCallEnded()
                dismiss()
            }
        }
        .onChange(of: manager.lastScreenShareParticipantSid) {
            if let sid = manager.lastScreenShareParticipantSid {
                withAnimation(.easeInOut(duration: 0.2)) {
                    focusedItem = FocusItem(participantSid: sid, source: .screenShare)
                }
            }
        }
        .onChange(of: manager.participants) {
            if let fi = focusedItem, fi.source == .screenShare {
                let p = manager.participants.first(where: { $0.sid == fi.participantSid })
                if p?.hasScreenShare != true {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        userPinned = false
                        focusedItem = nil
                    }
                }
            }
            // Clear pin if the focused participant left the room
            if let fi = focusedItem {
                let stillPresent = manager.participants.contains(where: { $0.sid == fi.participantSid })
                if !stillPresent {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        userPinned = false
                        focusedItem = nil
                    }
                }
            }
        }
        .onChange(of: manager.activeSpeakers) {
            updateLayout()
        }
        .onChange(of: manager.participants.count) {
            updateLayout()
        }
        .task {
            guard let params = manager.pendingTestConnect else { return }
            manager.pendingTestConnect = nil

            // Connect (audio playout starts automatically after connection)
            manager.connectWithToken(livekitUrl: params.livekitUrl, token: params.token)

            let hasMediaFile = params.mediaFile != nil && FileManager.default.fileExists(atPath: params.mediaFile!)

            // Auto-chat messages (turn-based)
            let chatMessages: [(Int, String)] = [
                (3, "iOS joined the room!"),
                (75, "iOS: my turn to speak!"),
                (85, "iOS: mid-turn check-in"),
                (100, "iOS: everyone speaking together!"),
            ]
            for (delay, text) in chatMessages {
                Task {
                    try? await Task.sleep(nanoseconds: UInt64(delay) * 1_000_000_000)
                    manager.sendMessage(text)
                }
            }

            // Helpers for starting/stopping media capture
            @MainActor func startMedia() {
                if hasMediaFile {
                    manager.startMediaFileCapture(params.mediaFile!)
                } else {
                    manager.startSyntheticAudio()
                }
            }
            @MainActor func stopMedia() {
                if hasMediaFile {
                    manager.stopMediaFileCapture()
                } else {
                    manager.stopSyntheticAudio()
                }
            }

            // Turn-based speaking: iOS speaks at 75-100s, muted otherwise (except warmup 0-5s and final 100-120s)
            Task {
                // 5s: mute mic+cam (bot's turn)
                try? await Task.sleep(nanoseconds: 5 * 1_000_000_000)
                print("[TURN] iOS muted (bot's turn)")
                manager.setMicEnabled(false)
                stopMedia()
                manager.setCameraEnabled(false)
                // 75s: unmute — iOS's turn to speak
                try? await Task.sleep(nanoseconds: 70 * 1_000_000_000)
                print("[TURN] iOS speaking\(hasMediaFile ? " (media file)" : " (synthetic audio)")")
                manager.setMicEnabled(true)
                manager.setCameraEnabled(true)
                startMedia()
                // 100s: everyone speaks (keep media on)
                try? await Task.sleep(nanoseconds: 25 * 1_000_000_000)
                print("[TURN] iOS unmuted (all speak)")
                // Already on — stays on for final phase
            }
        }
        // Lobby banner is now persistent — driven by waitingParticipants list
    }

    // MARK: - Adaptive Mode Helpers

    private func modeIcon(_ mode: AdaptiveMode) -> String {
        switch mode {
        case .office: return "wifi"
        case .pedestrian: return "figure.walk"
        case .car: return "car.fill"
        }
    }

    private func modeLabel(_ mode: AdaptiveMode) -> String {
        switch mode {
        case .office: return Strings.t("adaptive.office", lang: lang)
        case .pedestrian: return Strings.t("adaptive.pedestrian", lang: lang)
        case .car: return Strings.t("adaptive.car", lang: lang)
        }
    }

    // MARK: - Layout Engine Integration

    private func updateLayout() {
        let localSid = manager.participants.first?.sid ?? ""
        let screenShareFocus: FocusItem? = manager.lastScreenShareParticipantSid.map {
            FocusItem(participantSid: $0, source: .screenShare)
        }
        let pinnedFocus: FocusItem? = userPinned ? focusedItem : nil
        let (decision, newState) = computeLayout(
            participants: manager.participants,
            activeSpeakers: manager.activeSpeakers,
            pinnedItem: pinnedFocus,
            screenShare: screenShareFocus,
            adaptiveMode: effectiveAdaptiveMode,
            localParticipantSid: localSid,
            previousState: layoutState,
            nowMs: Date().timeIntervalSince1970 * 1000
        )
        layoutState = newState

        switch decision.mode {
        case .grid:
            if !userPinned {
                withAnimation(.easeInOut(duration: 0.2)) { focusedItem = nil }
            }
        case .focus:
            if let main = decision.mainTile, !userPinned {
                let newFocus = FocusItem(participantSid: main.participant.sid, source: main.source)
                if focusedItem != newFocus {
                    withAnimation(.easeInOut(duration: 0.2)) { focusedItem = newFocus }
                }
            }
        }
    }

    // MARK: - Grid Layout

    private var gridLayout: some View {
        let displayItems = buildDisplayItems(manager.participants)
        let count = displayItems.count
        guard count > 0 else { return AnyView(Color.clear) }

        return AnyView(GeometryReader { geo in
            let isLandscape = geo.size.width > geo.size.height
            let columnCount: Int = {
                if count == 1 { return 1 }
                if isLandscape { return min(count, 3) }
                return count <= 2 ? 1 : 2
            }()
            let rowCount = max(1, (count + columnCount - 1) / columnCount)
            let tileHeight = (geo.size.height - 16 - CGFloat(rowCount - 1) * 8) / CGFloat(rowCount)

            VStack(spacing: 8) {
                ForEach(Array(stride(from: 0, to: count, by: columnCount)), id: \.self) { rowStart in
                    HStack(spacing: 8) {
                        ForEach(rowStart..<min(rowStart + columnCount, count), id: \.self) { idx in
                            let item = displayItems[idx]
                            ZStack {
                                ParticipantTile(
                                    participant: item.participant,
                                    trackSidOverride: item.isScreenShare ? item.trackSid : nil,
                                    isActiveSpeaker: manager.activeSpeakers.contains(item.participant.sid),
                                    handRaisePosition: item.isScreenShare ? 0 : manager.handRaisedMap[item.participant.sid] ?? 0,
                                    isDark: isDark
                                )

                                // Screen share badge (top-leading)
                                if item.isScreenShare {
                                    VStack {
                                        HStack {
                                            Image(systemName: "display")
                                                .font(.system(size: 12, weight: .medium))
                                                .foregroundStyle(.white)
                                                .padding(6)
                                                .background(Color.black.opacity(0.6))
                                                .clipShape(RoundedRectangle(cornerRadius: 4))
                                                .padding(6)
                                            Spacer()
                                        }
                                        Spacer()
                                    }
                                }

                                // Expand icon (top-trailing) for screen share tiles
                                if item.isScreenShare {
                                    VStack {
                                        HStack {
                                            Spacer()
                                            Button {
                                                withAnimation(.easeInOut(duration: 0.2)) {
                                                    userPinned = true
                                                    focusedItem = FocusItem(participantSid: item.participant.sid, source: item.source)
                                                }
                                            } label: {
                                                Image(systemName: "arrow.up.left.and.arrow.down.right")
                                                    .font(.system(size: 12, weight: .medium))
                                                    .foregroundStyle(.white)
                                                    .frame(width: 28, height: 28)
                                                    .background(Color.black.opacity(0.6))
                                                    .clipShape(Circle())
                                            }
                                            .padding(6)
                                        }
                                        Spacer()
                                    }
                                }
                            }
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                            .clipShape(RoundedRectangle(cornerRadius: 8))
                            .onTapGesture {
                                withAnimation(.easeInOut(duration: 0.2)) {
                                    userPinned = true
                                    focusedItem = FocusItem(participantSid: item.participant.sid, source: item.source)
                                }
                            }
                            .onLongPressGesture {
                                withAnimation(.easeInOut(duration: 0.2)) {
                                    if userPinned && focusedItem?.participantSid == item.participant.sid {
                                        userPinned = false
                                        updateLayout()
                                    } else {
                                        userPinned = true
                                        focusedItem = FocusItem(participantSid: item.participant.sid, source: item.source)
                                    }
                                }
                            }
                        }
                    }
                    .frame(height: tileHeight)
                }
            }
            .padding(8)
        })
    }

    // MARK: - Focus Layout

    private func focusLayout(focused: DisplayItem, allItems: [DisplayItem], pinnedIndicatorSid: String? = nil, speakerIndicatorSid: String? = nil) -> some View {
        VStack(spacing: 8) {
            // Main focused view
            ZStack {
                ParticipantTile(
                    participant: focused.participant,
                    trackSidOverride: focused.isScreenShare ? focused.trackSid : nil,
                    large: true,
                    isActiveSpeaker: speakerIndicatorSid == focused.participant.sid || manager.activeSpeakers.contains(focused.participant.sid),
                    handRaisePosition: focused.isScreenShare ? 0 : manager.handRaisedMap[focused.participant.sid] ?? 0,
                    isPinned: pinnedIndicatorSid == focused.participant.sid,
                    isDark: isDark
                )

                // Screen share badge (top-leading)
                if focused.isScreenShare {
                    VStack {
                        HStack {
                            Image(systemName: "display")
                                .font(.system(size: 14, weight: .medium))
                                .foregroundStyle(.white)
                                .padding(8)
                                .background(Color.black.opacity(0.6))
                                .clipShape(RoundedRectangle(cornerRadius: 6))
                                .padding(8)
                            Spacer()
                        }
                        Spacer()
                    }
                }

                // Close / back-to-grid button (top-trailing)
                VStack {
                    HStack {
                        Spacer()
                        Button {
                            withAnimation(.easeInOut(duration: 0.2)) {
                                userPinned = false
                                focusedItem = nil
                            }
                        } label: {
                            Image(systemName: "xmark")
                                .font(.system(size: 14, weight: .bold))
                                .foregroundStyle(.white)
                                .frame(width: 32, height: 32)
                                .background(Color.black.opacity(0.6))
                                .clipShape(Circle())
                        }
                        .padding(8)
                    }
                    Spacer()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .accessibilityIdentifier("main-tile:\(focused.participant.sid)")
            .padding(.horizontal, 8)
            .padding(.top, 8)

            // Thumbnail bar
            let others = allItems.filter { $0.id != focused.id }
            if !others.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(others) { item in
                            ZStack(alignment: .topLeading) {
                                ParticipantTile(
                                    participant: item.participant,
                                    trackSidOverride: item.isScreenShare ? item.trackSid : nil,
                                    isActiveSpeaker: speakerIndicatorSid == item.participant.sid || manager.activeSpeakers.contains(item.participant.sid),
                                    handRaisePosition: item.isScreenShare ? 0 : manager.handRaisedMap[item.participant.sid] ?? 0,
                                    isPinned: pinnedIndicatorSid == item.participant.sid,
                                    isDark: isDark
                                )

                                if item.isScreenShare {
                                    Image(systemName: "display")
                                        .font(.system(size: 10, weight: .medium))
                                        .foregroundStyle(.white)
                                        .padding(4)
                                        .background(Color.black.opacity(0.6))
                                        .clipShape(RoundedRectangle(cornerRadius: 3))
                                        .padding(4)
                                }
                            }
                            .frame(width: 120, height: 90)
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                            .onTapGesture {
                                withAnimation(.easeInOut(duration: 0.2)) {
                                    userPinned = true
                                    focusedItem = FocusItem(participantSid: item.participant.sid, source: item.source)
                                }
                            }
                            .onLongPressGesture {
                                withAnimation(.easeInOut(duration: 0.2)) {
                                    if userPinned && focusedItem?.participantSid == item.participant.sid {
                                        userPinned = false
                                        updateLayout()
                                    } else {
                                        userPinned = true
                                        focusedItem = FocusItem(participantSid: item.participant.sid, source: item.source)
                                    }
                                }
                            }
                        }
                    }
                    .padding(.horizontal, 8)
                }
                .frame(height: 90)
                .padding(.bottom, 4)
            }
        }
    }

    // MARK: - Lobby Waiting Banner

    @ViewBuilder
    private var lobbyNotificationBanner: some View {
        if !manager.waitingParticipants.isEmpty {
            let first = manager.waitingParticipants[0]
            let message: String = {
                let base = Strings.t("lobby.joinRequest", lang: lang)
                    .replacingOccurrences(of: "{{name}}", with: first.username)
                if manager.waitingParticipants.count > 1 {
                    return base + " (+\(manager.waitingParticipants.count - 1))"
                }
                return base
            }()
            HStack(spacing: 12) {
                Text(message)
                    .font(.subheadline)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .lineLimit(2)

                Spacer()

                Button {
                    manager.admitParticipant(first.id)
                } label: {
                    Text(Strings.t("lobby.admit", lang: lang))
                        .font(.caption)
                        .fontWeight(.semibold)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(VisioColors.primary500)
                        .clipShape(Capsule())
                }

                Button {
                    showParticipantList = true
                } label: {
                    Text(Strings.t("lobby.view", lang: lang))
                        .font(.caption)
                        .fontWeight(.semibold)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(VisioColors.primaryDark300)
                        .clipShape(Capsule())
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(VisioColors.primaryDark100)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .padding(.horizontal, 12)
            .padding(.top, 8)
            .transition(.move(edge: .top).combined(with: .opacity))
        }
    }

    // MARK: - Pedestrian Single Tile

    private func pedestrianSingleTile(speaker: ParticipantInfo?) -> some View {
        let displayParticipant = speaker ?? manager.participants.first
        return Group {
            if let speaker = displayParticipant {
                ParticipantTile(
                    participant: speaker,
                    large: true,
                    isActiveSpeaker: manager.activeSpeakers.contains(speaker.sid),
                    handRaisePosition: manager.handRaisedMap[speaker.sid] ?? 0,
                    isDark: true
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .clipShape(RoundedRectangle(cornerRadius: 8))
                .accessibilityIdentifier("main-tile:\(speaker.sid)")
                .padding(8)
            }
        }
    }

    // MARK: - Car Audio-Only View

    private func carAudioOnlyView(speaker: ParticipantInfo?) -> some View {
        let activeSpeaker = speaker ?? manager.participants.first
        let speakerName = activeSpeaker?.name ?? activeSpeaker?.identity ?? ""
        return VStack(spacing: 24) {
            Spacer()
            Image(systemName: "speaker.wave.3.fill")
                .font(.system(size: 64))
                .foregroundStyle(.white.opacity(0.6))
            Text(speakerName)
                .font(.system(size: 36, weight: .bold))
                .foregroundStyle(.white)
                .lineLimit(2)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Text(Strings.t("adaptive.audioOnly", lang: lang))
                .font(.subheadline)
                .foregroundStyle(.white.opacity(0.5))
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Connection Banner

    @ViewBuilder
    private var connectionBanner: some View {
        switch manager.connectionState {
        case .connecting:
            bannerView(text: "\(Strings.t("status.connecting", lang: lang))...", color: .orange)
        case .reconnecting(let attempt):
            bannerView(text: "\(Strings.t("status.reconnecting", lang: lang)) (\(attempt))...", color: .orange)
        case .disconnected:
            bannerView(text: Strings.t("status.disconnected", lang: lang), color: VisioColors.greyscale400)
        case .waitingForHost:
            EmptyView()
        case .connected:
            EmptyView()
        }
    }

    private func bannerView(text: String, color: Color) -> some View {
        Text(text)
            .font(.caption)
            .fontWeight(.medium)
            .foregroundStyle(.white)
            .padding(6)
            .frame(maxWidth: .infinity)
            .background(color)
    }

    @ViewBuilder
    private var bandwidthBanner: some View {
        switch manager.bandwidthMode {
        case .reducedVideo:
            bannerView(
                text: Strings.t("bandwidth.reducedVideo", lang: lang),
                color: .orange
            )
        case .audioOnly:
            bannerView(
                text: Strings.t("bandwidth.audioOnly", lang: lang),
                color: VisioColors.error500
            )
        case .full:
            EmptyView()
        }
    }

    // MARK: - Reaction Emojis

    private static let reactionEmojis: [(id: String, emoji: String)] = [
        ("thumbsUp", "\u{1F44D}"),
        ("clap", "\u{1F44F}"),
        ("joy", "\u{1F602}"),
        ("openMouth", "\u{1F62E}"),
        ("tada", "\u{1F389}"),
        ("heart", "\u{2764}\u{FE0F}"),
    ]

    // MARK: - Control Bar

    private var isLargeButtons: Bool {
        effectiveAdaptiveMode == .pedestrian || effectiveAdaptiveMode == .car
    }

    private var buttonSize: CGFloat { isLargeButtons ? 96 : 38 }
    private var buttonIconSize: CGFloat { isLargeButtons ? 36 : 18 }
    private var buttonCornerRadius: CGFloat { isLargeButtons ? 16 : 8 }

    private var controlBar: some View {
        VStack(spacing: 4) {
            // Reaction picker row (above control bar) — office only
            if showReactionPicker && effectiveAdaptiveMode == .office {
                HStack(spacing: 0) {
                    ForEach(Self.reactionEmojis, id: \.id) { item in
                        Button {
                            manager.sendReaction(item.id)
                            showReactionPicker = false
                        } label: {
                            Text(item.emoji)
                                .font(.system(size: 28))
                                .padding(4)
                        }
                    }
                }
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 8)
                .padding(.vertical, 8)
                .background(Color.black.opacity(0.8))
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .padding(.horizontal, 16)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }

            // Overflow menu row (above control bar) — office only
            if showOverflow && effectiveAdaptiveMode == .office {
                HStack(spacing: 0) {
                    Spacer()

                    // Hand raise
                    overflowItem(
                        icon: "hand.raised.fill",
                        label: Strings.t(manager.isHandRaised ? "control.lowerHand" : "control.raiseHand", lang: lang),
                        isActive: manager.isHandRaised,
                        activeColor: VisioColors.handRaise
                    ) {
                        manager.toggleHandRaise()
                        showOverflow = false
                    }

                    Spacer()

                    // Reaction
                    overflowItem(
                        icon: "face.smiling",
                        label: "Reaction",
                        isActive: false,
                        activeColor: .clear
                    ) {
                        showOverflow = false
                        withAnimation(.easeInOut(duration: 0.2)) {
                            showReactionPicker.toggle()
                        }
                    }

                    Spacer()

                    // Settings
                    overflowItem(
                        icon: "gearshape.fill",
                        label: Strings.t("settings.incall", lang: lang),
                        isActive: false,
                        activeColor: .clear
                    ) {
                        showOverflow = false
                        inCallSettingsTab = 0
                        showInCallSettings = true
                    }

                    Spacer()
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(Color.black.opacity(0.8))
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .padding(.horizontal, 16)
                .transition(.move(edge: .bottom).combined(with: .opacity))

                // Adaptive mode override
                VStack(alignment: .leading, spacing: 6) {
                    Text(Strings.t("adaptive.override", lang: lang))
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(.white)
                    HStack(spacing: 8) {
                        let modeOptions: [(AdaptiveMode?, String)] = [
                            (nil, Strings.t("adaptive.auto", lang: lang)),
                            (.office, Strings.t("adaptive.office", lang: lang)),
                            (.pedestrian, Strings.t("adaptive.pedestrian", lang: lang)),
                            (.car, Strings.t("adaptive.car", lang: lang)),
                        ]
                        ForEach(Array(modeOptions.enumerated()), id: \.offset) { _, option in
                            let (mode, label) = option
                            let isSelected = mode == adaptiveModeOverride
                            Button {
                                adaptiveModeOverride = mode
                                manager.client.setAdaptiveModeOverride(mode: mode)
                            } label: {
                                Text(label)
                                    .font(.system(size: 11, weight: isSelected ? .bold : .regular))
                                    .foregroundStyle(isSelected ? .black : .white)
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 6)
                                    .background(isSelected ? VisioColors.primary500 : VisioColors.primaryDark100)
                                    .clipShape(Capsule())
                            }
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.black.opacity(0.8))
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .padding(.horizontal, 16)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }

            // Main control bar
            HStack(spacing: isLargeButtons ? 16 : 8) {
                // Mic toggle + audio route chevron (grouped) — all modes
                HStack(spacing: 1) {
                    Button {
                        manager.toggleMic()
                    } label: {
                        Image(systemName: manager.isMicEnabled ? "mic.fill" : "mic.slash.fill")
                            .font(.system(size: buttonIconSize, weight: .medium))
                            .foregroundStyle(.white)
                            .frame(width: buttonSize, height: buttonSize)
                            .background(manager.isMicEnabled ? VisioColors.primaryDark100 : VisioColors.error200)
                            .clipShape(UnevenRoundedRectangle(
                                topLeadingRadius: buttonCornerRadius,
                                bottomLeadingRadius: buttonCornerRadius,
                                bottomTrailingRadius: 2,
                                topTrailingRadius: 2
                            ))
                    }
                    .accessibilityLabel(Strings.t(manager.isMicEnabled ? "control.mute" : "control.unmute", lang: lang))

                    Button {
                        showAudioDevices = true
                    } label: {
                        Image(systemName: "chevron.up")
                            .font(.system(size: isLargeButtons ? 16 : 10, weight: .bold))
                            .foregroundStyle(.white)
                            .frame(width: isLargeButtons ? 40 : 22, height: buttonSize)
                            .background(VisioColors.primaryDark100)
                            .clipShape(UnevenRoundedRectangle(
                                topLeadingRadius: 2,
                                bottomLeadingRadius: 2,
                                bottomTrailingRadius: buttonCornerRadius,
                                topTrailingRadius: buttonCornerRadius
                            ))
                    }
                    .accessibilityLabel(Strings.t("control.audioDevices", lang: lang))
                }

                // Camera toggle — office and pedestrian only
                if effectiveAdaptiveMode != .car {
                    Button {
                        manager.toggleCamera()
                    } label: {
                        Image(systemName: manager.isCameraEnabled ? "video.fill" : "video.slash.fill")
                            .font(.system(size: buttonIconSize, weight: .medium))
                            .foregroundStyle(.white)
                            .frame(width: buttonSize, height: buttonSize)
                            .background(manager.isCameraEnabled ? VisioColors.primaryDark100 : VisioColors.error200)
                            .clipShape(RoundedRectangle(cornerRadius: buttonCornerRadius))
                    }
                    .accessibilityLabel(Strings.t(manager.isCameraEnabled ? "control.camOff" : "control.camOn", lang: lang))
                }

                // Participants with count badge — office only
                if effectiveAdaptiveMode == .office {
                    Button {
                        showParticipantList = true
                    } label: {
                        ZStack(alignment: .topTrailing) {
                            Image(systemName: "person.2.fill")
                                .font(.system(size: 16, weight: .medium))
                                .foregroundStyle(.white)
                                .frame(width: 38, height: 38)
                                .background(VisioColors.primaryDark100)
                                .clipShape(RoundedRectangle(cornerRadius: 8))

                            Text("\(manager.participants.count)")
                                .font(.system(size: 10, weight: .bold))
                                .foregroundStyle(.white)
                                .padding(.horizontal, 4)
                                .padding(.vertical, 1)
                                .background(VisioColors.primary500)
                                .clipShape(Capsule())
                                .offset(x: 4, y: -4)
                        }
                    }
                    .accessibilityLabel(Strings.t("participants.title", lang: lang))
                }

                // Chat with unread badge — office only
                if effectiveAdaptiveMode == .office {
                    Button {
                        showChat = true
                    } label: {
                        ZStack(alignment: .topTrailing) {
                            Image(systemName: "message.fill")
                                .font(.system(size: 18, weight: .medium))
                                .foregroundStyle(.white)
                                .frame(width: 38, height: 38)
                                .background(VisioColors.primaryDark100)
                                .clipShape(RoundedRectangle(cornerRadius: 8))

                            if manager.unreadCount > 0 {
                                Text(manager.unreadCount <= 9 ? "\(manager.unreadCount)" : "9+")
                                    .font(.system(size: 10, weight: .bold))
                                    .foregroundStyle(.white)
                                    .padding(.horizontal, 4)
                                    .padding(.vertical, 1)
                                    .background(VisioColors.error500)
                                    .clipShape(Capsule())
                                    .offset(x: 4, y: -4)
                            }
                        }
                    }
                    .accessibilityLabel(Strings.t("chat", lang: lang))
                }

                // More (overflow) button — office only
                if effectiveAdaptiveMode == .office {
                    Button {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            showOverflow.toggle()
                            if showOverflow {
                                showReactionPicker = false
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                            .font(.system(size: 18, weight: .medium))
                            .foregroundStyle(.white)
                            .frame(width: 38, height: 38)
                            .background(showOverflow ? VisioColors.primary500 : VisioColors.primaryDark100)
                            .clipShape(RoundedRectangle(cornerRadius: 8))
                    }
                    .accessibilityLabel("More")
                }

                // Hangup
                Button {
                    manager.disconnect()
                    CallKitManager.shared.reportCallEnded()
                    dismiss()
                } label: {
                    Image(systemName: "phone.down.fill")
                        .font(.system(size: buttonIconSize, weight: .medium))
                        .foregroundStyle(.white)
                        .frame(width: buttonSize, height: buttonSize)
                        .background(VisioColors.error500)
                        .clipShape(RoundedRectangle(cornerRadius: buttonCornerRadius))
                }
                .accessibilityIdentifier("hangup")
                .accessibilityLabel(Strings.t("control.leave", lang: lang))
            }
            .padding(isLargeButtons ? 12 : 8)
            .background(effectiveAdaptiveMode == .office
                ? VisioColors.surface(dark: isDark)
                : Color.black.opacity(0.6)
            )
            .clipShape(RoundedRectangle(cornerRadius: isLargeButtons ? 24 : 16))
            .padding(.horizontal, 8)
        }
        .padding(.bottom, 8)
    }

    // MARK: - Overflow Menu Item

    private func overflowItem(
        icon: String,
        label: String,
        isActive: Bool,
        activeColor: Color,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            VStack(spacing: 4) {
                Image(systemName: icon)
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(isActive ? .black : .white)
                    .frame(width: 38, height: 38)
                    .background(isActive ? activeColor : VisioColors.primaryDark100)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                Text(label)
                    .font(.system(size: 10))
                    .foregroundStyle(.white)
                    .lineLimit(1)
            }
            .accessibilityLabel(Strings.t("control.leave", lang: manager.currentLang))
        }
    }
}

// MARK: - Participant Tile

struct ParticipantTile: View {
    @EnvironmentObject private var manager: VisioManager
    let participant: ParticipantInfo
    var trackSidOverride: String? = nil
    var large: Bool = false
    var isActiveSpeaker: Bool = false
    var handRaisePosition: Int = 0
    var isPinned: Bool = false
    var isDark: Bool = true

    private var effectiveTrackSid: String? {
        trackSidOverride ?? participant.videoTrackSid
    }

    /// Video is paused by bandwidth degradation (not by user).
    private var videoPausedByBandwidth: Bool {
        guard participant.hasVideo,
              effectiveTrackSid != nil,
              trackSidOverride == nil else { return false }
        switch manager.bandwidthMode {
        case .audioOnly: return true
        case .reducedVideo: return !isActiveSpeaker
        case .full: return false
        }
    }

    var body: some View {
        ZStack(alignment: .bottom) {
            if videoPausedByBandwidth {
                videoPausedView
            } else if participant.hasVideo, let trackSid = effectiveTrackSid {
                VideoLayerView(
                    trackSid: trackSid,
                    isScreenShare: trackSid == participant.screenShareTrackSid
                )
            } else {
                avatarView
            }

            // Metadata bar at bottom
            metadataBar

            // Pin indicator (top-right)
            if isPinned {
                VStack {
                    HStack {
                        Spacer()
                        Image(systemName: "pin.fill")
                            .font(.system(size: 12))
                            .foregroundStyle(.white)
                            .padding(6)
                            .background(Color.black.opacity(0.6))
                            .clipShape(Circle())
                            .padding(4)
                    }
                    Spacer()
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isActiveSpeaker ? VisioColors.primary500 : .clear, lineWidth: 2)
        )
        .shadow(color: isActiveSpeaker ? VisioColors.primary500.opacity(0.5) : .clear, radius: 6)
    }

    private var videoPausedView: some View {
        ZStack {
            Color(red: 0.1, green: 0.1, blue: 0.18)

            VStack(spacing: 8) {
                Image(systemName: "video.slash")
                    .font(.title)
                    .foregroundStyle(Color.orange)
                Text(participant.name ?? participant.identity)
                    .font(.caption)
                    .foregroundStyle(Color(white: 0.53))
                    .lineLimit(1)
                Text(Strings.t("bandwidth.videoPaused", lang: manager.currentLang))
                    .font(.caption2)
                    .foregroundStyle(Color.orange)
                    .lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var avatarView: some View {
        ZStack {
            Color(red: 0.1, green: 0.1, blue: 0.18)

            VStack(spacing: 8) {
                Image(systemName: "video.slash")
                    .font(.title)
                    .foregroundStyle(Color.gray)
                Text(participant.name ?? participant.identity)
                    .font(.caption)
                    .foregroundStyle(Color(white: 0.53))
                    .lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var metadataBar: some View {
        HStack(spacing: 6) {
            // Mic muted indicator
            if participant.isMuted {
                Image(systemName: "mic.slash.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(VisioColors.error500)
            }

            // Hand raise pill
            if handRaisePosition > 0 {
                HStack(spacing: 2) {
                    Image(systemName: "hand.raised.fill")
                        .font(.system(size: 11))
                    Text("\(handRaisePosition)")
                        .font(.caption2)
                        .bold()
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(VisioColors.handRaise)
                .clipShape(Capsule())
                .foregroundStyle(.black)
            }

            Spacer()

            // Participant name
            Text(participant.name ?? participant.identity)
                .font(.caption)
                .lineLimit(1)
                .foregroundStyle(.white)

            // Connection quality
            connectionQualityIndicator
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Color.black.opacity(0.6))
    }

    @ViewBuilder
    private var connectionQualityIndicator: some View {
        switch participant.connectionQuality {
        case .excellent:
            Image(systemName: "wifi")
                .font(.system(size: 10))
                .foregroundStyle(.green)
        case .good:
            Image(systemName: "wifi")
                .font(.system(size: 10))
                .foregroundStyle(.yellow)
        case .poor:
            Image(systemName: "wifi.exclamationmark")
                .font(.system(size: 10))
                .foregroundStyle(.orange)
        case .lost:
            Image(systemName: "wifi.slash")
                .font(.system(size: 10))
                .foregroundStyle(VisioColors.error500)
        }
    }

    // MARK: - Helpers

    private var initials: String {
        let name = participant.name ?? participant.identity
        let parts = name.split(separator: " ")
        if parts.count >= 2 {
            return String(parts[0].prefix(1) + parts[1].prefix(1)).uppercased()
        }
        return String(name.prefix(2)).uppercased()
    }

    private var nameHue: Double {
        let name = participant.name ?? participant.identity
        let hash = name.unicodeScalars.reduce(0) { $0 + Int($1.value) }
        return Double(hash % 360) / 360.0
    }
}

// MARK: - Audio Device Sheet

struct AudioDeviceSheet: View {
    @EnvironmentObject private var manager: VisioManager
    @State private var snapshot = AudioDeviceSnapshot(
        availableInputs: [], currentOutputs: [],
        currentInput: nil, isSpeakerOverride: false
    )
    @Environment(\.dismiss) private var dismiss

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        NavigationStack {
            List {
                Section(Strings.t("audio.output", lang: lang)) {
                    Button {
                        do { try AVAudioSession.sharedInstance().overrideOutputAudioPort(.speaker) }
                        catch { NSLog("Failed to override audio to speaker: %@", error.localizedDescription) }
                        snapshot = AudioDeviceSnapshot.current()
                    } label: {
                        HStack {
                            Image(systemName: "speaker.wave.2.fill")
                                .foregroundStyle(VisioColors.primary500)
                            Text(Strings.t("audio.speaker", lang: lang))
                                .foregroundStyle(VisioColors.onSurface(dark: isDark))
                            Spacer()
                            if snapshot.isSpeakerOverride {
                                Image(systemName: "checkmark")
                                    .foregroundStyle(VisioColors.primary500)
                            }
                        }
                    }

                    Button {
                        do { try AVAudioSession.sharedInstance().overrideOutputAudioPort(.none) }
                        catch { NSLog("Failed to override audio to earpiece: %@", error.localizedDescription) }
                        snapshot = AudioDeviceSnapshot.current()
                    } label: {
                        HStack {
                            Image(systemName: "iphone")
                                .foregroundStyle(VisioColors.primary500)
                            Text(Strings.t("audio.earpiece", lang: lang))
                                .foregroundStyle(VisioColors.onSurface(dark: isDark))
                            Spacer()
                            if !snapshot.isSpeakerOverride && snapshot.currentOutputs.contains(where: { $0.portType == .builtInReceiver || $0.portType == .builtInSpeaker }) && !snapshot.currentOutputs.contains(where: { isExternalAudioOutput($0) }) {
                                Image(systemName: "checkmark")
                                    .foregroundStyle(VisioColors.primary500)
                            }
                        }
                    }

                    ForEach(snapshot.currentOutputs.filter { isExternalAudioOutput($0) }, id: \.uid) { port in
                        HStack {
                            Image(systemName: iconForAudioOutputPort(port))
                                .foregroundStyle(VisioColors.primary500)
                            Text(port.portName)
                                .foregroundStyle(VisioColors.onSurface(dark: isDark))
                            Spacer()
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                Section(Strings.t("audio.input", lang: lang)) {
                    ForEach(snapshot.availableInputs, id: \.uid) { port in
                        Button {
                            selectAudioInput(port)
                            snapshot = AudioDeviceSnapshot.current()
                        } label: {
                            HStack {
                                Image(systemName: iconForAudioInputPort(port))
                                    .foregroundStyle(VisioColors.primary500)
                                Text(port.portName)
                                    .foregroundStyle(VisioColors.onSurface(dark: isDark))
                                Spacer()
                                if port.uid == snapshot.currentInput?.uid {
                                    Image(systemName: "checkmark")
                                        .foregroundStyle(VisioColors.primary500)
                                }
                            }
                        }
                    }
                }
            }
            .scrollContentBackground(.hidden)
            .background(VisioColors.background(dark: isDark))
            .navigationTitle(Strings.t("audio.source", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
            .toolbarBackground(VisioColors.surface(dark: isDark), for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .appToolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(Strings.t("audio.done", lang: lang)) { dismiss() }
                        .foregroundStyle(VisioColors.primary500)
                }
            }
        }
        .onAppear { snapshot = AudioDeviceSnapshot.current() }
    }
}

// MARK: - Reaction Overlay

struct ReactionOverlay: View {
    let reactions: [ReactionData]

    private static let reactionEmojis: [(id: String, emoji: String)] = [
        ("thumbsUp", "\u{1F44D}"),
        ("clap", "\u{1F44F}"),
        ("joy", "\u{1F602}"),
        ("openMouth", "\u{1F62E}"),
        ("tada", "\u{1F389}"),
        ("heart", "\u{2764}\u{FE0F}"),
    ]

    var body: some View {
        let now = Date()
        let active = reactions.filter { now.timeIntervalSince($0.timestamp) < 3.0 }

        GeometryReader { geo in
            ZStack(alignment: .bottomLeading) {
                Color.clear

                ForEach(active) { reaction in
                    FloatingReaction(
                        reaction: reaction,
                        emojiDisplay: Self.reactionEmojis.first(where: { $0.id == reaction.emoji })?.emoji ?? reaction.emoji,
                        screenWidth: geo.size.width
                    )
                }
            }
        }
        .allowsHitTesting(false)
    }
}

struct FloatingReaction: View {
    let reaction: ReactionData
    let emojiDisplay: String
    let screenWidth: CGFloat

    @State private var progress: CGFloat = 0

    var body: some View {
        let xOffset = CGFloat(abs((reaction.id * 37 + 13) % Int64(max(screenWidth * 0.2, 1))))
        let yOffset = -300.0 * progress
        let alpha = progress > 0.7 ? 1.0 - ((progress - 0.7) / 0.3) : 1.0

        VStack(spacing: 2) {
            Text(emojiDisplay)
                .font(.system(size: 32))
            Text(reaction.participantName)
                .font(.system(size: 10))
                .foregroundStyle(.white)
                .lineLimit(1)
                .padding(.horizontal, 4)
                .padding(.vertical, 1)
                .background(Color.black.opacity(0.6))
                .clipShape(RoundedRectangle(cornerRadius: 4))
        }
        .offset(x: xOffset, y: yOffset)
        .opacity(alpha)
        .onAppear {
            withAnimation(.linear(duration: 3.0)) {
                progress = 1.0
            }
        }
    }
}

#Preview {
    NavigationStack {
        CallView(roomURL: "meet.example.com/test", displayName: "Alice")
            .environmentObject(VisioManager())
    }
}
