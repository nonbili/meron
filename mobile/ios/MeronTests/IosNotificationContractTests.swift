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

    func testRemindersRequireNotificationAuthorization() {
        XCTAssertFalse(IosReminders.allowsScheduling(.notDetermined))
        XCTAssertFalse(IosReminders.allowsScheduling(.denied))
        XCTAssertTrue(IosReminders.allowsScheduling(.authorized))
        XCTAssertTrue(IosReminders.allowsScheduling(.provisional))
    }

    func testReminderDoesNotReplayAtTheFractionalDayBoundary() {
        let at = 100_000.0
        let selection = IosReminders.selectRequests(
            reminders: [["key": "task:one", "at": at]],
            history: ["task:one": at],
            pending: [],
            now: at + 86_400.5
        )
        XCTAssertTrue(selection.requests.isEmpty)
        XCTAssertEqual(selection.history["task:one"], at)
    }

    func testDuePendingRemindersKeepTheirSlotsWithoutExceedingTheLimit() {
        let now = 100_000.0
        var reminders: [[String: Any]] = [["key": "task:due", "at": now - 1]]
        reminders += (1...64).map { index -> [String: Any] in
            ["key": "task:future-\(index)", "at": now + Double(index)]
        }
        let selection = IosReminders.selectRequests(
            reminders: reminders,
            history: ["task:due": now - 1],
            pending: ["meron-reminder:task:due"],
            now: now
        )
        XCTAssertEqual(selection.requests.count, 63)
        XCTAssertEqual(selection.wanted.count, 64)
        XCTAssertTrue(selection.wanted.contains("meron-reminder:task:due"))
    }


    func testCanceledRequestsDoNotTurnIntoDeliveryReceipts() {
        let now = 100_000.0
        let selection = IosReminders.selectRequests(
            reminders: [["key": "event:new", "at": now + 600]],
            history: ["event:canceled-future": now + 60, "event:canceled-due": now - 1],
            pending: ["meron-reminder:event:canceled-future", "meron-reminder:event:canceled-due"],
            now: now
        )
        XCTAssertTrue(selection.history.isEmpty)
        XCTAssertEqual(selection.requests.count, 1)
        XCTAssertEqual(selection.wanted, ["meron-reminder:event:new"])
    }

    func testNewMailNotificationRemembersTheMessagesItAnnounced() throws {
        let content = IosNotificationService.notifyNewMailContentForTesting(
            accountName: "Work",
            from: "Ann",
            subject: "Lunch?",
            count: 2,
            accountId: "acct",
            folder: "INBOX",
            threadKey: "<abc@example.com>",
            storeMessages: [IosStoreMessage(folder: "INBOX", messageId: "41"), IosStoreMessage(folder: "INBOX", messageId: "42")]
        )

        let tracked = try XCTUnwrap(iosTrackedNotification(identifier: "meron-new-mail-1", userInfo: content.userInfo))
        XCTAssertEqual(tracked.accountId, "acct")
        XCTAssertEqual(tracked.messages.map(\.messageId), ["41", "42"])
        // Reminders and refresh summaries are never withdrawn as read mail.
        XCTAssertNil(iosTrackedNotification(identifier: "meron-reminder:event:1", userInfo: content.userInfo))
    }

    func testBatchNotificationIsWithdrawnOnlyOnceAllItsMailIsRead() {
        let first = IosStoreMessage(folder: "INBOX", messageId: "41")
        let second = IosStoreMessage(folder: "INBOX", messageId: "42")
        let batch = IosTrackedNotification(identifier: "meron-new-mail-1", accountId: "acct", messages: [first, second])
        let single = IosTrackedNotification(identifier: "meron-new-mail-2", accountId: "acct", messages: [first])

        XCTAssertEqual(
            iosStaleNotificationIds([batch, single], stillUnread: [IosStoreRow(accountId: "acct", message: second)]),
            ["meron-new-mail-2"]
        )
        // The same UID in another account is a different message.
        XCTAssertEqual(
            iosStaleNotificationIds([single], stillUnread: [IosStoreRow(accountId: "other", message: first)]),
            ["meron-new-mail-2"]
        )
    }
}
