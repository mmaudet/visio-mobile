import AuthenticationServices
import Security
import SwiftUI
import UIKit
import WebKit

// MARK: - OidcAuthManager

@MainActor
class OidcAuthManager: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {

    /// Called when the OIDC flow completes (cookie or nil).
    var onComplete: ((String?) -> Void)?

    /// Tracks the active session so it isn't deallocated mid-flow.
    private var authSession: ASWebAuthenticationSession?

    /// The meet instance being authenticated against.
    @Published var pendingInstance: String?

    /// Known session cookie names (Meet uses "meet_sessionid", others may use "sessionid").
    private static let cookieNames = ["meet_sessionid", "sessionid"]

    // MARK: - OIDC Flow

    /// Launches the OIDC authentication flow.
    ///
    /// Tries ASWebAuthenticationSession with visio:// callback first (requires server
    /// support for custom scheme returnTo). If the server rejects the custom scheme
    /// (returns to homepage instead), falls back to WKWebView-based extraction.
    func launchOidcFlow(meetInstance: String, completion: @escaping (String?) -> Void) {
        onComplete = completion
        pendingInstance = meetInstance

        // Try ASWebAuthenticationSession first — best UX (password manager, saved sessions)
        let returnTo = "visio://auth-callback"
        let encodedReturnTo = returnTo.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? returnTo
        guard let authURL = URL(string: "https://\(meetInstance)/api/v1.0/authenticate/?returnTo=\(encodedReturnTo)") else {
            completion(nil)
            return
        }

        let session = ASWebAuthenticationSession(
            url: authURL,
            callbackURLScheme: "visio"
        ) { [weak self] callbackURL, error in
            guard let self else { return }
            self.authSession = nil

            if let error {
                let nsError = error as NSError
                // ASWebAuthenticationSessionErrorCodeCanceledLogin = 1
                if nsError.domain == ASWebAuthenticationSessionError.errorDomain
                    && nsError.code == ASWebAuthenticationSessionError.canceledLogin.rawValue {
                    // User cancelled — don't fallback, just report nil
                    self.onComplete?(nil)
                    self.onComplete = nil
                    self.pendingInstance = nil
                    return
                }
                // Other error — try fallback
                print("[OidcAuthManager] ASWebAuth failed: \(error.localizedDescription), falling back to webview")
                self.pendingInstance = meetInstance
                return
            }

            // Success — try to extract cookie from shared storage
            self.extractSessionCookie(meetInstance: meetInstance) { cookie in
                if let cookie {
                    self.onComplete?(cookie)
                    self.onComplete = nil
                    self.pendingInstance = nil
                } else {
                    // Cookie not in shared storage — server may not support custom scheme returnTo
                    // Fall back to webview
                    print("[OidcAuthManager] No cookie after ASWebAuth, falling back to webview")
                    self.pendingInstance = meetInstance
                }
            }
        }

        session.prefersEphemeralWebBrowserSession = false
        session.presentationContextProvider = self
        authSession = session
        session.start()
    }

    /// Called by the webview fallback when it extracts a cookie.
    func onWebViewCookie(_ cookie: String?, meetInstance: String) {
        pendingInstance = nil
        onComplete?(cookie)
        onComplete = nil
    }

    // MARK: - Cookie Extraction

    private func extractSessionCookie(meetInstance: String, completion: @escaping (String?) -> Void) {
        guard let instanceURL = URL(string: "https://\(meetInstance)/") else {
            completion(nil)
            return
        }

        let cookies = HTTPCookieStorage.shared.cookies(for: instanceURL) ?? []
        if let sessionCookie = cookies.first(where: { Self.cookieNames.contains($0.name) })?.value,
           !sessionCookie.isEmpty {
            completion(sessionCookie)
        } else {
            // Retry once after a short delay
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(500))
                let retryCookies = HTTPCookieStorage.shared.cookies(for: instanceURL) ?? []
                let cookie = retryCookies.first(where: { Self.cookieNames.contains($0.name) })?.value
                completion(cookie?.isEmpty == false ? cookie : nil)
            }
        }
    }

    // MARK: - ASWebAuthenticationPresentationContextProviding

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated {
            guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                  let window = scene.windows.first(where: { $0.isKeyWindow }) ?? scene.windows.first else {
                return ASPresentationAnchor()
            }
            return window
        }
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
