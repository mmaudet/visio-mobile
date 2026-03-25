import Foundation
import Security

// MARK: - OidcAuthManager

class OidcAuthManager: ObservableObject {

    /// The meet instance being authenticated against.
    /// When set, the WKWebView auth sheet is presented.
    @Published var pendingInstance: String?

    /// Known session cookie names (Meet uses "meet_sessionid", others may use "sessionid").
    static let cookieNames = ["meet_sessionid", "sessionid"]

    // MARK: - OIDC Flow

    /// Presents the WKWebView auth sheet for the given meet instance.
    func launchOidcFlow(meetInstance: String) {
        pendingInstance = meetInstance
    }

    /// Called by the WKWebView when it extracts a cookie (or nil on dismiss).
    func onWebViewCookie(_ cookie: String?, meetInstance: String) {
        pendingInstance = nil
    }

    // MARK: - Code Exchange

    /// Exchanges a one-time OIDC code for a session cookie via the server's
    /// `/api/v1.0/auth/session-exchange/` endpoint.
    /// Used by the WKWebView coordinator when the server includes `?code=` in the redirect.
    static func exchangeCode(
        meetInstance: String,
        code: String,
        onCookie: @escaping (String) -> Void,
        onFailure: @escaping () -> Void
    ) {
        guard let url = URL(string: "https://\(meetInstance)/api/v1.0/auth/session-exchange/") else {
            DispatchQueue.main.async { onFailure() }
            return
        }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: ["code": code])

        URLSession.shared.dataTask(with: request) { data, _, _ in
            let cookie = data
                .flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: String] }
                .flatMap { dict in Self.cookieNames.compactMap { dict[$0] }.first }

            DispatchQueue.main.async {
                if let cookie {
                    onCookie(cookie)
                } else {
                    NSLog("[OidcAuthManager] session-exchange failed or missing key")
                    onFailure()
                }
            }
        }.resume()
    }

    // MARK: - Keychain Storage

    func saveCookie(_ cookie: String) {
        let data = cookie.data(using: .utf8)!
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "visio_sessionid",
            kSecAttrService as String: "io.visio.mobile",
        ]
        SecItemDelete(query as CFDictionary)
        var addQuery = query
        addQuery[kSecValueData as String] = data
        SecItemAdd(addQuery as CFDictionary, nil)
    }

    func getSavedCookie() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "visio_sessionid",
            kSecAttrService as String: "io.visio.mobile",
            kSecReturnData as String: true,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func clearCookie() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "visio_sessionid",
            kSecAttrService as String: "io.visio.mobile",
        ]
        SecItemDelete(query as CFDictionary)
    }
}
