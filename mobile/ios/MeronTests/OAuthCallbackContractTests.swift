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
