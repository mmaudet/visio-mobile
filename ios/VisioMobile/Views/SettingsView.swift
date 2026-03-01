import SwiftUI
import visioFFI

struct SettingsView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var displayName: String = ""
    @State private var micOnJoin: Bool = true
    @State private var cameraOnJoin: Bool = false
    @State private var language: String = Strings.detectSystemLang()
    @State private var theme: String = "light"

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        NavigationStack {
            Form {
                Section(Strings.t("settings.profile", lang: lang)) {
                    TextField(Strings.t("settings.displayName", lang: lang), text: $displayName)
                        .autocorrectionDisabled()
                }

                Section(Strings.t("settings.joinMeeting", lang: lang)) {
                    Toggle(Strings.t("settings.micOnJoin", lang: lang), isOn: $micOnJoin)
                    Toggle(Strings.t("settings.camOnJoin", lang: lang), isOn: $cameraOnJoin)
                }

                Section(Strings.t("settings.theme", lang: lang)) {
                    Picker(Strings.t("settings.theme", lang: lang), selection: $theme) {
                        Text(Strings.t("settings.theme.light", lang: lang)).tag("light")
                        Text(Strings.t("settings.theme.dark", lang: lang)).tag("dark")
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                    .onChange(of: theme) { newTheme in
                        manager.setTheme(newTheme)
                    }
                }

                Section(Strings.t("settings.language", lang: lang)) {
                    Picker(Strings.t("settings.language", lang: lang), selection: $language) {
                        ForEach(Strings.supportedLangs, id: \.self) { code in
                            Text(Strings.t("lang.\(code)", lang: code)).tag(code)
                        }
                    }
                    .pickerStyle(.menu)
                    .onChange(of: language) { newLang in
                        manager.setLanguage(newLang)
                    }
                }
            }
            .scrollContentBackground(.hidden)
            .background(VisioColors.background(dark: isDark))
            .navigationTitle(Strings.t("settings", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
            .toolbarBackground(VisioColors.surface(dark: isDark), for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(Strings.t("settings.save", lang: lang)) {
                        save()
                        dismiss()
                    }
                    .foregroundStyle(VisioColors.primary500)
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button(Strings.t("settings.cancel", lang: lang)) {
                        dismiss()
                    }
                    .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                }
            }
        }
        .onAppear { load() }
    }

    private func load() {
        let settings = manager.getSettings()
        displayName = settings.displayName ?? ""
        micOnJoin = settings.micEnabledOnJoin
        cameraOnJoin = settings.cameraEnabledOnJoin
        language = settings.language ?? Strings.detectSystemLang()
        theme = settings.theme ?? "light"
    }

    private func save() {
        let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        manager.setDisplayName(name.isEmpty ? nil : name)
        manager.updateDisplayName(name)
        manager.setMicEnabledOnJoin(micOnJoin)
        manager.setCameraEnabledOnJoin(cameraOnJoin)
        manager.setLanguage(language)
    }
}

#Preview {
    SettingsView()
        .environmentObject(VisioManager())
}
