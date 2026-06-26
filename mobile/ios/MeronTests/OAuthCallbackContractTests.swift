import Foundation
@testable import Meron
import MeronShared
import XCTest

final class OAuthCallbackContractTests: XCTestCase {
    func testOAuthCallbackStoresAuthorizationCode() {
        var code = ""

        let status = applyOAuthCallbackToComposeState(
            rawUrl: "jp.nonbili.meron.oauth://oauth?code=mobile-code&state=state-123",
            expectedState: "state-123",
            authorizationCode: &code
        )

        XCTAssertEqual(code, "mobile-code")
        XCTAssertEqual(status, "OAuth authorization code received; use Exchange Code And Add Account.")
    }

    func testOAuthCallbackRejectsWrongStateWithoutReplacingCode() {
        var code = "existing-code"

        let status = applyOAuthCallbackToComposeState(
            rawUrl: "jp.nonbili.meron.oauth://oauth?code=mobile-code&state=wrong",
            expectedState: "state-123",
            authorizationCode: &code
        )

        XCTAssertEqual(code, "existing-code")
        XCTAssertEqual(status, "OAuth callback failed: OAuth state mismatch")
    }

    func testOAuthCallbackAcceptsConfiguredHttpsRedirectUri() {
        var code = ""

        let status = applyOAuthCallbackToComposeState(
            rawUrl: "https://mail.example.com/oauth/ios?code=app-link-code&state=state-123",
            expectedState: "state-123",
            redirectUri: "https://mail.example.com/oauth/ios",
            authorizationCode: &code
        )

        XCTAssertEqual(code, "app-link-code")
        XCTAssertEqual(status, "OAuth authorization code received; use Exchange Code And Add Account.")
    }

    func testPendingOAuthFlowPersistsCallbackStateForResume() throws {
        let suiteName = "OAuthCallbackContractTests.pendingFlow"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        defer {
            defaults.removePersistentDomain(forName: suiteName)
        }

        let flow = IosPendingOAuthFlow(
            provider: "outlook",
            state: "state-123",
            verifier: "verifier-abc",
            redirectUri: "https://mail.example.com/oauth/ios",
            email: "ada@example.com"
        )

        saveIosPendingOAuthFlow(flow, defaults: defaults)

        XCTAssertEqual(loadIosPendingOAuthFlow(defaults: defaults), flow)
        XCTAssertTrue(
            OAuthFlowKt.isOAuthCallbackUrl(
                rawUrl: "https://mail.example.com/oauth/ios?code=mobile-code&state=state-123",
                redirectUri: loadIosPendingOAuthFlow(defaults: defaults)?.redirectUri ?? ""
            )
        )

        clearIosPendingOAuthFlow(defaults: defaults)

        XCTAssertNil(loadIosPendingOAuthFlow(defaults: defaults))
    }

    func testInfoPlistRegistersSharedOAuthRedirectUriScheme() throws {
        let redirectUri = try XCTUnwrap(URL(string: OAuthFlowKt.defaultOAuthRedirectUri()))
        let urlTypes = try XCTUnwrap(Bundle.main.object(forInfoDictionaryKey: "CFBundleURLTypes") as? [[String: Any]])
        let schemes = urlTypes
            .compactMap { $0["CFBundleURLSchemes"] as? [String] }
            .flatMap { $0 }

        XCTAssertEqual(redirectUri.scheme, "jp.nonbili.meron.oauth")
        XCTAssertEqual(redirectUri.host, "oauth")
        XCTAssertTrue(schemes.contains("jp.nonbili.meron.oauth"))
    }

    func testSourceInfoPlistExposesProviderOAuthBuildSettings() throws {
        let testFile = URL(fileURLWithPath: #filePath)
        let infoPlist = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Meron")
            .appendingPathComponent("Info.plist")
        let data = try Data(contentsOf: infoPlist)
        let plist = try XCTUnwrap(
            PropertyListSerialization.propertyList(from: data, options: [], format: nil) as? [String: Any]
        )

        XCTAssertEqual(plist["MERON_GOOGLE_CLIENT_ID"] as? String, "$(MERON_GOOGLE_CLIENT_ID)")
        XCTAssertEqual(plist["MERON_GOOGLE_CLIENT_SECRET"] as? String, "$(MERON_GOOGLE_CLIENT_SECRET)")
        XCTAssertEqual(plist["MERON_GOOGLE_REDIRECT_URI"] as? String, "$(MERON_GOOGLE_REDIRECT_URI)")
        XCTAssertEqual(plist["MERON_OUTLOOK_CLIENT_ID"] as? String, "$(MERON_OUTLOOK_CLIENT_ID)")
        XCTAssertEqual(plist["MERON_OUTLOOK_CLIENT_SECRET"] as? String, "$(MERON_OUTLOOK_CLIENT_SECRET)")
        XCTAssertEqual(plist["MERON_OUTLOOK_REDIRECT_URI"] as? String, "$(MERON_OUTLOOK_REDIRECT_URI)")
    }

    func testOAuthClientResolverDoesNotReuseGmailFallbackForOutlook() {
        XCTAssertEqual(
            iosResolvedOAuthClientId(
                provider: "outlook",
                configuredClientId: iosDefaultGoogleOAuthClientId,
                infoDictionary: [:]
            ),
            ""
        )
        XCTAssertEqual(
            iosResolvedOAuthClientId(
                provider: "gmail",
                configuredClientId: "",
                infoDictionary: [:]
            ),
            iosDefaultGoogleOAuthClientId
        )
    }

    func testOAuthResolverUsesProviderSpecificInfoPlistValues() {
        let info: [String: Any] = [
            "MERON_OUTLOOK_CLIENT_ID": "outlook-client",
            "MERON_OUTLOOK_CLIENT_SECRET": "outlook-secret",
            "MERON_OUTLOOK_REDIRECT_URI": "https://mail.example.com/oauth/outlook",
            "MERON_GOOGLE_CLIENT_ID": "$(MERON_GOOGLE_CLIENT_ID)",
            "MERON_GOOGLE_REDIRECT_URI": "https://mail.example.com/oauth/google",
        ]

        XCTAssertEqual(
            iosResolvedOAuthClientId(provider: "outlook", configuredClientId: iosDefaultGoogleOAuthClientId, infoDictionary: info),
            "outlook-client"
        )
        XCTAssertEqual(
            iosResolvedOAuthClientSecret(provider: "outlook", configuredClientSecret: "", infoDictionary: info),
            "outlook-secret"
        )
        XCTAssertEqual(
            iosResolvedOAuthRedirectUri(provider: "outlook", configuredRedirectUri: OAuthFlowKt.defaultOAuthRedirectUri(), infoDictionary: info),
            "https://mail.example.com/oauth/outlook"
        )
        XCTAssertEqual(
            iosResolvedOAuthClientId(provider: "gmail", configuredClientId: "", infoDictionary: info),
            iosDefaultGoogleOAuthClientId
        )
        XCTAssertEqual(
            iosResolvedOAuthRedirectUri(provider: "gmail", configuredRedirectUri: OAuthFlowKt.defaultOAuthRedirectUri(), infoDictionary: info),
            "https://mail.example.com/oauth/google"
        )
    }

    func testEntitlementsDeclarePlaceholderAssociatedDomain() throws {
        let testFile = URL(fileURLWithPath: #filePath)
        let entitlements = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Meron")
            .appendingPathComponent("Meron.entitlements")
        let data = try Data(contentsOf: entitlements)
        let plist = try XCTUnwrap(
            PropertyListSerialization.propertyList(from: data, options: [], format: nil) as? [String: Any]
        )
        let domains = try XCTUnwrap(plist["com.apple.developer.associated-domains"] as? [String])

        XCTAssertTrue(domains.contains("applinks:$(MERON_ASSOCIATED_DOMAIN)"))
    }
}
