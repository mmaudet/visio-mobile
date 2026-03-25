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
                      let pathSegment = url.pathComponents.dropFirst().first
                else { return }

                let instances = manager.client.getMeetInstances()
                guard instances.contains(host) else { return }

                let slugPattern = /^[a-z]{3}-[a-z]{4}-[a-z]{3}$/
                let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
                let rdName = components?.queryItems?.first(where: { $0.name == "visio" })?.value

                if pathSegment.wholeMatch(of: slugPattern) != nil {
                    var pendingURL = "https://\(host)/\(pathSegment)"
                    if let name = rdName, !name.isEmpty {
                        var allowed = CharacterSet.urlQueryAllowed
                        allowed.remove(charactersIn: " +&=")
                        let encoded = name.addingPercentEncoding(withAllowedCharacters: allowed) ?? name
                        pendingURL += "?visio=\(encoded)"
                    }
                    manager.pendingDeepLink = pendingURL
                } else if let resolved = manager.client.resolveVisioAlias(name: pathSegment) {
                    if let name = rdName, !name.isEmpty {
                        var allowed = CharacterSet.urlQueryAllowed
                        allowed.remove(charactersIn: " +&=")
                        let encoded = name.addingPercentEncoding(withAllowedCharacters: allowed) ?? name
                        manager.pendingDeepLink = "\(resolved)?visio=\(encoded)"
                    } else {
                        manager.pendingDeepLink = resolved
                    }
                } else {
                    manager.pendingDeepLinkError = Strings.t("error.unknownAlias", lang: manager.currentLang)
                        .replacingOccurrences(of: "{name}", with: pathSegment)
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
