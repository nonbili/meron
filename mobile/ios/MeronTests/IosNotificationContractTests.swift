@testable import Meron
import UserNotifications
import XCTest

final class IosNotificationContractTests: XCTestCase {
    func testRefreshNotificationContentUsesExpectedTitleBodyAndSound() {
        let content = IosNotificationService.refreshNotificationContent("2 account(s) refreshed")

        XCTAssertEqual(content.title, "Refresh complete")
        XCTAssertEqual(content.body, "2 account(s) refreshed")
        XCTAssertNotNil(content.sound)
    }

    func testNewMailNotificationCarriesOpenThreadPayload() throws {
        let content = IosNotificationService.notifyNewMailContentForTesting(
            accountName: "Work",
            from: "Ann",
            subject: "[github] build failed",
            count: 1,
            accountId: "acct",
            folder: "inbox",
            threadKey: "<abc@example.com>"
        )

        XCTAssertEqual(content.userInfo[IosNotificationPayloadKey.accountId] as? String, "acct")
        XCTAssertEqual(content.userInfo[IosNotificationPayloadKey.folder] as? String, "inbox")
        XCTAssertEqual(content.userInfo[IosNotificationPayloadKey.threadKey] as? String, "<abc@example.com>")
        XCTAssertEqual(content.userInfo[IosNotificationPayloadKey.subject] as? String, "[github] build failed")
        let target = try XCTUnwrap(iosNotificationThreadTarget(userInfo: content.userInfo))
        XCTAssertEqual(target, IosNotificationThreadTarget(accountId: "acct", folder: "inbox", threadKey: "<abc@example.com>", subject: "[github] build failed"))
    }

    func testTappedNotificationSurvivesBeingRepublishedToTheApp() throws {
        // The tap is handed to SwiftUI through NotificationCenter, which carries
        // it in userInfo — an object payload is dropped on the floor by the
        // subscriber, and the thread never opens.
        let target = IosNotificationThreadTarget(
            accountId: "rss-8f2a",
            folder: "inbox",
            threadKey: "sub-1",
            subject: "Bun 1.4 is finally fresh"
        )

        let republished = try XCTUnwrap(
            iosNotificationThreadTarget(userInfo: iosNotificationThreadTargetUserInfo(target))
        )
        XCTAssertEqual(republished, target)
    }

    func testNotificationThreadIdUsesGroupingSubjectAndInboxCasing() {
        let target = IosNotificationThreadTarget(
            accountId: "acct",
            folder: "inbox",
            threadKey: "<abc@example.com>",
            subject: "[github] build failed"
        )

        let expectedCompound = "<abc@example.com>#build failed"
        let expectedEncoded = Data(expectedCompound.utf8)
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        XCTAssertEqual(iosNotificationThreadId(target), "acct#INBOX#t.\(expectedEncoded)")
    }

    func testNotificationThreadIdFeedKeysNameTheirSubscription() {
        // A feed thread id carries no folder and no encoded key: it is the id
        // `rss.recent` puts on the feed row, which is what a tap has to match.
        let target = IosNotificationThreadTarget(
            accountId: "rss-8f2a",
            folder: "inbox",
            threadKey: "sub-1",
            subject: "Bun 1.4 is finally fresh"
        )

        XCTAssertEqual(iosNotificationThreadId(target), "rss-8f2a#rss#sub-1")
    }

    func testNotificationThreadIdUidKeyHasNoSubject() {
        let target = IosNotificationThreadTarget(
            accountId: "acct",
            folder: "inbox",
            threadKey: "uid:42",
            subject: "anything"
        )

        XCTAssertEqual(iosNotificationThreadId(target), "acct#INBOX#42")
    }

    func testNotificationThreadIdGmailThreadIdHasNoSubject() {
        let target = IosNotificationThreadTarget(
            accountId: "acct",
            folder: "inbox",
            threadKey: "gmthrid:123",
            subject: "[nonbili/Nora] Profiles bug [Linux Flatpak] (Issue #295)"
        )
        let expectedEncoded = Data("gmthrid:123".utf8)
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")

        XCTAssertEqual(iosNotificationThreadId(target), "acct#INBOX#t.\(expectedEncoded)")
    }
}
