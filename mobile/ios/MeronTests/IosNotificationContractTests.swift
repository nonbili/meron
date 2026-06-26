@testable import Meron
import UserNotifications
import XCTest

final class IosNotificationContractTests: XCTestCase {
    func testRefreshNotificationContentUsesExpectedTitleBodyAndSound() {
        let content = IosNotificationService.refreshNotificationContent("2 account(s) refreshed")

        XCTAssertEqual(content.title, "Meron refresh complete")
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

    func testNotificationThreadIdUidKeyHasNoSubject() {
        let target = IosNotificationThreadTarget(
            accountId: "acct",
            folder: "inbox",
            threadKey: "uid:42",
            subject: "anything"
        )

        XCTAssertEqual(iosNotificationThreadId(target), "acct#INBOX#42")
    }
}
