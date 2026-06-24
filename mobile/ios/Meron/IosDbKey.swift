import Foundation
import Security

/// Provides the SQLCipher passphrase (64 hex chars = a raw 32-byte key) for the
/// local meron.db.
///
/// The passphrase is random, generated once, and stored in the iOS Keychain.
/// It is protected with `kSecAttrAccessibleAfterFirstUnlock` so the background
/// refresh task can open the store while the device is locked (but not before
/// the first unlock after boot). Keychain items are hardware-encrypted and
/// scoped to this app, so the passphrase is not recoverable from the DB file.
enum IosDbKey {
    private static let service = "jp.nonbili.meron"
    private static let account = "meron_db_key"
    private static let keyBytes = 32

    static func get() -> String {
        if let existing = load() {
            return existing
        }
        let key = randomHexKey()
        store(key)
        return key
    }

    private static func load() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data,
              let key = String(data: data, encoding: .utf8)
        else {
            return nil
        }
        return key
    }

    private static func store(_ key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: Data(key.utf8),
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        SecItemDelete(query as CFDictionary)
        SecItemAdd(query as CFDictionary, nil)
    }

    private static func randomHexKey() -> String {
        var bytes = [UInt8](repeating: 0, count: keyBytes)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}
