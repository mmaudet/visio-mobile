import SwiftUI
import AVFoundation
import AVKit

// MARK: - MicLevelMonitor

/// Retains the AVAudioEngine and exposes mic level as a published property.
class MicLevelMonitor: ObservableObject {
    @Published var level: Float = 0
    private var engine: AVAudioEngine?

    func start() {
        guard engine == nil else { return }

        // Configure audio session for mic monitoring
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker, .allowBluetooth])
        try? session.setActive(true)

        let engine = AVAudioEngine()
        let inputNode = engine.inputNode
        let format = inputNode.outputFormat(forBus: 0)

        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            guard let data = buffer.floatChannelData?[0] else { return }
            let frameCount = Int(buffer.frameLength)
            var sumSq: Float = 0
            for i in 0..<frameCount {
                sumSq += data[i] * data[i]
            }
            let rms = sqrt(sumSq / Float(max(frameCount, 1)))
            DispatchQueue.main.async {
                self?.level = min(rms * 25.0, 1.0)
            }
        }

        try? engine.start()
        self.engine = engine
    }

    func stop() {
        engine?.inputNode.removeTap(onBus: 0)
        engine?.stop()
        engine = nil
        level = 0
    }

    deinit { stop() }
}

// MARK: - MicLevelView

struct MicLevelView: View {
    @StateObject private var monitor = MicLevelMonitor()

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.gray.opacity(0.3))
                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.green)
                    .frame(width: geometry.size.width * CGFloat(min(monitor.level, 1.0)))
                    .animation(.linear(duration: 0.1), value: monitor.level)
            }
        }
        .onAppear { monitor.start() }
        .onDisappear { monitor.stop() }
    }
}

// MARK: - WaitingState

enum WaitingState { case idle, waiting, denied, timeout }

// MARK: - BackgroundFilterSheet

private struct BackgroundFilterSheet: View {
    @Binding var backgroundMode: String
    let lang: String
    let isDark: Bool
    @EnvironmentObject private var manager: VisioManager

    var body: some View {
        NavigationStack {
            List {
                // Off
                Button { setMode("off") } label: {
                    HStack {
                        Image(systemName: "circle.slash").foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("prejoin.bgOff", lang: lang))
                            .foregroundStyle(VisioColors.onBackground(dark: isDark))
                        Spacer()
                        if backgroundMode == "off" {
                            Image(systemName: "checkmark").foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Blur
                Button { setMode("blur") } label: {
                    HStack {
                        Image(systemName: "aqi.medium").foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("prejoin.bgBlur", lang: lang))
                            .foregroundStyle(VisioColors.onBackground(dark: isDark))
                        Spacer()
                        if backgroundMode == "blur" {
                            Image(systemName: "checkmark").foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Blur light
                Button { setMode("blur-light") } label: {
                    HStack {
                        Image(systemName: "aqi.low").foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("prejoin.bgBlurLight", lang: lang))
                            .foregroundStyle(VisioColors.onBackground(dark: isDark))
                        Spacer()
                        if backgroundMode == "blur-light" {
                            Image(systemName: "checkmark").foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Background images grid
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
                                        .stroke(backgroundMode == "image:\(id)" ? VisioColors.primary500 : Color.clear, lineWidth: 2)
                                )
                                .onTapGesture { setMode("image:\(id)") }
                        }
                    }
                }
                .padding(.vertical, 4)
            }
            .navigationTitle(Strings.t("prejoin.backgroundFilters", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    private func setMode(_ mode: String) {
        backgroundMode = mode
        let client = manager.client
        Task.detached {
            if mode.hasPrefix("image:") {
                let id = UInt8(mode.dropFirst(6)) ?? 0
                if let path = Bundle.main.path(forResource: "\(id)", ofType: "jpg", inDirectory: "backgrounds") {
                    try? client.loadBackgroundImage(id: id, jpegPath: path)
                }
            }
            client.setBackgroundMode(mode: mode)
        }
    }
}

// MARK: - PreJoinView

struct PreJoinView: View {
    let roomURL: String
    let initialDisplayName: String
    var roomDisplayName: String? = nil

    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var displayName: String = ""
    @State private var isCameraOn = true
    @State private var isMicOn = true
    @State private var audioMode: AudioMode = .computer
    @State private var navigateToCall = false
    @State private var isFrontCamera = true
    @State private var showFilterSheet = false
    @State private var backgroundMode = "off"
    @State private var waitingState: WaitingState = .idle
    @State private var speakerTestPlayer: AVAudioPlayer?

    // Must be computed properties — currentTheme/currentLang are @Published instance props
    private var isDark: Bool { manager.currentTheme == "dark" }
    private var lang: String { manager.currentLang }

    enum AudioMode { case computer, none }

    var body: some View {
        let slug = roomURL.contains("/") ? String(roomURL.split(separator: "/").last ?? "") : roomURL

        ScrollView {
            VStack(spacing: 20) {
                // Room name
                if let name = roomDisplayName {
                    Text(name)
                        .font(.title2)
                        .fontWeight(.semibold)
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                    Text(slug)
                        .font(.subheadline)
                        .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                } else {
                    Text(slug)
                        .font(.title2)
                        .fontWeight(.semibold)
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                }

                // Display name
                TextField(Strings.t("prejoin.displayName", lang: lang), text: $displayName)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 300)

                // Camera preview section
                cameraPreviewSection

                // Audio config section (Task 15)
                audioConfigSection
                    .padding(.horizontal, 32)

                // Actions (Task 17)
                actionsSection
                    .padding(.horizontal, 32)
            }
            .padding(24)
        }
        .background(VisioColors.background(dark: isDark).ignoresSafeArea())
        .navigationBarBackButtonHidden(true)
        .onAppear {
            displayName = initialDisplayName
            let settings = manager.client.getSettings()
            isCameraOn = settings.cameraEnabledOnJoin
            isMicOn = settings.micEnabledOnJoin
        }
        .onChange(of: manager.connectionState) {
            if case .connected = manager.connectionState {
                waitingState = .idle
                navigateToCall = true
            }
        }
        .navigationDestination(isPresented: $navigateToCall) {
            CallView(
                roomURL: roomURL,
                displayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines),
                roomDisplayName: roomDisplayName
            )
        }
        .sheet(isPresented: $showFilterSheet) {
            BackgroundFilterSheet(backgroundMode: $backgroundMode, lang: lang, isDark: isDark)
                .presentationDetents([.medium])
                .environmentObject(manager)
        }
    }

    // MARK: - Camera Preview Section

    private var cameraPreviewSection: some View {
        VStack(spacing: 8) {
            // Camera preview area
            ZStack {
                if isCameraOn {
                    BlurredCameraPreviewView(isFront: isFrontCamera)
                        .aspectRatio(4.0/3.0, contentMode: .fit)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                } else {
                    RoundedRectangle(cornerRadius: 12)
                        .fill(Color.black)
                        .aspectRatio(4.0/3.0, contentMode: .fit)
                        .overlay(
                            Text(String((displayName.first ?? Character("?")).uppercased()))
                                .font(.system(size: 40, weight: .semibold))
                                .foregroundStyle(.white)
                                .frame(width: 72, height: 72)
                                .background(VisioColors.primary500)
                                .clipShape(Circle())
                        )
                }
            }

            // Camera controls: front/back toggle + on/off
            HStack {
                Button {
                    isFrontCamera.toggle()
                } label: {
                    Image(systemName: "arrow.triangle.2.circlepath.camera")
                        .foregroundStyle(VisioColors.primary500)
                }

                Spacer()

                Toggle(isOn: Binding(
                    get: { isCameraOn },
                    set: { newValue in
                        if newValue {
                            let status = AVCaptureDevice.authorizationStatus(for: .video)
                            switch status {
                            case .authorized:
                                isCameraOn = true
                            case .notDetermined:
                                AVCaptureDevice.requestAccess(for: .video) { granted in
                                    DispatchQueue.main.async { isCameraOn = granted }
                                }
                            default:
                                isCameraOn = false
                            }
                        } else {
                            isCameraOn = false
                        }
                    }
                )) {
                    Text(Strings.t("prejoin.camera", lang: lang))
                        .font(.subheadline)
                }
                .toggleStyle(.switch)
                .tint(VisioColors.primary500)
            }
            .padding(.horizontal, 12)

            // Background filters button (Task 16)
            Button {
                showFilterSheet = true
            } label: {
                Label(Strings.t("prejoin.backgroundFilters", lang: lang), systemImage: "camera.filters")
                    .font(.subheadline)
                    .foregroundStyle(VisioColors.primary500)
            }
        }
    }

    // MARK: - Audio Config Section (Task 15)

    private var audioConfigSection: some View {
        VStack(spacing: 8) {
            // Computer audio option
            Button {
                audioMode = .computer
            } label: {
                HStack {
                    Image(systemName: audioMode == .computer ? "largecircle.fill.circle" : "circle")
                        .foregroundStyle(VisioColors.primary500)
                    Text(Strings.t("prejoin.computerAudio", lang: lang))
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                    Spacer()
                }
                .padding(12)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(audioMode == .computer ? VisioColors.primary500 : VisioColors.border(dark: isDark), lineWidth: 1)
                )
            }

            if audioMode == .computer {
                // Mic toggle
                HStack {
                    Image(systemName: "mic.fill")
                        .foregroundStyle(VisioColors.primary500)
                        .frame(width: 20)
                    Text(Strings.t("prejoin.microphone", lang: lang))
                        .font(.subheadline)
                    Spacer()
                    Toggle("", isOn: Binding(
                        get: { isMicOn },
                        set: { newValue in
                            if newValue {
                                let status = AVCaptureDevice.authorizationStatus(for: .audio)
                                switch status {
                                case .authorized:
                                    isMicOn = true
                                case .notDetermined:
                                    AVCaptureDevice.requestAccess(for: .audio) { granted in
                                        DispatchQueue.main.async { isMicOn = granted }
                                    }
                                default:
                                    isMicOn = false
                                }
                            } else {
                                isMicOn = false
                            }
                        }
                    ))
                        .toggleStyle(.switch)
                        .tint(VisioColors.primary500)
                        .labelsHidden()
                }
                .padding(.horizontal, 12)

                // VU meter
                if isMicOn {
                    MicLevelView()
                        .frame(height: 4)
                        .padding(.horizontal, 36)
                }

                // Speaker test
                Button {
                    playSpeakerTest()
                } label: {
                    Label(Strings.t("prejoin.testSpeaker", lang: lang), systemImage: "speaker.wave.2")
                        .font(.subheadline)
                        .foregroundStyle(VisioColors.primary500)
                }
                .padding(.horizontal, 12)

                // Audio route picker
                HStack(spacing: 8) {
                    Image(systemName: "speaker.wave.2")
                        .foregroundStyle(VisioColors.primary500)
                        .frame(width: 20)
                    Text(Strings.t("prejoin.audioRoute", lang: lang))
                        .font(.subheadline)
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                    Spacer()
                    AudioRoutePickerButton(tintColor: UIColor(VisioColors.primary500))
                        .frame(width: 32, height: 32)
                }
                .padding(.horizontal, 12)
            }

            // No audio option
            Button {
                audioMode = .none
            } label: {
                HStack {
                    Image(systemName: audioMode == .none ? "largecircle.fill.circle" : "circle")
                        .foregroundStyle(VisioColors.primary500)
                    Text(Strings.t("prejoin.noAudio", lang: lang))
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                    Spacer()
                }
                .padding(12)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(audioMode == .none ? VisioColors.primary500 : VisioColors.border(dark: isDark), lineWidth: 1)
                )
            }
        }
    }

    private func playSpeakerTest() {
        guard let url = Bundle.main.url(forResource: "speaker-test", withExtension: "mp3") else { return }
        speakerTestPlayer = try? AVAudioPlayer(contentsOf: url)
        speakerTestPlayer?.play()
    }

    // MARK: - Actions Section (Task 17)

    private var actionsSection: some View {
        HStack(spacing: 12) {
            Button(Strings.t("prejoin.cancel", lang: lang)) {
                dismiss()
            }
            .buttonStyle(.bordered)
            .disabled(waitingState == .waiting)

            switch waitingState {
            case .idle:
                Button {
                    joinRoom()
                } label: {
                    Text(Strings.t("prejoin.joinNow", lang: lang))
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(VisioColors.primary500)

            case .waiting:
                Button {} label: {
                    HStack {
                        ProgressView().scaleEffect(0.7)
                        Text(Strings.t("prejoin.waitingForApproval", lang: lang))
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(VisioColors.primary500)
                .disabled(true)

            case .denied:
                VStack(spacing: 8) {
                    Text(Strings.t("prejoin.accessDenied", lang: lang))
                        .foregroundStyle(VisioColors.error500)
                        .font(.subheadline)
                    Button(Strings.t("prejoin.backToHome", lang: lang)) { dismiss() }
                        .buttonStyle(.bordered)
                }

            case .timeout:
                VStack(spacing: 8) {
                    Text(Strings.t("prejoin.requestTimeout", lang: lang))
                        .foregroundStyle(VisioColors.error500)
                        .font(.subheadline)
                    Button(Strings.t("prejoin.backToHome", lang: lang)) { dismiss() }
                        .buttonStyle(.bordered)
                }
            }
        }
    }

    private func joinRoom() {
        waitingState = .waiting

        // Save settings + display name
        let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        manager.client.setDisplayName(name: name.isEmpty ? nil : name)
        manager.client.setCameraEnabledOnJoin(enabled: isCameraOn)
        if audioMode == .none {
            manager.client.setMicEnabledOnJoin(enabled: false)
        } else {
            manager.client.setMicEnabledOnJoin(enabled: isMicOn)
        }

        // Append room query param so the FFI stores it in history
        var connectURL = roomURL
        if let rdName = roomDisplayName, !rdName.isEmpty {
            var allowed = CharacterSet.urlQueryAllowed
            allowed.remove(charactersIn: " +&=")
            let encoded = rdName.addingPercentEncoding(withAllowedCharacters: allowed) ?? rdName
            let separator = connectURL.contains("?") ? "&" : "?"
            connectURL += "\(separator)visio=\(encoded)"
        }

        manager.connect(url: connectURL, username: name.isEmpty ? nil : name)

        // Start 60s timeout
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { [self] in
            if waitingState == .waiting {
                waitingState = .timeout
            }
        }
    }
}
