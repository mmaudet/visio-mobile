import SwiftUI

struct HomeView: View {
    @EnvironmentObject private var manager: VisioManager

    @State private var roomURL: String = ""
    @State private var displayName: String = ""
    @State private var navigateToCall: Bool = false

    var body: some View {
        VStack(spacing: 32) {
            Spacer()

            // App branding
            VStack(spacing: 8) {
                Image(systemName: "video.fill")
                    .font(.system(size: 48))
                    .foregroundStyle(.blue)
                Text("Visio")
                    .font(.largeTitle)
                    .fontWeight(.bold)
            }

            // Input fields
            VStack(spacing: 16) {
                TextField("meet.example.com/room-name", text: $roomURL)
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)

                TextField("Display name (optional)", text: $displayName)
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.words)
            }
            .padding(.horizontal, 32)

            // Join button
            Button {
                navigateToCall = true
            } label: {
                Label("Join", systemImage: "phone.fill")
                    .font(.headline)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.borderedProminent)
            .disabled(roomURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .padding(.horizontal, 32)

            Spacer()
            Spacer()
        }
        .navigationTitle("Visio")
        .navigationDestination(isPresented: $navigateToCall) {
            CallView(
                roomURL: roomURL.trimmingCharacters(in: .whitespacesAndNewlines),
                displayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
    }
}

#Preview {
    NavigationStack {
        HomeView()
            .environmentObject(VisioManager())
    }
}
