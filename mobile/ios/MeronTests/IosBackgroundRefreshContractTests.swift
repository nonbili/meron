@testable import Meron
import XCTest

final class IosBackgroundRefreshContractTests: XCTestCase {
    func testMailAccountBuildsMailSyncRequestForInboxRefresh() throws {
        let request = try XCTUnwrap(
            iosRefreshSyncRequest(
                accountId: "mail-account",
                engine: "mail",
                provider: "gmail",
                authType: "gmail_oauth",
                paused: false,
                needsReconnect: false,
                id: 7
            )
        )

        XCTAssertEqual(request.id, 7)
        XCTAssertEqual(request.method, "mail.sync")
        XCTAssertEqual(request.params["account_id"] as? String, "mail-account")
        XCTAssertEqual(request.params["folder_id"] as? String, "inbox")
        XCTAssertEqual(request.params["limit"] as? Int, 50)
        XCTAssertEqual(request.params["folders"] as? Bool, true)
    }

    func testRssAccountBuildsRssSyncRequest() throws {
        let request = try XCTUnwrap(
            iosRefreshSyncRequest(
                accountId: "rss-account",
                engine: "rss",
                provider: "custom",
                authType: "password",
                paused: false,
                needsReconnect: false,
                id: 8
            )
        )

        XCTAssertEqual(request.id, 8)
        XCTAssertEqual(request.method, "rss.sync")
        XCTAssertEqual(request.params["account_id"] as? String, "rss-account")
        XCTAssertNil(request.params["folder_id"])
    }

    func testPausedOrDisconnectedAccountsAreSkipped() {
        XCTAssertNil(
            iosRefreshSyncRequest(
                accountId: "mail-account",
                engine: "mail",
                provider: "gmail",
                authType: "gmail_oauth",
                paused: true,
                needsReconnect: false,
                id: 9
            )
        )
        XCTAssertNil(
            iosRefreshSyncRequest(
                accountId: "mail-account",
                engine: "mail",
                provider: "gmail",
                authType: "gmail_oauth",
                paused: false,
                needsReconnect: true,
                id: 10
            )
        )
        XCTAssertNil(
            iosRefreshSyncRequest(
                accountId: "",
                engine: "mail",
                provider: "gmail",
                authType: "gmail_oauth",
                paused: false,
                needsReconnect: false,
                id: 11
            )
        )
    }
}
