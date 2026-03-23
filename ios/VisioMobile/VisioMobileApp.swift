import SwiftUI

@main
struct VisioMobileApp: App {
    // Use the shared singleton so CallKit can access it
    @ObservedObject private var manager = VisioManager.shared
    @Environment(\.scenePhase) private var scenePhase

    init() {
        Strings.initialize()
    }

    var body: some Scene {
        WindowGroup {
            NavigationStack {
                HomeView()
            }
            .environmentObject(manager)
            .preferredColorScheme(manager.currentTheme == "dark" ? .dark : .light)
            .onAppear { manager.initAuth() }
            .onOpenURL { url in
                #if DEBUG
                if url.scheme == "visio-test" && url.host == "connect" {
                    let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
                    let livekitUrl = components?.queryItems?.first(where: { $0.name == "livekit_url" })?.value
                    let token = components?.queryItems?.first(where: { $0.name == "token" })?.value
                    let mediaFile = components?.queryItems?.first(where: { $0.name == "media_file" })?.value
                    if let livekitUrl, let token {
                        NSLog("VisioMobileApp: test deep link → \(livekitUrl), media=\(mediaFile ?? "none")")
                        manager.pendingTestConnect = TestConnectParams(livekitUrl: livekitUrl, token: token, mediaFile: mediaFile)
                    }
                    return
                }
                #endif

                guard url.scheme == "visio",
                      let host = url.host,
                      let slug = url.pathComponents.dropFirst().first
                else { return }

                let instances = manager.client.getMeetInstances()
                if instances.contains(host) {
                    // Preserve room query param if present
                    let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
                    let rdName = components?.queryItems?.first(where: { $0.name == "visio" })?.value
                    var pendingURL = "https://\(host)/\(slug)"
                    if let name = rdName, !name.isEmpty {
                        var allowed = CharacterSet.urlQueryAllowed
                        allowed.remove(charactersIn: " +&=")
                        let encoded = name.addingPercentEncoding(withAllowedCharacters: allowed) ?? name
                        pendingURL += "?visio=\(encoded)"
                    }
                    manager.pendingDeepLink = pendingURL
                }
            }
            .onChange(of: scenePhase) { phase in
                switch phase {
                case .background:
                    manager.onAppBackgrounded()
                case .active:
                    manager.onAppForegrounded()
                case .inactive:
                    break
                @unknown default:
                    break
                }
            }
        }
    }
}
