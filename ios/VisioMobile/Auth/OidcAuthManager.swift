import AuthenticationServices
import Security
import UIKit

class OidcAuthManager: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {

    @Published var pendingInstance: String?
    private var authSession: ASWebAuthenticationSession?

    /// Launch OIDC flow via ASWebAuthenticationSession.
    /// The server redirects to visio://auth-callback?code={uuid}.
    /// Returns the exchange code via completion handler.
    func launchOidcFlow(meetInstance: String, completion: @escaping (String?) -> Void) {
        pendingInstance = meetInstance

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
                if nsError.domain == ASWebAuthenticationSessionError.errorDomain
                    && nsError.code == ASWebAuthenticationSessionError.canceledLogin.rawValue {
                    DispatchQueue.main.async {
                        self.pendingInstance = nil
                        completion(nil)
                    }
                    return
                }
                NSLog("[OidcAuthManager] ASWebAuth error: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            // Extract exchange code from: visio://auth-callback?code={uuid}
            guard let url = callbackURL,
                  let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
                  let code = components.queryItems?.first(where: { $0.name == "code" })?.value else {
                NSLog("[OidcAuthManager] No code parameter in callback URL: \(callbackURL?.absoluteString ?? "nil")")
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            DispatchQueue.main.async {
                self.pendingInstance = nil
                completion(code)
            }
        }

        session.prefersEphemeralWebBrowserSession = false
        session.presentationContextProvider = self
        authSession = session
        session.start()
    }

    // MARK: - ASWebAuthenticationPresentationContextProviding

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first(where: { $0.isKeyWindow }) ?? scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
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
