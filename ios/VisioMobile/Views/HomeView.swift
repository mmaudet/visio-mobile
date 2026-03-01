import SwiftUI

struct HomeView: View {
    @EnvironmentObject private var manager: VisioManager

    @State private var roomURL: String = ""
    @State private var displayName: String = ""
    @State private var navigateToCall: Bool = false
    @State private var showSettings: Bool = false

    var body: some View {
        ZStack {
            VisioColors.primaryDark50.ignoresSafeArea()

            VStack(spacing: 32) {
                Spacer()

                // App branding
                VStack(spacing: 8) {
                    Image(systemName: "video.fill")
                        .font(.system(size: 48))
                        .foregroundStyle(VisioColors.primary500)
                    Text("Visio Mobile")
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .foregroundStyle(.white)
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
                .tint(VisioColors.primary500)
                .disabled(roomURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .padding(.horizontal, 32)

                Spacer()
                Spacer()
            }
        }
        .navigationTitle("Visio Mobile")
        .navigationBarTitleDisplayMode(.inline)
        .toolbarColorScheme(.dark, for: .navigationBar)
        .toolbarBackground(VisioColors.primaryDark75, for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showSettings = true
                } label: {
                    Image(systemName: "gearshape.fill")
                        .foregroundStyle(VisioColors.greyscale400)
                }
            }
        }
        .navigationDestination(isPresented: $navigateToCall) {
            CallView(
                roomURL: roomURL.trimmingCharacters(in: .whitespacesAndNewlines),
                displayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
                .environmentObject(manager)
        }
        .onAppear {
            // Pre-fill display name from settings
            let settings = manager.getSettings()
            if let savedName = settings.displayName, !savedName.isEmpty, displayName.isEmpty {
                displayName = savedName
            }
        }
        .preferredColorScheme(.dark)
    }
}

#Preview {
    NavigationStack {
        HomeView()
            .environmentObject(VisioManager())
    }
}
