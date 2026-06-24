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
}
