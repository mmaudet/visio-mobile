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
            .onOpenURL { url in
                guard url.scheme == "visio",
                      let host = url.host
                else { return }

                // Handle auth callback
                if host == "auth-callback" {
                    manager.handleAuthCallback(url: url.absoluteString)
                    return
                }

                // Handle room deep link
                guard let slug = url.pathComponents.dropFirst().first else { return }
                let instances = manager.client.getMeetInstances()
                if instances.contains(host) {
                    manager.pendingDeepLink = "https://\(host)/\(slug)"
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
