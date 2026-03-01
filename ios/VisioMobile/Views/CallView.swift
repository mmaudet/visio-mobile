import SwiftUI
import AVFoundation
import visioFFI

struct CallView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase

    let roomURL: String
    let displayName: String

    @State private var showChat: Bool = false
    @State private var showAudioDevices: Bool = false
    @State private var focusedParticipant: String? = nil

    var body: some View {
        ZStack {
            VisioColors.primaryDark50.ignoresSafeArea()

            VStack(spacing: 0) {
                // Connection state banner
                connectionBanner

                // Error banner
                if let error = manager.errorMessage {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.white)
                        .padding(8)
                        .frame(maxWidth: .infinity)
                        .background(VisioColors.error500)
                }

                // Main content area: video grid or waiting
                if manager.participants.isEmpty {
                    Spacer()
                    VStack(spacing: 12) {
                        ProgressView()
                            .tint(.white)
                        Text("Waiting for participants...")
                            .foregroundStyle(VisioColors.greyscale400)
                    }
                    Spacer()
                } else if let focused = focusedParticipant,
                          let focusedP = manager.participants.first(where: { $0.sid == focused }) {
                    // Focus layout
                    focusLayout(focused: focusedP)
                } else {
                    // Grid layout
                    gridLayout
                }

                // Control bar
                controlBar
            }
        }
        .navigationTitle("Call")
        .navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(true)
        .toolbarColorScheme(.dark, for: .navigationBar)
        .toolbarBackground(VisioColors.primaryDark75, for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        .fullScreenCover(isPresented: $showChat) {
            NavigationStack {
                ChatView()
                    .environmentObject(manager)
            }
            .onAppear { manager.setChatOpen(true) }
            .onDisappear { manager.setChatOpen(false) }
        }
        .sheet(isPresented: $showAudioDevices) {
            AudioDeviceSheet()
                .presentationDetents([.medium])
        }
        .onAppear {
            let name = displayName.isEmpty ? nil : displayName
            manager.connect(url: roomURL, username: name)
            manager.startAudioPlayout()
            CallKitManager.shared.reportCallStarted(roomName: roomURL)
        }
        .onDisappear {
            manager.stopAudioPlayout()
        }
        .onChange(of: scenePhase) { phase in
            if phase == .background {
                PiPManager.shared.startIfNeeded()
            } else if phase == .active {
                PiPManager.shared.stop()
            }
        }
        .preferredColorScheme(.dark)
    }

    // MARK: - Grid Layout

    private var gridLayout: some View {
        let count = manager.participants.count
        let columnCount = count <= 2 ? 1 : 2
        let columns = Array(repeating: GridItem(.flexible(), spacing: 8), count: columnCount)

        return ScrollView {
            LazyVGrid(columns: columns, spacing: 8) {
                ForEach(manager.participants, id: \.sid) { participant in
                    ParticipantTile(
                        participant: participant,
                        isActiveSpeaker: manager.activeSpeakers.contains(participant.sid),
                        handRaisePosition: manager.handRaisedMap[participant.sid] ?? 0
                    )
                    .aspectRatio(16.0 / 9.0, contentMode: .fit)
                    .onTapGesture {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            focusedParticipant = participant.sid
                        }
                    }
                }
            }
            .padding(8)
        }
    }

    // MARK: - Focus Layout

    private func focusLayout(focused: ParticipantInfo) -> some View {
        VStack(spacing: 8) {
            // Large focused participant
            ParticipantTile(
                participant: focused,
                large: true,
                isActiveSpeaker: manager.activeSpeakers.contains(focused.sid),
                handRaisePosition: manager.handRaisedMap[focused.sid] ?? 0
            )
            .onTapGesture {
                withAnimation(.easeInOut(duration: 0.2)) {
                    focusedParticipant = nil
                }
            }

            // Horizontal strip of other participants
            let others = manager.participants.filter { $0.sid != focused.sid }
            if !others.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(others, id: \.sid) { p in
                            ParticipantTile(
                                participant: p,
                                isActiveSpeaker: manager.activeSpeakers.contains(p.sid),
                                handRaisePosition: manager.handRaisedMap[p.sid] ?? 0
                            )
                            .frame(width: 160, height: 120)
                            .onTapGesture {
                                withAnimation(.easeInOut(duration: 0.2)) {
                                    focusedParticipant = p.sid
                                }
                            }
                        }
                    }
                    .padding(.horizontal, 8)
                }
                .frame(height: 120)
            }
        }
        .padding(8)
    }

    // MARK: - Connection Banner

    @ViewBuilder
    private var connectionBanner: some View {
        switch manager.connectionState {
        case .connecting:
            bannerView(text: "Connecting...", color: .orange)
        case .reconnecting(let attempt):
            bannerView(text: "Reconnecting (attempt \(attempt))...", color: .orange)
        case .disconnected:
            bannerView(text: "Disconnected", color: VisioColors.greyscale400)
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

    // MARK: - Control Bar

    private var controlBar: some View {
        HStack(spacing: 8) {
            // Mic toggle + audio route chevron (grouped)
            HStack(spacing: 1) {
                // Mic toggle
                Button {
                    manager.toggleMic()
                } label: {
                    Image(systemName: manager.isMicEnabled ? "mic.fill" : "mic.slash.fill")
                        .font(.system(size: 18, weight: .medium))
                        .foregroundStyle(.white)
                        .frame(width: 44, height: 44)
                        .background(manager.isMicEnabled ? VisioColors.primaryDark100 : VisioColors.error200)
                        .clipShape(UnevenRoundedRectangle(topLeadingRadius: 12, bottomLeadingRadius: 12, bottomTrailingRadius: 4, topTrailingRadius: 4))
                }

                // Audio route chevron
                Button {
                    showAudioDevices = true
                } label: {
                    Image(systemName: "chevron.up")
                        .font(.system(size: 12, weight: .bold))
                        .foregroundStyle(.white)
                        .frame(width: 28, height: 44)
                        .background(VisioColors.primaryDark100)
                        .clipShape(UnevenRoundedRectangle(topLeadingRadius: 4, bottomLeadingRadius: 4, bottomTrailingRadius: 12, topTrailingRadius: 12))
                }
            }

            // Camera toggle
            Button {
                manager.toggleCamera()
            } label: {
                Image(systemName: manager.isCameraEnabled ? "video.fill" : "video.slash.fill")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(.white)
                    .frame(width: 44, height: 44)
                    .background(manager.isCameraEnabled ? VisioColors.primaryDark100 : VisioColors.error200)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }

            // Camera switch (front/back)
            Button {
                // Camera switch is a no-op placeholder for now
                // Would need CameraCapture to support position switching
            } label: {
                Image(systemName: "arrow.triangle.2.circlepath.camera")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(.white)
                    .frame(width: 44, height: 44)
                    .background(VisioColors.primaryDark100)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }

            // Hand raise
            Button {
                manager.toggleHandRaise()
            } label: {
                Image(systemName: "hand.raised.fill")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(manager.isHandRaised ? .black : .white)
                    .frame(width: 44, height: 44)
                    .background(manager.isHandRaised ? VisioColors.handRaise : VisioColors.primaryDark100)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }

            // Chat with unread badge
            Button {
                showChat = true
            } label: {
                ZStack(alignment: .topTrailing) {
                    Image(systemName: "message.fill")
                        .font(.system(size: 18, weight: .medium))
                        .foregroundStyle(.white)
                        .frame(width: 44, height: 44)
                        .background(VisioColors.primaryDark100)
                        .clipShape(RoundedRectangle(cornerRadius: 12))

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

            // Hangup
            Button {
                manager.disconnect()
                CallKitManager.shared.reportCallEnded()
                dismiss()
            } label: {
                Image(systemName: "phone.down.fill")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(.white)
                    .frame(width: 44, height: 44)
                    .background(VisioColors.error500)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }
        }
        .padding(12)
        .background(VisioColors.primaryDark75)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .padding(.horizontal, 12)
        .padding(.bottom, 8)
    }
}

// MARK: - Participant Tile

struct ParticipantTile: View {
    let participant: ParticipantInfo
    var large: Bool = false
    var isActiveSpeaker: Bool = false
    var handRaisePosition: Int = 0

    var body: some View {
        ZStack(alignment: .bottom) {
            // Video or avatar fallback
            if let trackSid = participant.videoTrackSid {
                VideoLayerView(trackSid: trackSid)
            } else {
                avatarView
            }

            // Metadata bar at bottom
            metadataBar
        }
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isActiveSpeaker ? VisioColors.primary500 : .clear, lineWidth: 2)
        )
        .shadow(color: isActiveSpeaker ? VisioColors.primary500.opacity(0.5) : .clear, radius: 6)
    }

    private var avatarView: some View {
        ZStack {
            VisioColors.primaryDark50

            Circle()
                .fill(Color(hue: nameHue, saturation: 0.5, brightness: 0.35))
                .frame(width: large ? 80 : 64, height: large ? 80 : 64)
                .overlay(
                    Text(initials)
                        .font(large ? .title : .title2)
                        .bold()
                        .foregroundStyle(.white)
                )
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
    @State private var availableInputs: [AVAudioSessionPortDescription] = []
    @State private var currentInput: AVAudioSessionPortDescription?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                // Built-in speaker option (always available)
                Button {
                    // Setting preferred input to nil selects built-in speaker
                    try? AVAudioSession.sharedInstance().setPreferredInput(nil)
                    currentInput = nil
                    dismiss()
                } label: {
                    HStack {
                        Image(systemName: "speaker.wave.2.fill")
                            .foregroundStyle(VisioColors.primary500)
                        Text("iPhone Speaker")
                            .foregroundStyle(.white)
                        Spacer()
                        if currentInput == nil {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Available inputs (Bluetooth, wired headsets, etc.)
                ForEach(availableInputs, id: \.uid) { port in
                    Button {
                        selectInput(port)
                    } label: {
                        HStack {
                            Image(systemName: iconForPort(port))
                                .foregroundStyle(VisioColors.primary500)
                            Text(port.portName)
                                .foregroundStyle(.white)
                            Spacer()
                            if port.uid == currentInput?.uid {
                                Image(systemName: "checkmark")
                                    .foregroundStyle(VisioColors.primary500)
                            }
                        }
                    }
                }
            }
            .scrollContentBackground(.hidden)
            .background(VisioColors.primaryDark50)
            .navigationTitle("Audio Source")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(.dark, for: .navigationBar)
            .toolbarBackground(VisioColors.primaryDark75, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .foregroundStyle(VisioColors.primary500)
                }
            }
        }
        .onAppear { loadDevices() }
        .preferredColorScheme(.dark)
    }

    private func loadDevices() {
        let session = AVAudioSession.sharedInstance()
        availableInputs = session.availableInputs ?? []
        currentInput = session.currentRoute.inputs.first
    }

    private func selectInput(_ port: AVAudioSessionPortDescription) {
        try? AVAudioSession.sharedInstance().setPreferredInput(port)
        currentInput = port
        dismiss()
    }

    private func iconForPort(_ port: AVAudioSessionPortDescription) -> String {
        switch port.portType {
        case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
            return "wave.3.right"
        case .headphones:
            return "headphones"
        case .builtInMic:
            return "iphone"
        default:
            return "speaker.wave.2"
        }
    }
}

#Preview {
    NavigationStack {
        CallView(roomURL: "meet.example.com/test", displayName: "Alice")
            .environmentObject(VisioManager())
    }
}
