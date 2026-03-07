import SwiftUI
import AVFoundation

struct InCallSettingsSheet: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State var selectedTab: Int

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        NavigationStack {
            HStack(spacing: 0) {
                // Sidebar icons
                VStack(spacing: 4) {
                    tabButton(icon: "mic.fill", tab: 0, label: Strings.t("settings.incall.micro", lang: lang))
                    tabButton(icon: "video.fill", tab: 1, label: Strings.t("settings.incall.camera", lang: lang))
                    tabButton(icon: "bell.fill", tab: 2, label: Strings.t("settings.incall.notifications", lang: lang))
                    Spacer()
                }
                .padding(.vertical, 12)
                .padding(.horizontal, 8)
                .background(VisioColors.surface(dark: isDark))

                Divider()

                // Content area
                Group {
                    switch selectedTab {
                    case 0: microTab
                    case 1: cameraTab
                    case 2: notificationsTab
                    default: microTab
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .background(VisioColors.background(dark: isDark))
            .navigationTitle(Strings.t("settings.incall", lang: lang))
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
    }

    // MARK: - Tab Button

    private func tabButton(icon: String, tab: Int, label: String) -> some View {
        Button {
            selectedTab = tab
        } label: {
            Image(systemName: icon)
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(selectedTab == tab ? VisioColors.primary500 : VisioColors.secondaryText(dark: isDark))
                .frame(width: 44, height: 44)
                .background(selectedTab == tab ? VisioColors.primary500.opacity(0.15) : .clear)
                .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .accessibilityLabel(label)
    }

    // MARK: - Micro Tab

    private var microTab: some View {
        MicroTabContent()
            .environmentObject(manager)
    }

    // MARK: - Camera Tab

    private var cameraTab: some View {
        List {
            Section(Strings.t("settings.incall.cameraSelect", lang: lang)) {
                Button {
                    manager.switchCamera(toFront: true)
                } label: {
                    HStack {
                        Image(systemName: "camera.front.fill")
                            .foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("settings.incall.cameraFront", lang: lang))
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        if manager.isFrontCamera {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                Button {
                    manager.switchCamera(toFront: false)
                } label: {
                    HStack {
                        Image(systemName: "camera.rear.fill")
                            .foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("settings.incall.cameraBack", lang: lang))
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        if !manager.isFrontCamera {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }
            }

            Section(Strings.t("settings.incall.background", lang: lang)) {
                // Off option
                Button {
                    setBackgroundMode("off")
                } label: {
                    HStack {
                        Image(systemName: "circle.slash")
                            .foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("settings.incall.bgOff", lang: lang))
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        if manager.backgroundMode == "off" {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Blur option
                Button {
                    setBackgroundMode("blur")
                } label: {
                    HStack {
                        Image(systemName: "aqi.medium")
                            .foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("settings.incall.bgBlur", lang: lang))
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        if manager.backgroundMode == "blur" {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Image grid
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 4), spacing: 8) {
                    ForEach(1...8, id: \.self) { id in
                        if let path = Bundle.main.path(forResource: "\(id)", ofType: "jpg", inDirectory: "backgrounds/thumbnails"),
                           let img = UIImage(contentsOfFile: path) {
                            Image(uiImage: img)
                                .resizable()
                                .aspectRatio(16.0/9.0, contentMode: .fill)
                                .frame(height: 50)
                                .clipShape(RoundedRectangle(cornerRadius: 6))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 6)
                                        .stroke(manager.backgroundMode == "image:\(id)" ? VisioColors.primary500 : Color.clear, lineWidth: 2)
                                )
                                .onTapGesture { setBackgroundMode("image:\(id)") }
                        }
                    }
                }
                .padding(.vertical, 4)
            }
        }
        .scrollContentBackground(.hidden)
        .background(VisioColors.background(dark: isDark))
    }

    private func setBackgroundMode(_ mode: String) {
        manager.backgroundMode = mode
        DispatchQueue.global(qos: .userInitiated).async {
            if mode.hasPrefix("image:") {
                let id = UInt8(mode.dropFirst(6)) ?? 0
                if let path = Bundle.main.path(forResource: "\(id)", ofType: "jpg", inDirectory: "backgrounds") {
                    try? manager.client.loadBackgroundImage(id: id, jpegPath: path)
                }
            }
            manager.client.setBackgroundMode(mode: mode)
        }
    }

    // MARK: - Notifications Tab

    private var notificationsTab: some View {
        NotificationsTabContent()
            .environmentObject(manager)
    }
}

// MARK: - Micro Tab Content

private struct MicroTabContent: View {
    @EnvironmentObject private var manager: VisioManager
    @State private var availableInputs: [AVAudioSessionPortDescription] = []
    @State private var currentOutputs: [AVAudioSessionPortDescription] = []
    @State private var currentInput: AVAudioSessionPortDescription?
    @State private var isSpeakerOverride: Bool = false

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        List {
            // Output section
            Section(Strings.t("settings.incall.audioOutput", lang: lang)) {
                Button {
                    do { try AVAudioSession.sharedInstance().overrideOutputAudioPort(.speaker) }
                    catch { NSLog("Failed to override audio to speaker: %@", error.localizedDescription) }
                    loadDevices()
                } label: {
                    HStack {
                        Image(systemName: "speaker.wave.2.fill")
                            .foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("audio.speaker", lang: lang))
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        if isSpeakerOverride {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                Button {
                    do { try AVAudioSession.sharedInstance().overrideOutputAudioPort(.none) }
                    catch { NSLog("Failed to override audio to earpiece: %@", error.localizedDescription) }
                    loadDevices()
                } label: {
                    HStack {
                        Image(systemName: "iphone")
                            .foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("audio.earpiece", lang: lang))
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        if !isSpeakerOverride && currentOutputs.contains(where: { $0.portType == .builtInReceiver || $0.portType == .builtInSpeaker }) && !currentOutputs.contains(where: { isExternalOutput($0) }) {
                            Image(systemName: "checkmark")
                                .foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                ForEach(currentOutputs.filter { isExternalOutput($0) }, id: \.uid) { port in
                    HStack {
                        Image(systemName: iconForOutputPort(port))
                            .foregroundStyle(VisioColors.primary500)
                        Text(port.portName)
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Spacer()
                        Image(systemName: "checkmark")
                            .foregroundStyle(VisioColors.primary500)
                    }
                }
            }

            // Input section
            Section(Strings.t("settings.incall.audioInput", lang: lang)) {
                ForEach(availableInputs, id: \.uid) { port in
                    Button {
                        selectInput(port)
                    } label: {
                        HStack {
                            Image(systemName: iconForInputPort(port))
                                .foregroundStyle(VisioColors.primary500)
                            Text(port.portName)
                                .foregroundStyle(VisioColors.onSurface(dark: isDark))
                            Spacer()
                            if port.uid == currentInput?.uid {
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
        .onAppear { loadDevices() }
    }

    private func loadDevices() {
        let session = AVAudioSession.sharedInstance()
        availableInputs = session.availableInputs ?? []
        currentInput = session.currentRoute.inputs.first
        currentOutputs = session.currentRoute.outputs
        isSpeakerOverride = currentOutputs.contains { $0.portType == .builtInSpeaker }
    }

    private func selectInput(_ port: AVAudioSessionPortDescription) {
        do { try AVAudioSession.sharedInstance().setPreferredInput(port) }
        catch { NSLog("Failed to set preferred input: %@", error.localizedDescription) }
        currentInput = port
    }

    private func isExternalOutput(_ port: AVAudioSessionPortDescription) -> Bool {
        switch port.portType {
        case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP, .headphones, .airPlay, .carAudio, .usbAudio:
            return true
        default:
            return false
        }
    }

    private func iconForOutputPort(_ port: AVAudioSessionPortDescription) -> String {
        switch port.portType {
        case .bluetoothA2DP, .bluetoothLE, .bluetoothHFP:
            return "wave.3.right"
        case .headphones:
            return "headphones"
        case .airPlay:
            return "airplayaudio"
        case .carAudio:
            return "car"
        default:
            return "speaker.wave.2"
        }
    }

    private func iconForInputPort(_ port: AVAudioSessionPortDescription) -> String {
        switch port.portType {
        case .bluetoothHFP:
            return "wave.3.right"
        case .headsetMic:
            return "headphones"
        case .builtInMic:
            return "iphone"
        default:
            return "mic"
        }
    }
}

// MARK: - Notifications Tab Content

private struct NotificationsTabContent: View {
    @EnvironmentObject private var manager: VisioManager
    @State private var notifParticipant: Bool = true
    @State private var notifHandRaised: Bool = true
    @State private var notifMessage: Bool = true

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        List {
            Section(Strings.t("settings.incall.notifications", lang: lang)) {
                Toggle(isOn: $notifParticipant) {
                    Label(Strings.t("settings.incall.notifParticipant", lang: lang), systemImage: "person.badge.plus")
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                }
                .tint(VisioColors.primary500)
                .onChange(of: notifParticipant) { value in
                    manager.setNotificationParticipantJoin(value)
                }

                Toggle(isOn: $notifHandRaised) {
                    Label(Strings.t("settings.incall.notifHandRaised", lang: lang), systemImage: "hand.raised.fill")
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                }
                .tint(VisioColors.primary500)
                .onChange(of: notifHandRaised) { value in
                    manager.setNotificationHandRaised(value)
                }

                Toggle(isOn: $notifMessage) {
                    Label(Strings.t("settings.incall.notifMessage", lang: lang), systemImage: "message.fill")
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                }
                .tint(VisioColors.primary500)
                .onChange(of: notifMessage) { value in
                    manager.setNotificationMessageReceived(value)
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background(VisioColors.background(dark: isDark))
        .onAppear {
            let settings = manager.getSettings()
            notifParticipant = settings.notificationParticipantJoin
            notifHandRaised = settings.notificationHandRaised
            notifMessage = settings.notificationMessageReceived
        }
    }
}
