import AuthenticationServices
import Security
import UIKit

/// PKCE result delivered to the caller of `launchOidcFlow`.
///
/// The verifier was generated on this device and stored in the Keychain
/// before the SSO browser opened. It is the secret half of RFC 7636 §4.1:
/// callers MUST submit it to `VisioClient.exchangePkceCode(...)` to redeem
/// the authorization code for JWTs. State is already verified at the time
/// this struct is constructed.
struct PkceCallback {
    let code: String
    let verifier: String
}

class OidcAuthManager: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {

    @Published var pendingInstance: String?
    private var authSession: ASWebAuthenticationSession?

    /// Launch the OAuth2 + PKCE flow via ASWebAuthenticationSession.
    ///
    /// Generates a fresh code_verifier / code_challenge / state via the Rust
    /// core (RFC 7636 §4.1-4.2), stores verifier+state in the Keychain, and
    /// opens the SSO URL with `code_challenge`, `code_challenge_method=S256`,
    /// `state`, and `returnTo=/mobile-login`. The Meet server completes the
    /// IdP login, then redirects to `visio://auth-callback?code=…&state=…`.
    /// On callback we verify `state` (constant-time) before invoking the
    /// completion with the (code, verifier) tuple. The verifier is cleared
    /// from the Keychain regardless of outcome so a stale verifier cannot
    /// be replayed.
    func launchOidcFlow(meetInstance: String, completion: @escaping (PkceCallback?) -> Void) {
        pendingInstance = meetInstance

        let pkce = pkceGenerate()
        saveVerifier(pkce.verifier)
        saveState(pkce.state)

        guard let challengeEncoded = pkce.challenge.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed),
              let stateEncoded = pkce.state.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed),
              let returnToEncoded = "/mobile-login".addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed),
              let authURL = URL(string:
                "https://\(meetInstance)/api/v1.0/authenticate/?response_type=code"
                + "&code_challenge=\(challengeEncoded)&code_challenge_method=S256"
                + "&state=\(stateEncoded)&returnTo=\(returnToEncoded)&prompt=login")
        else {
            clearVerifier()
            clearState()
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
                if nsError.domain != ASWebAuthenticationSessionError.errorDomain
                    || nsError.code != ASWebAuthenticationSessionError.canceledLogin.rawValue {
                    NSLog("[OidcAuthManager] ASWebAuth error: \(error.localizedDescription)")
                }
                self.clearVerifier()
                self.clearState()
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            // Extract code+state from: visio://auth-callback?code=…&state=…
            let components = callbackURL.flatMap { URLComponents(url: $0, resolvingAgainstBaseURL: false) }
            let returnedCode = components?.queryItems?.first(where: { $0.name == "code" })?.value
            let returnedState = components?.queryItems?.first(where: { $0.name == "state" })?.value
            let savedVerifier = self.getSavedVerifier()
            let savedState = self.getSavedState()

            // Single-use: clear PKCE material before any branch below so a
            // replay with the same verifier cannot succeed.
            self.clearVerifier()
            self.clearState()

            guard let code = returnedCode, !code.isEmpty,
                  let state = returnedState, !state.isEmpty,
                  let verifier = savedVerifier, !verifier.isEmpty,
                  let expectedState = savedState, !expectedState.isEmpty
            else {
                NSLog("[OidcAuthManager] Missing code/state/verifier on PKCE callback")
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            guard Self.constantTimeEqual(state, expectedState) else {
                NSLog("[OidcAuthManager] PKCE state mismatch — possible CSRF attempt")
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            DispatchQueue.main.async {
                self.pendingInstance = nil
                completion(PkceCallback(code: code, verifier: verifier))
            }
        }

        session.prefersEphemeralWebBrowserSession = false
        session.presentationContextProvider = self
        authSession = session
        session.start()
    }

    /// Length-and-content constant-time comparison for the PKCE state nonce.
    /// Single-use values make timing oracles uninteresting in practice, but
    /// the constant-time form avoids any ambiguity if state is ever reused.
    private static func constantTimeEqual(_ a: String, _ b: String) -> Bool {
        let lhs = Array(a.utf8)
        let rhs = Array(b.utf8)
        if lhs.count != rhs.count { return false }
        var diff: UInt8 = 0
        for i in 0..<lhs.count {
            diff |= lhs[i] ^ rhs[i]
        }
        return diff == 0
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

    private static let service = "io.visio.mobile"
    private static let accessAccount = "visio_access_token"
    private static let refreshAccount = "visio_refresh_token"
    private static let instanceAccount = "visio_meet_instance"
    private static let verifierAccount = "visio_pkce_verifier"
    private static let stateAccount = "visio_pkce_state"

    private func keychainQuery(account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: account,
            kSecAttrService as String: Self.service,
        ]
    }

    private func storeKeychain(_ value: String, account: String) {
        guard let data = value.data(using: .utf8) else { return }
        let query = keychainQuery(account: account)
        SecItemDelete(query as CFDictionary)
        var addQuery = query
        addQuery[kSecValueData as String] = data
        SecItemAdd(addQuery as CFDictionary, nil)
    }

    private func readKeychain(account: String) -> String? {
        var query = keychainQuery(account: account)
        query[kSecReturnData as String] = true
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private func deleteKeychain(account: String) {
        SecItemDelete(keychainQuery(account: account) as CFDictionary)
    }

    /// Persist tokens together with the meet instance they were minted for.
    /// On restore, callers MUST use `getSavedMeetInstance()` (not the user's
    /// first instance in settings, which could be an attacker entry).
    func saveTokens(access: String, refresh: String, meetInstance: String) {
        storeKeychain(access, account: Self.accessAccount)
        storeKeychain(refresh, account: Self.refreshAccount)
        storeKeychain(meetInstance, account: Self.instanceAccount)
    }

    func getSavedAccessToken() -> String? { readKeychain(account: Self.accessAccount) }
    func getSavedRefreshToken() -> String? { readKeychain(account: Self.refreshAccount) }
    func getSavedMeetInstance() -> String? { readKeychain(account: Self.instanceAccount) }

    func clearTokens() {
        deleteKeychain(account: Self.accessAccount)
        deleteKeychain(account: Self.refreshAccount)
        deleteKeychain(account: Self.instanceAccount)
    }

    private func saveVerifier(_ verifier: String) { storeKeychain(verifier, account: Self.verifierAccount) }
    private func getSavedVerifier() -> String? { readKeychain(account: Self.verifierAccount) }
    private func clearVerifier() { deleteKeychain(account: Self.verifierAccount) }

    private func saveState(_ state: String) { storeKeychain(state, account: Self.stateAccount) }
    private func getSavedState() -> String? { readKeychain(account: Self.stateAccount) }
    private func clearState() { deleteKeychain(account: Self.stateAccount) }
}
