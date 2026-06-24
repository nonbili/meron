@testable import Meron
import XCTest

final class RustCoreBridgeContractTests: XCTestCase {
    func testRustCoreMatchesProtocolAndHandlesPing() throws {
        XCTAssertEqual(RustCoreBridge.protocolVersion(), 1)

        let dataDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("meron-ios-core-contract", isDirectory: true)
        try FileManager.default.createDirectory(
            at: dataDirectory,
            withIntermediateDirectories: true
        )

        let initJson = try jsonObject(RustCoreBridge.initJson(dataDirectory: dataDirectory.path))
        XCTAssertEqual(initJson["ok"] as? Bool, true)
        XCTAssertEqual(initJson["protocol"] as? Int, 1)
        XCTAssertEqual(initJson["data_dir"] as? String, dataDirectory.path)

        let pingJson = try jsonObject(
            RustCoreBridge.invokeJson(#"{"id":42,"method":"ping","params":{}}"#)
        )
        XCTAssertEqual(pingJson["id"] as? Int, 42)
        let result = try XCTUnwrap(pingJson["result"] as? [String: Any])
        XCTAssertEqual(result["pong"] as? Bool, true)
        XCTAssertEqual(result["protocol"] as? Int, 1)
    }

    func testRustCoreDeliversReadyEventThroughCAbi() throws {
        let events = RustCoreBridge.readyEvents()
        XCTAssertEqual(events.count, 1)

        let envelope = try jsonObject(events[0])
        XCTAssertEqual(envelope["event"] as? String, "ready")
        let detail = try XCTUnwrap(envelope["detail"] as? [String: Any])
        XCTAssertEqual(detail["protocol"] as? Int, 1)
    }

    private func jsonObject(_ raw: String) throws -> [String: Any] {
        let data = try XCTUnwrap(raw.data(using: .utf8))
        return try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any],
            "Expected JSON object from: \(raw)"
        )
    }
}
