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

    func testFeedArrivalsBecomeANotifiableBatch() throws {
        // What `rss.sync` returns under `new_messages`: the feed is the sender
        // and its subscription is the thread key a tap opens.
        let batch = try XCTUnwrap(
            iosNewMailBatch([
                "account": "rss-8f2a",
                "accountName": "My Feeds",
                "folder": "inbox",
                "count": 1,
                "muted": false,
                "from": "JavaScript Weekly",
                "subject": "Bun 1.4 is finally fresh",
                "preview": "This week in JavaScript",
                "threadKey": "sub-1",
            ])
        )

        XCTAssertEqual(
            batch,
            IosNewMailBatch(
                accountId: "rss-8f2a",
                accountName: "My Feeds",
                folder: "inbox",
                from: "JavaScript Weekly",
                subject: "Bun 1.4 is finally fresh",
                count: 1,
                threadKey: "sub-1"
            )
        )
    }

    func testMutedAccountsSyncButStaySilent() {
        XCTAssertNil(
            iosNewMailBatch([
                "account": "rss-8f2a",
                "accountName": "My Feeds",
                "folder": "inbox",
                "count": 3,
                "muted": true,
                "from": "JavaScript Weekly",
                "subject": "Bun 1.4 is finally fresh",
                "threadKey": "sub-1",
            ])
        )
    }

    func testPayloadNamingNoArrivalsPostsNothing() {
        XCTAssertNil(iosNewMailBatch(["account": "rss-8f2a", "count": 0]))
        XCTAssertNil(iosNewMailBatch(["account": "rss-8f2a"]))
    }
}
