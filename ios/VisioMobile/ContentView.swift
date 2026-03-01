import SwiftUI

/// Legacy placeholder view — kept for reference.
/// The app now uses HomeView as the root content.
struct ContentView: View {
    var body: some View {
        HomeView()
    }
}

#Preview {
    NavigationStack {
        ContentView()
            .environmentObject(VisioManager())
    }
}
