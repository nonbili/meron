import MeronShared
import XCTest
@testable import Meron

final class MailtoUrlContractTests: XCTestCase {
    func testMailtoUrlPrefillsComposeFields() throws {
        let draft = try XCTUnwrap(
            MailtoKt.parseMailtoUrl(
                rawUrl: "mailto:to@example.com?cc=cc@example.com&bcc=bcc@example.com" +
                    "&subject=Mobile%20Draft&body=Line%20one%0ALine%20two"
            )
        )
        var to = ""
        var cc = ""
        var bcc = ""
        var subject = ""
        var body = ""

        applyMailtoDraftToCompose(
            draft,
            to: &to,
            cc: &cc,
            bcc: &bcc,
            subject: &subject,
            body: &body
        )

        XCTAssertEqual(to, "to@example.com")
        XCTAssertEqual(cc, "cc@example.com")
        XCTAssertEqual(bcc, "bcc@example.com")
        XCTAssertEqual(subject, "Mobile Draft")
        XCTAssertEqual(body, "Line one\nLine two")
    }
}
