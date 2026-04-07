import Combine
import SwiftUI
import visioFFI

struct HomeView: View {
    @EnvironmentObject private var manager: VisioManager

    @State private var roomURL: String = ""
    @State private var resolvedRoomURL: String = ""
    @State private var displayName: String = ""
    @State private var navigateToCall: Bool = false
    @State private var isTestConnect: Bool = false
    @State private var showSettings: Bool = false
    @State private var roomStatus: String = "idle"
    @State private var meetInstances: [String] = []
    @State private var showServerPicker: Bool = false
    @State private var customServer: String = ""
    @State private var showCreateRoom: Bool = false
    @State private var roomDisplayName: String = ""
    @State private var roomHistory: [VisioHistoryEntry] = []
    @State private var historyJoinPending: Bool = false
    @State private var showCompactHeader: Bool = false
    @State private var selectedTab: Int = 0
    @State private var syncToastMessage: String?
    @State private var syncToastIsError: Bool = false
    @State private var now = Date()
    private let minuteTimer = Timer.publish(every: 60, on: .main, in: .common).autoconnect()

    private let oidcEnabled: Bool = isOidcEnabled()

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

    private var hasImminentMeeting: Bool {
        let nowTs = now.timeIntervalSince1970
        return manager.upcomingMeetings.contains { meeting in
            let start = TimeInterval(meeting.startTime)
            let end = TimeInterval(meeting.endTime)
            let minutesUntil = (start - nowTs) / 60
            return (minutesUntil >= 0 && minutesUntil < 15) || (start <= nowTs && end > nowTs)
        }
    }

    private static let slugPattern = /^[a-z]{3}-[a-z]{4}-[a-z]{3}$/

    private func extractSlug(_ input: String) -> String? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let candidate = trimmed.contains("/")
            ? String(trimmed.split(separator: "/").last ?? "")
            : trimmed
        return candidate.wholeMatch(of: Self.slugPattern) != nil ? candidate : nil
    }

    var body: some View {
        ZStack {
            VisioColors.background(dark: isDark).ignoresSafeArea()

            TabView(selection: $selectedTab) {
                // Tab 0: Join
                ScrollView {
                VStack(spacing: 32) {
                    VStack(spacing: 8) {
                        VisioLogo(size: 96)
                        Text(Strings.t("app.title", lang: lang))
                            .font(.largeTitle)
                            .fontWeight(.bold)
                            .foregroundStyle(VisioColors.onBackground(dark: isDark))
                    }
                    .padding(.top, 16)
                    .background(GeometryReader { geo in
                        Color.clear.onChange(of: geo.frame(in: .named("scroll")).minY) { _ in
                            showCompactHeader = geo.frame(in: .named("scroll")).minY < -20
                        }
                    })

                // Authentication section
                if oidcEnabled {
                    if manager.isAuthenticated {
                        AuthenticatedCard(
                            displayName: manager.authenticatedDisplayName,
                            email: manager.authenticatedEmail,
                            isDark: isDark,
                            lang: lang,
                            onLogout: { manager.logoutSession() }
                        )
                        .padding(.horizontal, 32)
                    } else {
                        Button(action: {
                            if meetInstances.count <= 1 {
                                guard let meetInstance = meetInstances.first else { return }
                                launchOidc(meetInstance: meetInstance)
                            } else {
                                customServer = ""
                                showServerPicker = true
                            }
                        }) {
                            Label(Strings.t("home.connect", lang: lang), systemImage: "person.circle")
                                .font(.headline)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 12)
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(VisioColors.primary500)
                        .padding(.horizontal, 32)
                    }
                }

                // Input fields
                VStack(spacing: 16) {
                    TextField(Strings.t("home.meetUrl.placeholder", lang: lang), text: $roomURL)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)

                    if roomStatus == "checking" {
                        Text(Strings.t("home.room.checking", lang: lang))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else if roomStatus == "valid" {
                        Text(Strings.t("home.room.valid", lang: lang))
                            .font(.caption)
                            .foregroundStyle(.green)
                    } else if roomStatus == "not_found" {
                        Text(Strings.t("home.room.notFound", lang: lang))
                            .font(.caption)
                            .foregroundStyle(.red)
                    }

                    TextField(Strings.t("home.roomDisplayName", lang: lang), text: $roomDisplayName)
                        .textFieldStyle(.roundedBorder)

                    TextField(Strings.t("home.displayName", lang: lang), text: $displayName)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.words)
                }
                .padding(.horizontal, 32)
                .task(id: roomURL) {
                    let trimmed = roomURL.trimmingCharacters(in: .whitespacesAndNewlines)
                    let isSlug = trimmed.wholeMatch(of: Self.slugPattern) != nil

                    // Build list of URLs to try
                    let urlsToTry: [String]
                    if isSlug, !meetInstances.isEmpty {
                        urlsToTry = meetInstances.map { "https://\($0)/\(trimmed)" }
                    } else {
                        if extractSlug(trimmed) != nil {
                            urlsToTry = [trimmed]
                        } else {
                            // Try alias resolution
                            let candidate = trimmed.contains("/")
                                ? String(trimmed.trimmingCharacters(in: CharacterSet(charactersIn: "/")).split(separator: "/").last ?? "")
                                : trimmed
                            if let aliasUrl = manager.client.resolveVisioAlias(name: candidate) {
                                urlsToTry = [aliasUrl]
                            } else {
                                roomStatus = "idle"
                                resolvedRoomURL = trimmed
                                return
                            }
                        }
                    }

                    roomStatus = "checking"
                    try? await Task.sleep(for: .milliseconds(500))
                    guard !Task.isCancelled else { return }

                    let uname = displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ? nil : displayName.trimmingCharacters(in: .whitespacesAndNewlines)

                    var foundValid = false
                    for url in urlsToTry {
                        guard !Task.isCancelled else { return }
                        let result = manager.client.validateRoom(url: url, username: uname)
                        if case .valid = result {
                            roomStatus = "valid"
                            resolvedRoomURL = url
                            foundValid = true
                            break
                        }
                    }
                    if !foundValid {
                        roomStatus = "not_found"
                        resolvedRoomURL = urlsToTry.first ?? trimmed
                    }
                }

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
                .disabled(roomStatus != "valid")
                .padding(.horizontal, 32)

                if oidcEnabled && manager.isAuthenticated {
                    Button {
                        showCreateRoom = true
                    } label: {
                        Label(Strings.t("home.createRoom", lang: lang), systemImage: "plus.rectangle")
                            .font(.headline)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                    }
                    .buttonStyle(.bordered)
                    .tint(VisioColors.primary500)
                    .padding(.horizontal, 32)
                }

                // Room history
                if !roomHistory.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(Strings.t("home.recentRooms", lang: lang))
                            .font(.subheadline)
                            .fontWeight(.medium)
                            .foregroundStyle(VisioColors.secondaryText(dark: isDark))

                        ForEach(Array(roomHistory.enumerated()), id: \.offset) { index, entry in
                            let slug = entry.url.contains("/") ? String(entry.url.split(separator: "/").last ?? "") : entry.url
                            let host = URL(string: entry.url)?.host ?? ""

                            Button {
                                roomURL = entry.url
                                resolvedRoomURL = entry.url
                                if let name = entry.displayName {
                                    roomDisplayName = name
                                }
                                // If already validated, navigate immediately
                                if roomStatus == "valid" {
                                    navigateToCall = true
                                } else {
                                    historyJoinPending = true
                                }
                            } label: {
                                HStack(spacing: 10) {
                                    if historyJoinPending && roomURL == entry.url {
                                        ProgressView()
                                            .scaleEffect(0.7)
                                            .frame(width: 14, height: 14)
                                    } else {
                                        Image(systemName: "globe")
                                            .font(.system(size: 14))
                                            .foregroundStyle(VisioColors.primary500)
                                    }

                                    VStack(alignment: .leading, spacing: 2) {
                                        if let name = entry.displayName {
                                            Text(name)
                                                .font(.body)
                                                .fontWeight(.bold)
                                                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                                            Text("\(slug) · \(host)")
                                                .font(.caption)
                                                .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                                        } else {
                                            Text(slug)
                                                .font(.body)
                                                .fontWeight(.medium)
                                                .foregroundStyle(VisioColors.onBackground(dark: isDark))
                                            if !host.isEmpty {
                                                Text(host)
                                                    .font(.caption)
                                                    .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                                            }
                                        }
                                    }

                                    Spacer()
                                }
                                .padding(.horizontal, 12)
                                .padding(.vertical, 10)
                                .background(
                                    RoundedRectangle(cornerRadius: 8)
                                        .fill(isDark
                                            ? Color(red: 0.12, green: 0.12, blue: 0.18)
                                            : Color(red: 0.95, green: 0.95, blue: 0.97))
                                )
                            }
                            .disabled(historyJoinPending)
                        }
                    }
                    .padding(.horizontal, 32)
                }

                }
                .padding(.bottom, 32)
            }
            .coordinateSpace(name: "scroll")
            .tag(0)
            .tabItem {
                Label(Strings.t("home.tab.join", lang: lang), systemImage: "video.fill")
            }

                // Tab 1: Meetings
                MeetingsTabView(
                    meetings: manager.upcomingMeetings,
                    hasCalendarUrl: manager.client.getCalendarUrl() != nil,
                    isLoading: manager.calendarLoading,
                    isDark: isDark,
                    lang: lang,
                    onSettings: { showSettings = true },
                    onRefresh: { manager.refreshCalendarNow() },
                    onJoinMeeting: { meeting in
                        roomURL = meeting.roomUrl
                        resolvedRoomURL = meeting.roomUrl
                        roomStatus = "valid"
                        selectedTab = 0
                        navigateToCall = true
                    }
                )
                .tag(1)
                .tabItem {
                    Label(Strings.t("home.tab.meetings", lang: lang), systemImage: "calendar")
                }
                .badge(hasImminentMeeting ? 1 : 0)
            }
            .onChange(of: selectedTab) { _ in
                if selectedTab == 1 { manager.refreshCalendarNow() }
            }
        }
        .navigationBarTitleDisplayMode(.inline)
        .toolbarColorScheme(isDark ? .dark : .light, for: .navigationBar)
        .toolbarBackground(VisioColors.background(dark: isDark), for: .navigationBar)
        .toolbarBackground(showCompactHeader ? .visible : .hidden, for: .navigationBar)
        .appToolbar {
            ToolbarItem(placement: .principal) {
                HStack(spacing: 6) {
                    VisioLogo(size: 24)
                    Text(Strings.t("app.title", lang: lang))
                        .font(.headline)
                        .foregroundStyle(VisioColors.onBackground(dark: isDark))
                }
                .opacity(showCompactHeader ? 1 : 0)
                .animation(.easeInOut(duration: 0.2), value: showCompactHeader)
            }
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
             callDestination
         }
        .sheet(isPresented: $showSettings) {
            SettingsView()
                .environmentObject(manager)
        }
        .onReceive(minuteTimer) { _ in now = Date() }
        .onAppear {
            // Pre-fill display name from manager (includes OIDC identity)
            let name = manager.displayName
            if !name.isEmpty && displayName.isEmpty {
                displayName = name
            }
            // Load meet instances
            meetInstances = manager.client.getMeetInstances()
            // Load room history
            roomHistory = manager.client.getVisioHistory()
            // Load cached meetings immediately, then refresh from network
            let cached = manager.client.getUpcomingMeetings()
            if !cached.isEmpty {
                manager.upcomingMeetings = cached
            }
            if manager.client.getCalendarUrl() != nil {
                manager.refreshCalendarNow()
            }
        }
        .onChange(of: manager.authenticatedDisplayName) { _ in
            if !manager.authenticatedDisplayName.isEmpty && displayName.isEmpty {
                displayName = manager.authenticatedDisplayName
            }
        }
        .onChange(of: roomStatus) { _ in
            guard historyJoinPending else { return }
            if roomStatus == "valid" {
                historyJoinPending = false
                navigateToCall = true
            } else if roomStatus == "not_found" || roomStatus == "idle" {
                // Validation failed — fall back to just showing the URL in the field
                historyJoinPending = false
            }
        }
        .onChange(of: manager.pendingDeepLink) { _ in
            if let link = manager.pendingDeepLink {
                // Extract room display name from the URL if present, then strip the param
                if let extracted = manager.client.extractRoomDisplayName(url: link) {
                    roomDisplayName = extracted
                }
                // Strip the query param from the URL shown in the field
                let cleanURL = link.components(separatedBy: "?").first ?? link
                roomURL = cleanURL
                resolvedRoomURL = cleanURL
                manager.pendingDeepLink = nil
                // Navigate directly to the pre-join setup page
                navigateToCall = true
            }
        }
        .onChange(of: manager.pendingTestConnect != nil) { _ in
            if manager.pendingTestConnect != nil {
                isTestConnect = true
                navigateToCall = true
            }
        }
        .onChange(of: manager.calendarSyncResult) { _ in
            guard let result = manager.calendarSyncResult else { return }
            switch result {
            case .success(let count):
                if count > 0 {
                    syncToastMessage = Strings.t("calendar.sync.success", lang: lang)
                        .replacingOccurrences(of: "{count}", with: "\(count)")
                } else {
                    syncToastMessage = Strings.t("calendar.sync.noMeetings", lang: lang)
                }
                syncToastIsError = false
            case .error:
                syncToastMessage = Strings.t("calendar.sync.error", lang: lang)
                syncToastIsError = true
            }
            manager.calendarSyncResult = nil
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                syncToastMessage = nil
            }
        }
        .overlay(alignment: .bottom) {
            if let message = syncToastMessage {
                Text(message)
                    .font(.subheadline)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(
                        Capsule()
                            .fill(syncToastIsError ? Color.red.opacity(0.9) : VisioColors.primary500.opacity(0.9))
                    )
                    .padding(.bottom, 24)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .animation(.easeInOut(duration: 0.3), value: syncToastMessage)
            }
        }
        .sheet(isPresented: $showCreateRoom) {
            CreateRoomSheet(
                lang: lang,
                onCreated: { roomUrl in
                    showCreateRoom = false
                    roomURL = roomUrl
                    resolvedRoomURL = roomUrl
                    roomStatus = "valid"
                    navigateToCall = true
                },
                onCancel: { showCreateRoom = false }
            )
            .environmentObject(manager)
        }
        .sheet(isPresented: $showServerPicker) {
            ServerPickerWithOidc(
                instances: meetInstances,
                customServer: $customServer,
                lang: lang,
                onDismiss: { showServerPicker = false }
            )
        }
        .alert(
            Strings.t("call.error", lang: lang),
            isPresented: Binding(
                get: { manager.pendingDeepLinkError != nil },
                set: { if !$0 { manager.pendingDeepLinkError = nil } }
            )
        ) {
            Button("OK") { manager.pendingDeepLinkError = nil }
        } message: {
            Text(manager.pendingDeepLinkError ?? "")
        }
    }

    @ViewBuilder
    private var callDestination: some View {
        if isTestConnect {
            CallView(roomURL: "e2e-test", displayName: "iOS User")
        } else {
            PreJoinView(
                roomURL: resolvedRoomURL,
                initialDisplayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines),
                roomDisplayName: roomDisplayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    ? nil
                    : roomDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
                isPresented: $navigateToCall
            )
        }
    }

    private func launchOidc(meetInstance: String) {
        manager.authManager.launchOidcFlow(meetInstance: meetInstance) { [weak manager] code in
            guard let code, let manager else { return }
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let sessionId = try manager.client.exchangeOidcCode(meetInstance: meetInstance, code: code)
                    DispatchQueue.main.async {
                        manager.onAuthCookieReceived(sessionId, meetInstance: meetInstance)
                    }
                } catch {
                    NSLog("[HomeView] OIDC code exchange failed: \(error)")
                }
            }
        }
    }
}

// MARK: - Server Picker

/// Server picker that navigates to the OIDC web view within the same sheet.
private struct ServerPickerWithOidc: View {
    let instances: [String]
    @Binding var customServer: String
    let lang: String
    let onDismiss: () -> Void

    @EnvironmentObject private var manager: VisioManager

    /// Normalizes a meet instance by stripping protocol prefixes and trailing slashes.
    private func normalizeInstance(_ input: String) -> String {
        var result = input
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        if result.hasPrefix("https://") {
            result = String(result.dropFirst(8))
        } else if result.hasPrefix("http://") {
            result = String(result.dropFirst(7))
        }
        if let slashIndex = result.firstIndex(of: "/") {
            result = String(result[..<slashIndex])
        }
        return result
    }

    private func selectInstance(_ instance: String) {
        onDismiss()
        // Use ASWebAuthenticationSession + exchange code
        manager.authManager.launchOidcFlow(meetInstance: instance) { [weak manager] code in
            guard let code, let manager else { return }
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let sessionId = try manager.client.exchangeOidcCode(meetInstance: instance, code: code)
                    DispatchQueue.main.async {
                        manager.onAuthCookieReceived(sessionId, meetInstance: instance)
                    }
                } catch {
                    NSLog("[ServerPicker] OIDC code exchange failed: \(error)")
                }
            }
        }
    }

    var body: some View {
        NavigationStack {
            // Server picker list
            List {
                Section {
                    ForEach(instances, id: \.self) { instance in
                        Button(instance) {
                            selectInstance(instance)
                        }
                    }
                }
                Section {
                    TextField(Strings.t("home.serverPicker.custom", lang: lang), text: $customServer)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                    Button(Strings.t("home.connect", lang: lang)) {
                        let normalized = normalizeInstance(customServer)
                        if !normalized.isEmpty {
                            selectInstance(normalized)
                        }
                    }
                    .disabled(customServer.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .navigationTitle(Strings.t("home.serverPicker.title", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(Strings.t("home.serverPicker.cancel", lang: lang)) {
                        onDismiss()
                    }
                }
            }
        }
    }
}

// MARK: - Authenticated Card

private struct AuthenticatedCard: View {
    let displayName: String
    let email: String
    let isDark: Bool
    let lang: String
    let onLogout: () -> Void

    private var initials: String {
        let parts = displayName.split(separator: " ").prefix(2)
        let result = parts.compactMap { $0.first?.uppercased() }.joined()
        if !result.isEmpty { return result }
        return email.first?.uppercased() ?? "?"
    }

    var body: some View {
        HStack(spacing: 12) {
            // Avatar circle
            ZStack {
                Circle()
                    .fill(VisioColors.primary500)
                    .frame(width: 44, height: 44)
                Text(initials)
                    .font(.system(size: 16, weight: .bold))
                    .foregroundStyle(.white)
            }

            // Name and email
            VStack(alignment: .leading, spacing: 2) {
                Text(displayName.isEmpty ? email : displayName)
                    .font(.body)
                    .fontWeight(.semibold)
                    .foregroundStyle(VisioColors.onBackground(dark: isDark))
                    .lineLimit(1)
                if !email.isEmpty && !displayName.isEmpty {
                    Text(email)
                        .font(.caption)
                        .foregroundStyle(VisioColors.secondaryText(dark: isDark))
                        .lineLimit(1)
                }
            }

            Spacer()

            // Logout button
            Button(action: onLogout) {
                Image(systemName: "rectangle.portrait.and.arrow.right")
                    .foregroundStyle(VisioColors.secondaryText(dark: isDark))
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(isDark
                    ? Color(red: 0.12, green: 0.12, blue: 0.18)
                    : Color(red: 0.95, green: 0.95, blue: 0.97))
        )
    }
}

// MARK: - Create Room Sheet

private struct CreateRoomSheet: View {
    @EnvironmentObject private var manager: VisioManager
    let lang: String
    let onCreated: (String) -> Void
    let onCancel: () -> Void

    @State private var roomDisplayName: String = ""
    @State private var accessLevel: String = "public"
    @State private var creating: Bool = false
    @State private var error: String? = nil
    @State private var createdUrl: String? = nil
    @State private var copiedHttp: Bool = false
    @State private var copiedDeep: Bool = false
    @State private var searchQuery: String = ""
    @State private var searchResults: [UserSearchResult] = []
    @State private var invitedUsers: [UserSearchResult] = []
    @State private var createdRoomId: String? = nil
    @State private var searchTask: Task<Void, Never>? = nil
    @State private var pendingAliasConflictName: String? = nil
    @State private var pendingAliasConflictUrl: String? = nil
    @State private var showAliasConflict: Bool = false

    private var deepLink: String {
        guard let url = createdUrl else { return "" }
        let stripped = url.replacingOccurrences(of: "https://", with: "")
        return "visio://\(stripped)"
    }

    var body: some View {
        NavigationStack {
            Form {
                if createdUrl == nil {
                    createFormContent
                } else {
                    resultFormContent
                }
            }
            .navigationTitle(Strings.t("home.createRoom", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(Strings.t("settings.cancel", lang: lang)) { onCancel() }
                }
            }
            .alert(
                Strings.t("alias.conflictTitle", lang: lang)
                    .replacingOccurrences(of: "{name}", with: pendingAliasConflictName ?? ""),
                isPresented: $showAliasConflict
            ) {
                Button(Strings.t("alias.conflictReplace", lang: lang)) {
                    if let name = pendingAliasConflictName, let url = pendingAliasConflictUrl {
                        manager.client.addVisioAlias(name: name, url: url)
                    }
                    pendingAliasConflictName = nil
                    pendingAliasConflictUrl = nil
                }
                Button(Strings.t("alias.conflictCancel", lang: lang), role: .cancel) {
                    pendingAliasConflictName = nil
                    pendingAliasConflictUrl = nil
                }
            }
        }
    }

    // MARK: - Create form (before room is created)

    @ViewBuilder
    private var createFormContent: some View {
                    Section {
                        TextField(Strings.t("home.roomDisplayName", lang: lang), text: $roomDisplayName)
                    }

                    Section {
                        Picker(Strings.t("home.createRoom.access", lang: lang), selection: $accessLevel) {
                            Text(Strings.t("home.createRoom.public", lang: lang)).tag("public")
                            Text(Strings.t("home.createRoom.trusted", lang: lang)).tag("trusted")
                            Text(Strings.t("home.createRoom.restricted", lang: lang)).tag("restricted")
                        }
                        .pickerStyle(.inline)
                        .labelsHidden()

                        if accessLevel == "public" {
                            Text(Strings.t("home.createRoom.publicDesc", lang: lang))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        } else if accessLevel == "trusted" {
                            Text(Strings.t("home.createRoom.trustedDesc", lang: lang))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        } else {
                            Text(Strings.t("home.createRoom.restrictedDesc", lang: lang))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    } header: {
                        Text(Strings.t("home.createRoom.access", lang: lang))
                    }

                    if accessLevel == "restricted" {
                        Section(header: Text(Strings.t("restricted.invite", lang: lang))) {
                            TextField(Strings.t("restricted.searchUsers", lang: lang), text: $searchQuery)
                                .onChange(of: searchQuery) { _ in
                                    searchTask?.cancel()
                                    guard searchQuery.count >= 3 else {
                                        searchResults = []
                                        return
                                    }
                                    searchTask = Task {
                                        try? await Task.sleep(nanoseconds: 300_000_000)
                                        guard !Task.isCancelled else { return }
                                        let query = searchQuery
                                        DispatchQueue.global(qos: .userInitiated).async {
                                            do {
                                                let results = try manager.client.searchUsers(query: query)
                                                DispatchQueue.main.async {
                                                    searchResults = results.filter { user in
                                                        !invitedUsers.contains(where: { $0.id == user.id })
                                                    }
                                                }
                                            } catch {
                                                DispatchQueue.main.async { searchResults = [] }
                                            }
                                        }
                                    }
                                }

                            ForEach(searchResults, id: \.id) { user in
                                Button {
                                    invitedUsers.append(user)
                                    searchQuery = ""
                                    searchResults = []
                                } label: {
                                    VStack(alignment: .leading) {
                                        Text(user.fullName ?? user.email)
                                        Text(user.email)
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                }
                            }
                        }

                        if !invitedUsers.isEmpty {
                            Section(header: Text(Strings.t("restricted.members", lang: lang))) {
                                ForEach(invitedUsers, id: \.id) { user in
                                    HStack {
                                        Text(user.fullName ?? user.email)
                                        Spacer()
                                        Button {
                                            invitedUsers.removeAll { $0.id == user.id }
                                        } label: {
                                            Image(systemName: "xmark.circle.fill")
                                                .foregroundStyle(.secondary)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let error {
                        Section {
                            Text(error)
                                .foregroundStyle(.red)
                                .font(.caption)
                        }
                    }

                    Section {
                        Button {
                            let meetInstance = manager.authenticatedMeetInstance
                            guard !meetInstance.isEmpty else { return }
                            creating = true
                            error = nil
                            DispatchQueue.global(qos: .userInitiated).async {
                                do {
                                    let result = try manager.client.createRoom(
                                        meetUrl: "https://\(meetInstance)",
                                        accessLevel: accessLevel
                                    )
                                    // Add accesses for invited users
                                    if accessLevel == "restricted" {
                                        for user in invitedUsers {
                                            _ = try? manager.client.addAccess(userId: user.id, roomId: result.id)
                                        }
                                    }
                                    DispatchQueue.main.async {
                                        createdRoomId = result.id
                                        let trimmedName = roomDisplayName.trimmingCharacters(in: .whitespacesAndNewlines)
                                        let baseUrl = "https://\(meetInstance)/\(result.slug)"
                                        if !trimmedName.isEmpty {
                                            var allowed = CharacterSet.urlQueryAllowed
                                            allowed.remove(charactersIn: " +&=")
                                            let encoded = trimmedName.addingPercentEncoding(withAllowedCharacters: allowed) ?? trimmedName
                                            createdUrl = "\(baseUrl)?visio=\(encoded)"
                                            let conflict = manager.client.checkVisioAliasConflict(name: trimmedName, url: baseUrl)
                                            if conflict == nil {
                                                manager.client.addVisioAlias(name: trimmedName, url: baseUrl)
                                            } else {
                                                pendingAliasConflictName = trimmedName
                                                pendingAliasConflictUrl = baseUrl
                                                showAliasConflict = true
                                            }
                                        } else {
                                            createdUrl = baseUrl
                                        }
                                        creating = false
                                    }
                                } catch {
                                    DispatchQueue.main.async {
                                        self.error = error.localizedDescription
                                        creating = false
                                    }
                                }
                            }
                        } label: {
                            HStack {
                                Spacer()
                                Text(creating
                                    ? Strings.t("home.createRoom.creating", lang: lang)
                                    : Strings.t("home.createRoom.create", lang: lang))
                                    .fontWeight(.semibold)
                                Spacer()
                            }
                        }
                        .disabled(creating)
                    }
    }

    // MARK: - Result form (after room is created)

    @ViewBuilder
    private var resultFormContent: some View {
                    Section {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Image(systemName: "globe")
                                    .font(.caption)
                                Text(Strings.t("settings.incall.roomLink", lang: lang))
                                    .font(.subheadline)
                                    .fontWeight(.semibold)
                                Spacer()
                                Button {
                                    UIPasteboard.general.string = createdUrl
                                    copiedHttp = true
                                    DispatchQueue.main.asyncAfter(deadline: .now() + 2) { copiedHttp = false }
                                } label: {
                                    Image(systemName: copiedHttp ? "checkmark" : "doc.on.doc")
                                        .font(.caption)
                                }
                                ShareLink(item: createdUrl!) {
                                    Image(systemName: "square.and.arrow.up")
                                        .font(.caption)
                                }
                            }
                            TextField("", text: .constant(createdUrl!))
                                .font(.caption)
                                .textFieldStyle(.roundedBorder)
                                .disabled(true)
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Image(systemName: "iphone")
                                    .font(.caption)
                                Text(Strings.t("settings.incall.deepLink", lang: lang))
                                    .font(.subheadline)
                                    .fontWeight(.semibold)
                                Spacer()
                                Button {
                                    UIPasteboard.general.string = deepLink
                                    copiedDeep = true
                                    DispatchQueue.main.asyncAfter(deadline: .now() + 2) { copiedDeep = false }
                                } label: {
                                    Image(systemName: copiedDeep ? "checkmark" : "doc.on.doc")
                                        .font(.caption)
                                }
                                ShareLink(item: createdUrl!) {
                                    Image(systemName: "square.and.arrow.up")
                                        .font(.caption)
                                }
                            }
                            TextField("", text: .constant(deepLink))
                                .font(.caption)
                                .textFieldStyle(.roundedBorder)
                                .disabled(true)
                        }
                    } header: {
                        Text(Strings.t("settings.incall.roomInfo", lang: lang))
                    }

                    if !roomDisplayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        let host = (createdUrl ?? "").replacingOccurrences(of: "https://", with: "").components(separatedBy: "/").first ?? ""
                        let simplifiedUrl = "visio://\(host)/\(roomDisplayName.trimmingCharacters(in: .whitespacesAndNewlines))"
                        Section {
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Image(systemName: "link")
                                        .font(.caption)
                                    Text(Strings.t("home.createVisio.simplifiedUrl", lang: lang))
                                        .font(.subheadline)
                                        .fontWeight(.semibold)
                                    Spacer()
                                    Button {
                                        UIPasteboard.general.string = simplifiedUrl
                                    } label: {
                                        Image(systemName: "doc.on.doc")
                                            .font(.caption)
                                    }
                                }
                                TextField("", text: .constant(simplifiedUrl))
                                    .font(.caption)
                                    .textFieldStyle(.roundedBorder)
                                    .disabled(true)
                                Text(Strings.t("home.createVisio.simplifiedUrlHint", lang: lang))
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }

                    Section {
                        Button {
                            onCreated(createdUrl!)
                        } label: {
                            HStack {
                                Spacer()
                                Label(Strings.t("home.join", lang: lang), systemImage: "phone.fill")
                                    .fontWeight(.semibold)
                                Spacer()
                            }
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
