import SwiftUI
import visioFFI

struct SettingsView: View {
    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var displayName: String = ""
    @State private var micOnJoin: Bool = true
    @State private var cameraOnJoin: Bool = false
    @State private var adaptiveModeEnabled: Bool = false
    @State private var language: String = Strings.detectSystemLang()
    @State private var theme: String = "light"
    @State private var meetInstances: [String] = ["visio.numerique.gouv.fr"]
    @State private var newInstance: String = ""
    @State private var calendarUrl: String = ""
    @State private var calendarRefreshInterval: CalendarRefreshInterval = .minutes15
    @State private var historyCleared: Bool = false

    private var lang: String { manager.currentLang }
    private var isDark: Bool { theme == "dark" }

    /// Normalizes a meet instance by stripping protocol prefixes and trailing slashes.
    /// Converts "https://meet.example.com/" to "meet.example.com".
    private func normalizeInstance(_ input: String) -> String {
        var result = input
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        if result.hasPrefix("https://") {
            result = String(result.dropFirst(8))
        } else if result.hasPrefix("http://") {
            result = String(result.dropFirst(7))
        }
        // Remove trailing slashes and any path
        if let slashIndex = result.firstIndex(of: "/") {
            result = String(result[..<slashIndex])
        }
        return result
    }

    var body: some View {
        NavigationStack {
            Form {
                Section(Strings.t("settings.profile", lang: lang)) {
                    TextField(Strings.t("settings.displayName", lang: lang), text: $displayName)
                        .autocorrectionDisabled()
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        .listRowBackground(VisioColors.surface(dark: isDark))
                }

                Section(Strings.t("settings.joinMeeting", lang: lang)) {
                    Toggle(Strings.t("settings.micOnJoin", lang: lang), isOn: $micOnJoin)
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        .listRowBackground(VisioColors.surface(dark: isDark))
                    Toggle(Strings.t("settings.camOnJoin", lang: lang), isOn: $cameraOnJoin)
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        .listRowBackground(VisioColors.surface(dark: isDark))
                    Toggle(Strings.t("settings.adaptiveMode", lang: lang), isOn: $adaptiveModeEnabled)
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        .listRowBackground(VisioColors.surface(dark: isDark))
                }

                Section(Strings.t("settings.theme", lang: lang)) {
                    ForEach(["light", "dark"], id: \.self) { option in
                        ThemeOptionRow(
                            label: Strings.t("settings.theme.\(option)", lang: lang),
                            isSelected: theme == option,
                            isDark: isDark,
                            onTap: {
                                theme = option
                                manager.setTheme(option)
                            }
                        )
                    }
                }

                Section(Strings.t("settings.language", lang: lang)) {
                    Picker(Strings.t("settings.language", lang: lang), selection: $language) {
                        ForEach(Strings.supportedLangs, id: \.self) { code in
                            Text(Strings.t("lang.\(code)", lang: code)).tag(code)
                        }
                    }
                    .pickerStyle(.menu)
                    .foregroundStyle(VisioColors.onSurface(dark: isDark))
                    .listRowBackground(VisioColors.surface(dark: isDark))
                    .onChange(of: language) { _ in
                        manager.setLanguage(language)
                    }
                }

                Section(Strings.t("settings.meetInstances", lang: lang)) {
                    ForEach(meetInstances, id: \.self) { instance in
                        HStack {
                            Text(instance)
                                .foregroundStyle(VisioColors.onSurface(dark: isDark))
                            Spacer()
                            Button {
                                meetInstances.removeAll { $0 == instance }
                            } label: {
                                Image(systemName: "minus.circle.fill")
                                    .foregroundStyle(.red)
                            }
                        }
                        .listRowBackground(VisioColors.surface(dark: isDark))
                    }
                    HStack {
                        TextField(Strings.t("settings.instancePlaceholder", lang: lang), text: $newInstance)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .keyboardType(.URL)
                            .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        Button {
                            let normalized = normalizeInstance(newInstance)
                            if !normalized.isEmpty && !meetInstances.contains(normalized) {
                                meetInstances.append(normalized)
                                newInstance = ""
                            }
                        } label: {
                            Image(systemName: "plus.circle.fill")
                                .foregroundStyle(VisioColors.primary500)
                        }
                        .disabled(newInstance.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                    .listRowBackground(VisioColors.surface(dark: isDark))
                }
                Section(Strings.t("settings.calendar", lang: lang)) {
                    TextField(Strings.t("settings.calendarUrl", lang: lang), text: $calendarUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .foregroundStyle(VisioColors.onSurface(dark: isDark))
                        .listRowBackground(VisioColors.surface(dark: isDark))
                        .onChange(of: calendarUrl) { _ in
                            let trimmed = calendarUrl.trimmingCharacters(in: .whitespacesAndNewlines)
                            manager.client.setCalendarUrl(url: trimmed.isEmpty ? nil : trimmed)
                            if !trimmed.isEmpty {
                                manager.requestNotificationPermissionIfNeeded()
                            }
                        }

                    Picker(Strings.t("settings.calendarRefresh", lang: lang), selection: $calendarRefreshInterval) {
                        Text(Strings.t("settings.calendarRefresh.5min", lang: lang)).tag(CalendarRefreshInterval.minutes5)
                        Text(Strings.t("settings.calendarRefresh.15min", lang: lang)).tag(CalendarRefreshInterval.minutes15)
                        Text(Strings.t("settings.calendarRefresh.1h", lang: lang)).tag(CalendarRefreshInterval.hour1)
                        Text(Strings.t("settings.calendarRefresh.4h", lang: lang)).tag(CalendarRefreshInterval.hours4)
                        Text(Strings.t("settings.calendarRefresh.manual", lang: lang)).tag(CalendarRefreshInterval.manual)
                    }
                    .foregroundStyle(VisioColors.onSurface(dark: isDark))
                    .listRowBackground(VisioColors.surface(dark: isDark))
                    .onChange(of: calendarRefreshInterval) { _ in
                        manager.client.setCalendarRefreshInterval(interval: calendarRefreshInterval)
                    }

                    if !calendarUrl.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Button(role: .destructive) {
                            calendarUrl = ""
                            manager.client.setCalendarUrl(url: nil)
                        } label: {
                            Text(Strings.t("settings.calendarRemove", lang: lang))
                        }
                        .listRowBackground(VisioColors.surface(dark: isDark))
                    }
                }

                Section {
                    Button(role: .destructive) {
                        manager.client.clearVisioHistory()
                        historyCleared = true
                    } label: {
                        Text(Strings.t("settings.clearHistory", lang: lang))
                    }
                    .listRowBackground(VisioColors.surface(dark: isDark))
                }
            }
            .scrollContentBackground(.hidden)
            .background(VisioColors.background(dark: isDark))
            .navigationTitle(Strings.t("settings", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
            .toolbarBackground(VisioColors.surface(dark: isDark), for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .appToolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(Strings.t("settings.done", lang: lang)) {
                        dismiss()
                    }
                    .foregroundStyle(VisioColors.primary500)
                }
            }
        }
        .preferredColorScheme(isDark ? .dark : .light)
        .overlay(alignment: .bottom) {
            if historyCleared {
                Text(Strings.t("settings.historyCleared", lang: lang))
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(
                        RoundedRectangle(cornerRadius: 10)
                            .fill(VisioColors.primary500.opacity(0.9))
                    )
                    .foregroundStyle(.white)
                    .padding(.bottom, 40)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .onAppear {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                            withAnimation { historyCleared = false }
                        }
                    }
            }
        }
        .animation(.easeInOut(duration: 0.3), value: historyCleared)
        .onDisappear { save() }
        .onAppear { load() }
    }

    private func load() {
        let settings = manager.getSettings()
        displayName = settings.displayName ?? ""
        micOnJoin = settings.micEnabledOnJoin
        cameraOnJoin = settings.cameraEnabledOnJoin
        language = settings.language ?? Strings.detectSystemLang()
        theme = settings.theme
        adaptiveModeEnabled = manager.client.isAdaptiveModeEnabled()
        meetInstances = manager.client.getMeetInstances()
        calendarUrl = manager.client.getCalendarUrl() ?? ""
        calendarRefreshInterval = manager.client.getCalendarRefreshInterval()
    }

    private func save() {
        let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        manager.setDisplayName(name.isEmpty ? nil : name)
        manager.updateDisplayName(name)
        manager.setMicEnabledOnJoin(micOnJoin)
        manager.setCameraEnabledOnJoin(cameraOnJoin)
        manager.setLanguage(language)
        let wasEnabled = manager.client.isAdaptiveModeEnabled()
        manager.client.setAdaptiveModeEnabled(enabled: adaptiveModeEnabled)
        if wasEnabled && !adaptiveModeEnabled {
            manager.stopContextDetection()
        } else if !wasEnabled && adaptiveModeEnabled {
            manager.startContextDetection()
        }
        manager.client.setMeetInstances(instances: meetInstances)
    }
}

private struct PressedKey: PreferenceKey {
    static let defaultValue = false
    static func reduce(value: inout Bool, nextValue: () -> Bool) {
        value = value || nextValue()
    }
}

private struct ThemeRowStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .preference(key: PressedKey.self, value: configuration.isPressed)
    }
}

private struct ThemeOptionRow: View {
    let label: String
    let isSelected: Bool
    let isDark: Bool
    let onTap: () -> Void

    @State private var pressed = false

    var body: some View {
        Button(action: onTap) {
            HStack {
                Text(label)
                    .foregroundStyle(VisioColors.onSurface(dark: isDark))
                Spacer()
                if isSelected {
                    Image(systemName: "checkmark")
                        .foregroundStyle(VisioColors.primary500)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(ThemeRowStyle())
        .onPreferenceChange(PressedKey.self) { pressed = $0 }
        .listRowBackground(
            pressed
                ? VisioColors.surfaceVariant(dark: isDark)
                : VisioColors.surface(dark: isDark)
        )
    }
}

#Preview {
    SettingsView()
        .environmentObject(VisioManager())
}
