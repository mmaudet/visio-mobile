import SwiftUI

@main
struct VisioMobileApp: App {
    @StateObject private var manager = VisioManager()

    var body: some Scene {
        WindowGroup {
            NavigationStack {
                HomeView()
            }
            .environmentObject(manager)
        }
    }
}
