import SwiftUI
import WebKit
import visioFFI

struct HomeView: View {
    @EnvironmentObject private var manager: VisioManager

    @State private var roomURL: String = ""
    @State private var resolvedRoomURL: String = ""
    @State private var displayName: String = ""
    @State private var navigateToCall: Bool = false
    @State private var showSettings: Bool = false
    @State private var roomStatus: String = "idle"
    @State private var meetInstances: [String] = []
    @State private var showServerPicker: Bool = false
    @State private var customServer: String = ""
    @State private var showCreateRoom: Bool = false
    @State private var roomHistory: [String] = []
    @State private var historyJoinPending: Bool = false
    @State private var showCompactHeader: Bool = false

    private var lang: String { manager.currentLang }
    private var isDark: Bool { manager.currentTheme == "dark" }

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
                        Color.clear.onChange(of: geo.frame(in: .named("scroll")).minY) { value in
                            showCompactHeader = value < -20
                        }
                    })

                Text(Strings.t("home.subtitle", lang: lang))
                    .font(.subheadline)
                    .foregroundStyle(VisioColors.secondaryText(dark: isDark))

                // Authentication section
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
                        guard extractSlug(trimmed) != nil else {
                            roomStatus = "idle"
                            resolvedRoomURL = trimmed
                            return
                        }
                        urlsToTry = [trimmed]
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

                if manager.isAuthenticated {
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

                        ForEach(Array(roomHistory.enumerated()), id: \.offset) { index, url in
                            let slug = url.contains("/") ? String(url.split(separator: "/").last ?? "") : url
                            let host = URL(string: url)?.host ?? ""

                            Button {
                                roomURL = url
                                resolvedRoomURL = url
                                // If already validated, navigate immediately
                                if roomStatus == "valid" {
                                    navigateToCall = true
                                } else {
                                    historyJoinPending = true
                                }
                            } label: {
                                HStack(spacing: 10) {
                                    if historyJoinPending && roomURL == url {
                                        ProgressView()
                                            .scaleEffect(0.7)
                                            .frame(width: 14, height: 14)
                                    } else {
                                        Image(systemName: "globe")
                                            .font(.system(size: 14))
                                            .foregroundStyle(VisioColors.primary500)
                                    }

                                    VStack(alignment: .leading, spacing: 2) {
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
            CallView(
                roomURL: resolvedRoomURL,
                displayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        .sheet(isPresented: $showSettings) {
            SettingsView()
                .environmentObject(manager)
        }
        .onAppear {
            // Pre-fill display name from manager (includes OIDC identity)
            let name = manager.displayName
            if !name.isEmpty && displayName.isEmpty {
                displayName = name
            }
            // Load meet instances
            meetInstances = manager.client.getMeetInstances()
            // Load room history
            roomHistory = manager.client.getRoomHistory()
        }
        .onChange(of: manager.authenticatedDisplayName) { newValue in
            if !newValue.isEmpty && displayName.isEmpty {
                displayName = newValue
            }
        }
        .onChange(of: roomStatus) { newStatus in
            guard historyJoinPending else { return }
            if newStatus == "valid" {
                historyJoinPending = false
                navigateToCall = true
            } else if newStatus == "not_found" || newStatus == "idle" {
                // Validation failed — fall back to just showing the URL in the field
                historyJoinPending = false
            }
        }
        .onChange(of: manager.pendingDeepLink) { newValue in
            if let link = newValue {
                roomURL = link
                manager.pendingDeepLink = nil
            }
        }
        .onChange(of: manager.pendingTestConnect != nil) { hasTestConnect in
            if hasTestConnect {
                navigateToCall = true
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
                onComplete: { cookie, instance in
                    showServerPicker = false
                    if let cookie {
                        manager.onAuthCookieReceived(cookie, meetInstance: instance)
                    }
                },
                onDismiss: { showServerPicker = false }
            )
        }
        .sheet(isPresented: Binding(
            get: { manager.authManager.pendingInstance != nil },
            set: { if !$0 { manager.authManager.onWebViewCookie(nil, meetInstance: "") } }
        )) {
            if let instance = manager.authManager.pendingInstance {
                OidcFallbackWebView(meetInstance: instance) { cookie in
                    manager.authManager.onWebViewCookie(cookie, meetInstance: instance)
                    if let cookie {
                        manager.onAuthCookieReceived(cookie, meetInstance: instance)
                    }
                }
            }
        }
    }

    private func launchOidc(meetInstance: String) {
        manager.authManager.launchOidcFlow(meetInstance: meetInstance) { [weak manager] cookie in
            if let cookie, let manager {
                manager.onAuthCookieReceived(cookie, meetInstance: meetInstance)
            }
        }
    }
}

// MARK: - OIDC Fallback WebView

/// WKWebView-based OIDC fallback for servers that don't support custom scheme returnTo.
private struct OidcFallbackWebView: UIViewRepresentable {
    let meetInstance: String
    let onCookie: (String?) -> Void

    private static let cookieNames = ["meet_sessionid", "sessionid"]

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        let returnTo = "https://\(meetInstance)/"
        let encodedReturnTo = returnTo.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? returnTo
        if let url = URL(string: "https://\(meetInstance)/api/v1.0/authenticate/?returnTo=\(encodedReturnTo)") {
            webView.load(URLRequest(url: url))
        }
        return webView
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {}

    class Coordinator: NSObject, WKNavigationDelegate {
        let parent: OidcFallbackWebView
        init(_ parent: OidcFallbackWebView) { self.parent = parent }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            guard let url = webView.url else { return }
            let instance = parent.meetInstance
            // Detect when we've landed on the instance homepage after auth
            guard url.host == instance,
                  !url.path.contains("/authenticate"),
                  !url.path.contains("/oauth2/"),
                  !url.path.contains("/callback") else { return }

            webView.configuration.websiteDataStore.httpCookieStore.getAllCookies { [parent] cookies in
                if let cookie = cookies.first(where: {
                    OidcFallbackWebView.cookieNames.contains($0.name) && $0.domain.contains(instance)
                }) {
                    Task { @MainActor in parent.onCookie(cookie.value) }
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
    let onComplete: (String?, String) -> Void  // (cookie?, meetInstance)
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
        // Dismiss the server picker sheet, then launch ASWebAuthenticationSession
        onDismiss()
        manager.authManager.launchOidcFlow(meetInstance: instance) { cookie in
            onComplete(cookie, instance)
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

    private var deepLink: String {
        guard let url = createdUrl else { return "" }
        let stripped = url.replacingOccurrences(of: "https://", with: "")
        return "visio://\(stripped)"
    }

    var body: some View {
        NavigationStack {
            Form {
                if createdUrl == nil {
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
                                .onChange(of: searchQuery) { newValue in
                                    searchTask?.cancel()
                                    guard newValue.count >= 3 else {
                                        searchResults = []
                                        return
                                    }
                                    searchTask = Task {
                                        try? await Task.sleep(nanoseconds: 300_000_000)
                                        guard !Task.isCancelled else { return }
                                        let query = newValue
                                        let client = manager.client
                                        let currentInvited = invitedUsers
                                        do {
                                            let results = try await Task.detached {
                                                try client.searchUsers(query: query)
                                            }.value
                                            searchResults = results.filter { user in
                                                !currentInvited.contains(where: { $0.id == user.id })
                                            }
                                        } catch {
                                            searchResults = []
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
                            let client = manager.client
                            let level = accessLevel
                            let users = invitedUsers
                            Task {
                                do {
                                    let result = try await Task.detached {
                                        try client.createRoom(
                                            meetUrl: "https://\(meetInstance)",
                                            name: "",
                                            accessLevel: level
                                        )
                                    }.value
                                    // Add accesses for invited users
                                    if level == "restricted" {
                                        let roomId = result.id
                                        await Task.detached {
                                            for user in users {
                                                _ = try? client.addAccess(userId: user.id, roomId: roomId)
                                            }
                                        }.value
                                    }
                                    createdRoomId = result.id
                                    createdUrl = "https://\(meetInstance)/\(result.slug)"
                                    creating = false
                                } catch {
                                    self.error = error.localizedDescription
                                    creating = false
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
                } else {
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
                                    Task { try? await Task.sleep(for: .seconds(2)); copiedHttp = false }
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
                                    Task { try? await Task.sleep(for: .seconds(2)); copiedDeep = false }
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
            .navigationTitle(Strings.t("home.createRoom", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(Strings.t("settings.cancel", lang: lang)) { onCancel() }
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
