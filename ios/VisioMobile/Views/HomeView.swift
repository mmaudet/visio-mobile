import SwiftUI

struct HomeView: View {
    @EnvironmentObject private var manager: VisioManager

    @State private var roomURL: String = ""
    @State private var displayName: String = ""
    @State private var navigateToCall: Bool = false
    @State private var showSettings: Bool = false

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    var body: some View {
        ZStack {
            VisioColors.background(dark: isDark).ignoresSafeArea()

            VStack(spacing: 32) {
                Spacer()

                // App branding with tricolore logo
                VStack(spacing: 8) {
                    VisioLogo(size: 64)
                    Text(Strings.t("app.title", lang: lang))
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                }

                Text(Strings.t("home.subtitle", lang: lang))
                    .font(.subheadline)
                    .foregroundStyle(VisioColors.secondaryText(dark: isDark))

                // Input fields
                VStack(spacing: 16) {
                    TextField(Strings.t("home.meetUrl.placeholder", lang: lang), text: $roomURL)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)

                    TextField(Strings.t("home.displayName", lang: lang), text: $displayName)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.words)
                }
                .padding(.horizontal, 32)

                // Join button
                Button {
                    navigateToCall = true
                } label: {
                    Label(Strings.t("home.join", lang: lang), systemImage: "phone.fill")
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
        .navigationTitle(Strings.t("app.title", lang: lang))
        .navigationBarTitleDisplayMode(.inline)
        .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
        .toolbarBackground(VisioColors.surface(dark: isDark), for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showSettings = true
                } label: {
                    Image(systemName: "gearshape.fill")
                        .foregroundStyle(VisioColors.secondaryText(dark: isDark))
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
            // Pre-fill display name from manager
            let name = manager.displayName
            if !name.isEmpty && displayName.isEmpty {
                displayName = name
            }
        }
    }
}

#Preview {
    NavigationStack {
        HomeView()
            .environmentObject(VisioManager())
    }
}
