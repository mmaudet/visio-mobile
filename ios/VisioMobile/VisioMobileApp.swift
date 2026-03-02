import SwiftUI

@main
struct VisioMobileApp: App {
    // Use the shared singleton so CallKit can access it
    @ObservedObject private var manager = VisioManager.shared

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
                      let host = url.host,
                      let slug = url.pathComponents.dropFirst().first
                else { return }

                let instances = manager.client.getMeetInstances()
                if instances.contains(host) {
                    manager.pendingDeepLink = "https://\(host)/\(slug)"
                }
            }
        }
    }
}
