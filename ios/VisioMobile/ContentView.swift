import SwiftUI

struct ContentView: View {
    var body: some View {
        VStack(spacing: 16) {
            Text("Visio")
                .font(.largeTitle)
                .fontWeight(.bold)
            Text("Phase 0 — iOS skeleton")
                .foregroundColor(.secondary)
        }
    }
}

#Preview {
    ContentView()
}
