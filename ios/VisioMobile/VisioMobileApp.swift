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
            .preferredColorScheme(.dark)
        }
    }
}
