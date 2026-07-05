import CryptoKit
import Foundation
import MeronUI

func saveIosPendingOAuthFlow(_ flow: IosPendingOAuthFlow, defaults: UserDefaults = .standard) {
    guard let data = try? JSONEncoder().encode(flow) else { return }
    defaults.set(data, forKey: iosPendingOAuthFlowKey)
}

func loadIosPendingOAuthFlow(defaults: UserDefaults = .standard) -> IosPendingOAuthFlow? {
    guard let data = defaults.data(forKey: iosPendingOAuthFlowKey) else { return nil }
    return try? JSONDecoder().decode(IosPendingOAuthFlow.self, from: data)
}

func clearIosPendingOAuthFlow(defaults: UserDefaults = .standard) {
    defaults.removeObject(forKey: iosPendingOAuthFlowKey)
}

func iosOAuthInfoValue(_ key: String, infoDictionary: [String: Any]? = Bundle.main.infoDictionary) -> String {
    let value = (infoDictionary?[key] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    guard !value.isEmpty, !value.hasPrefix("$(") else { return "" }
    return value
}

func iosResolvedOAuthClientId(
    provider: String,
    configuredClientId: String,
    infoDictionary: [String: Any]? = Bundle.main.infoDictionary
) -> String {
    let provider = provider.lowercased()
    let configured = configuredClientId.trimmingCharacters(in: .whitespacesAndNewlines)
    if provider == "outlook" {
        if !configured.isEmpty, configured != iosDefaultGoogleOAuthClientId {
            return configured
        }
        return iosOAuthInfoValue("MERON_OUTLOOK_CLIENT_ID", infoDictionary: infoDictionary)
    }
    if !configured.isEmpty {
        return configured
    }
    return iosOAuthInfoValue("MERON_GOOGLE_CLIENT_ID", infoDictionary: infoDictionary).isEmpty
        ? iosDefaultGoogleOAuthClientId
        : iosOAuthInfoValue("MERON_GOOGLE_CLIENT_ID", infoDictionary: infoDictionary)
}

func iosResolvedOAuthClientSecret(
    provider: String,
    configuredClientSecret: String,
    infoDictionary: [String: Any]? = Bundle.main.infoDictionary
) -> String {
    let configured = configuredClientSecret.trimmingCharacters(in: .whitespacesAndNewlines)
    if !configured.isEmpty {
        return configured
    }
    return iosOAuthInfoValue(
        provider.lowercased() == "outlook" ? "MERON_OUTLOOK_CLIENT_SECRET" : "MERON_GOOGLE_CLIENT_SECRET",
        infoDictionary: infoDictionary
    )
}

func iosResolvedOAuthRedirectUri(
    provider: String,
    configuredRedirectUri: String,
    infoDictionary: [String: Any]? = Bundle.main.infoDictionary
) -> String {
    let configured = configuredRedirectUri.trimmingCharacters(in: .whitespacesAndNewlines)
    let key = provider.lowercased() == "outlook" ? "MERON_OUTLOOK_REDIRECT_URI" : "MERON_GOOGLE_REDIRECT_URI"
    let bundled = iosOAuthInfoValue(key, infoDictionary: infoDictionary)
    if !bundled.isEmpty {
        return bundled
    }
    return configured.isEmpty ? OAuthFlowKt.defaultOAuthRedirectUri() : configured
}

func pkceChallenge(_ verifier: String) -> String {
    let digest = SHA256.hash(data: Data(verifier.utf8))
    return Data(digest)
        .base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}
