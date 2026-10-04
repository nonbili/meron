@testable import Meron
import XCTest

final class IosUnreadWidgetTests: XCTestCase {
    func testReadOnlyAccountCommandsDoNotRefreshWidgets() {
        XCTAssertFalse(IosUnreadWidgets.commandChangesSnapshot("account.autodiscover"))
        XCTAssertFalse(IosUnreadWidgets.commandChangesSnapshot("account.probeCert"))
        XCTAssertFalse(IosUnreadWidgets.commandChangesSnapshot("account.list"))
        XCTAssertTrue(IosUnreadWidgets.commandChangesSnapshot("account.remove"))
        XCTAssertTrue(IosUnreadWidgets.commandChangesSnapshot("account.setUnified"))
        XCTAssertTrue(IosUnreadWidgets.commandChangesSnapshot("account.futureMutation"))
    }

    func testRowDatesStayStableBetweenTimelineReloads() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try XCTUnwrap(TimeZone(secondsFromGMT: 0))
        let now = try XCTUnwrap(calendar.date(from: DateComponents(year: 2026, month: 10, day: 3, hour: 12)))
        let thread = UnreadWidgetThread(id: "a#INBOX#1", sender: "", subject: "", date: now.addingTimeInterval(-120).timeIntervalSince1970)
        let locale = Locale(identifier: "en_US")
        let label = thread.dateLabel(relativeTo: now, calendar: calendar, locale: locale)
        XCTAssertEqual(label, thread.dateLabel(relativeTo: now.addingTimeInterval(1800), calendar: calendar, locale: locale))
        let nextDay = now.addingTimeInterval(86400)
        let oldLabel = thread.dateLabel(relativeTo: nextDay, calendar: calendar, locale: locale)
        XCTAssertNotEqual(label, oldLabel)
        XCTAssertEqual(oldLabel, thread.dateLabel(relativeTo: nextDay.addingTimeInterval(1800), calendar: calendar, locale: locale))
    }

    func testUnifiedCountExcludesOptedOutAccountsAndMissingAccountFallsBack() throws {
        let snapshot = try XCTUnwrap(UnreadWidgetSnapshotBuilder.build { method, params in
            switch method {
            case "account.list":
                return ["accounts": [["id": "a", "email": "a@example.com"], ["id": "b", "display_name": "Work", "included_in_unified": false]]]
            case "mail.folderList":
                return ["folders": [["name": "INBOX", "role": "inbox", "unread": params["account_id"] as? String == "a" ? 2 : 5]]]
            case "mail.threadList":
                XCTAssertEqual(params["refresh"] as? Bool, false)
                XCTAssertEqual(params["filter"] as? String, "unread")
                return ["threads": []]
            default: return nil
            }
        })
        XCTAssertEqual(snapshot.scope(nil)?.count, 2)
        XCTAssertEqual(snapshot.scope("b")?.count, 5)
        XCTAssertEqual(snapshot.scope("deleted")?.id, "unified")
        XCTAssertEqual(snapshot.scope("a")?.title, "a@example.com")
    }

    func testUnsyncedAccountIsZeroAndReadFailureDoesNotPublish() throws {
        let snapshot = try XCTUnwrap(UnreadWidgetSnapshotBuilder.build { method, _ in
            switch method {
            case "account.list": return ["accounts": [["id": "a"]]]
            case "mail.folderList": return ["folders": []]
            case "mail.threadList": return ["threads": []]
            default: return nil
            }
        })
        XCTAssertEqual(snapshot.scope("a")?.count, 0)
        XCTAssertNil(UnreadWidgetSnapshotBuilder.build { method, _ in
            method == "account.list" ? ["accounts": [["id": "a"]]] : nil
        })
    }

    func testThreadsUseSenderFallbackSortDeduplicateAndRoundTrip() throws {
        let snapshot = try XCTUnwrap(UnreadWidgetSnapshotBuilder.build { method, _ in
            method == "account.list" ? ["accounts": []] : ["threads": [
                ["id": "a#INBOX#2", "from_name": "", "from_addr": "sender@example.com", "subject": "Hi", "date": 10],
                ["id": "a#INBOX#3", "date": 20], ["id": "a#INBOX#3", "date": 20],
            ]]
        })
        XCTAssertEqual(snapshot.scope(nil)?.threads.map(\.id), ["a#INBOX#3", "a#INBOX#2"])
        XCTAssertEqual(snapshot.scope(nil)?.threads.last?.sender, "sender@example.com")
        let restored = try JSONDecoder().decode(UnreadWidgetSnapshot.self, from: JSONEncoder().encode(snapshot))
        XCTAssertEqual(restored, snapshot)
    }

    func testThreadLinksPreserveHeaderKeysFolderAndUnicode() throws {
        let key = "<abc@example.com>#日本語 & subject"
        let encoded = Data(key.utf8).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        let thread = UnreadWidgetThread(id: "acct#Folder with # sign#t.\(encoded)", sender: "", subject: "", date: 0)
        XCTAssertEqual(iosWidgetThreadTarget(thread.url), IosWidgetThreadTarget(accountId: "acct", folder: "Folder with # sign", threadKey: key))
        let simple = UnreadWidgetThread(id: "acct#INBOX#t.\(encoded)", sender: "", subject: "", date: 0)
        XCTAssertEqual(iosWidgetThreadTarget(simple.url), IosWidgetThreadTarget(accountId: "acct", folder: "INBOX", threadKey: key))
        XCTAssertEqual(URLComponents(url: thread.url, resolvingAgainstBaseURL: false)?.queryItems?.first?.value, thread.id)
        XCTAssertEqual(try iosWidgetThreadTarget(XCTUnwrap(URL(string: "meron://thread?id=acct%23INBOX%2312")))?.threadKey, "uid:12")
        XCTAssertEqual(try iosWidgetThreadTarget(XCTUnwrap(URL(string: "meron://thread?id=rss-a%23rss%23subscription")))?.threadKey, "subscription")
        XCTAssertNil(try iosWidgetThreadTarget(XCTUnwrap(URL(string: "https://thread?id=bad"))))
        XCTAssertNil(try iosWidgetThreadTarget(XCTUnwrap(URL(string: "meron://thread?id=a%23INBOX%23t.!!!"))))
    }
}
