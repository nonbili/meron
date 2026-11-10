@testable import Meron
import XCTest

final class RustCoreBridgeContractTests: XCTestCase {
    func testDatabaseKeyFailuresDoNotGenerateOrReplaceKeys() throws {
        enum Failure: Error { case unavailable }
        var generated = false
        var stored = false
        let generate = { generated = true; return "new-key" }
        let store: (String) -> String = { stored = true; return $0 }

        XCTAssertThrowsError(try IosDbKey.resolve(load: { throw Failure.unavailable }, generate: generate, store: store, hasEncryptedStore: { false }))
        XCTAssertFalse(generated)
        XCTAssertFalse(stored)
        XCTAssertThrowsError(try IosDbKey.resolve(load: { nil }, generate: generate, store: store, hasEncryptedStore: { true }))
        XCTAssertFalse(generated)
        XCTAssertFalse(stored)
        XCTAssertEqual(try IosDbKey.resolve(load: { "original-key" }, generate: generate, store: store, hasEncryptedStore: { true }), "original-key")
        XCTAssertFalse(generated)
        XCTAssertFalse(stored)
        XCTAssertThrowsError(try IosDbKey.resolve(load: { nil }, generate: generate, store: { _ in throw Failure.unavailable }, hasEncryptedStore: { false }))
        XCTAssertTrue(generated)
        XCTAssertEqual(try IosDbKey.resolve(load: { nil }, generate: generate, store: { _ in "persisted-winner" }, hasEncryptedStore: { false }), "persisted-winner")
    }

    func testIosCoreResponseErrors() {
        XCTAssertEqual(iosCoreResponseError(#"{"error":{"message":"username required"}}"#)?.localizedDescription, "username required")
        XCTAssertNil(iosCoreResponseError(#"{"result":{"error":{"message":"not an envelope"}}}"#))
        XCTAssertNil(iosCoreResponseError(#"{"result":{"ok":true}}"#))
        XCTAssertNil(iosCoreResponseError("not JSON"))
    }

    func testOpenPgpLocalMailLab() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let email = environment["MERON_PGP_LAB_EMAIL"],
              let password = environment["MERON_PGP_LAB_PASSWORD"]
        else {
            throw XCTSkip("Opt-in local mail lab")
        }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("pgp-lab-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        _ = RustCoreBridge.initJson(dataDirectory: directory.path)
        func invoke(_ method: String, _ params: [String: Any]) throws -> [String: Any] {
            let data = try JSONSerialization.data(withJSONObject: ["id": 73, "method": method, "params": params])
            let response = try jsonObject(RustCoreBridge.invokeJson(String(decoding: data, as: UTF8.self)))
            XCTAssertNil(response["error"], "Core error for \(method)")
            return try XCTUnwrap(response["result"] as? [String: Any])
        }
        let added = try invoke("account.addPassword", [
            "email": email, "username": email, "password": password,
            "imap_host": "127.0.0.1", "imap_port": 3143,
            "smtp_host": "127.0.0.1", "smtp_port": 3587,
            "tls": false, "imap_tls": false, "smtp_tls": false,
            "starttls": false, "smtp_starttls": false,
        ])
        let account = try XCTUnwrap(added["account"] as? [String: Any])
        let id = try XCTUnwrap(account["id"] as? String)
        _ = try invoke("pgp.account.create", ["account": id])
        var message: [String: Any] = [
            "account": id, "to": email, "subject": "Meron iOS OpenPGP lab \(UUID().uuidString)",
            "body": "ios-openpgp-lab-secret", "encrypt": true,
            "message_id": "<ios-pgp-\(UUID().uuidString)@mail.localhost>",
        ]
        XCTAssertEqual(try invoke("mail.send", message)["ok"] as? Bool, true)
        message["draft_id"] = "<ios-pgp-draft-\(UUID().uuidString)@mail.localhost>"
        XCTAssertEqual(try invoke("mail.saveDraft", message)["ok"] as? Bool, true)
        _ = try invoke("mail.sync", ["account_id": id, "folder_id": "INBOX"])
        let page = try invoke("mail.threadList", ["account_id": id, "folder_id": "INBOX"])
        let threads = try XCTUnwrap(page["threads"] as? [[String: Any]])
        var decrypted = false
        for thread in threads {
            let threadId = try XCTUnwrap((thread["thread_id"] ?? thread["id"]) as? String)
            let read = try invoke("mail.threadRead", ["thread_id": threadId])
            for item in try XCTUnwrap(read["messages"] as? [[String: Any]]) {
                if (item["body"] as? String)?.contains("ios-openpgp-lab-secret") == true {
                    let crypto = try XCTUnwrap(item["crypto"] as? [String: Any])
                    XCTAssertEqual(crypto["encrypted"] as? Bool, true)
                    XCTAssertEqual(crypto["signed"] as? Bool, true)
                    decrypted = true
                }
            }
        }
        XCTAssertTrue(decrypted, "Encrypted mail decrypted after SMTP/IMAP round trip")
        let draftsPage = try invoke("mail.threadList", ["account_id": id, "folder_id": "Drafts"])
        var decryptedDraft = false
        for thread in try XCTUnwrap(draftsPage["threads"] as? [[String: Any]]) {
            let threadId = try XCTUnwrap((thread["thread_id"] ?? thread["id"]) as? String)
            let read = try invoke("mail.threadRead", ["thread_id": threadId])
            for item in try XCTUnwrap(read["messages"] as? [[String: Any]]) {
                if (item["body"] as? String)?.contains("ios-openpgp-lab-secret") == true {
                    let crypto = try XCTUnwrap(item["crypto"] as? [String: Any])
                    XCTAssertEqual(crypto["encrypted"] as? Bool, true)
                    decryptedDraft = true
                }
            }
        }
        XCTAssertTrue(decryptedDraft, "Encrypted draft decrypted after IMAP round trip")
    }

    func testOpenPgpKeysRoundTripThroughPackagedCore() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("pgp-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        XCTAssertEqual(try jsonObject(RustCoreBridge.initJson(dataDirectory: directory.path))["ok"] as? Bool, true)
        func invoke(_ method: String, _ params: [String: Any]) throws -> [String: Any] {
            let data = try JSONSerialization.data(withJSONObject: ["id": 72, "method": method, "params": params])
            let response = try jsonObject(RustCoreBridge.invokeJson(String(decoding: data, as: UTF8.self)))
            XCTAssertNil(response["error"], "Core error for \(method)")
            return try XCTUnwrap(response["result"] as? [String: Any])
        }
        let added = try invoke("account.addPassword", [
            "email": "ios-pgp@example.org", "username": "ios-pgp@example.org", "password": "test-only",
            "imap_host": "localhost", "smtp_host": "localhost",
        ])
        let account = try XCTUnwrap(added["account"] as? [String: Any])
        let id = try XCTUnwrap(account["id"] as? String)
        let params: [String: Any] = ["account": id]
        XCTAssertEqual(try invoke("pgp.account.state", params)["enabled"] as? Bool, false)
        let created = try invoke("pgp.account.create", params)
        let key = try XCTUnwrap(created["key"] as? [String: Any])
        let fingerprint = try XCTUnwrap(key["fingerprint"] as? String)
        let exported = try invoke("pgp.account.exportSecret", ["account": id, "passphrase": "mobile-test-passphrase"])
        let secret = try XCTUnwrap(exported["armored"] as? String)
        XCTAssertTrue(secret.contains("BEGIN PGP PRIVATE KEY"))
        _ = try invoke("pgp.account.disable", params)
        XCTAssertEqual(try invoke("pgp.account.state", params)["enabled"] as? Bool, false)
        let imported = try invoke("pgp.account.importKey", ["account": id, "data": secret, "passphrase": "mobile-test-passphrase"])
        let result = try XCTUnwrap(imported["result"] as? [String: Any])
        XCTAssertEqual(result["status"] as? String, "imported")
        let importedKey = try XCTUnwrap(result["key"] as? [String: Any])
        XCTAssertEqual(importedKey["fingerprint"] as? String, fingerprint)
        XCTAssertEqual(try invoke("pgp.account.state", params)["enabled"] as? Bool, true)
    }

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
