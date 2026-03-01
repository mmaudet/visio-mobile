import SwiftUI
import visioFFI

struct SettingsView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var displayName: String = ""
    @State private var micOnJoin: Bool = true
    @State private var cameraOnJoin: Bool = false
    @State private var language: String = "fr"

    var body: some View {
        NavigationStack {
            Form {
                Section("Profile") {
                    TextField("Display name", text: $displayName)
                        .autocorrectionDisabled()
                }

                Section("Join meeting") {
                    Toggle("Mic enabled", isOn: $micOnJoin)
                    Toggle("Camera enabled", isOn: $cameraOnJoin)
                }

                Section("Language") {
                    Picker("Language", selection: $language) {
                        Text("Francais").tag("fr")
                        Text("English").tag("en")
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                }
            }
            .scrollContentBackground(.hidden)
            .background(VisioColors.primaryDark50)
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(.dark, for: .navigationBar)
            .toolbarBackground(VisioColors.primaryDark75, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        save()
                        dismiss()
                    }
                    .foregroundStyle(VisioColors.primary500)
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                    .foregroundStyle(VisioColors.greyscale400)
                }
            }
        }
        .onAppear { load() }
        .preferredColorScheme(.dark)
    }

    private func load() {
        let settings = manager.getSettings()
        displayName = settings.displayName ?? ""
        micOnJoin = settings.micEnabledOnJoin
        cameraOnJoin = settings.cameraEnabledOnJoin
        language = settings.language ?? "fr"
    }

    private func save() {
        let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        manager.setDisplayName(name.isEmpty ? nil : name)
        manager.setMicEnabledOnJoin(micOnJoin)
        manager.setCameraEnabledOnJoin(cameraOnJoin)
        manager.setLanguage(language)
    }
}

#Preview {
    SettingsView()
        .environmentObject(VisioManager())
}
