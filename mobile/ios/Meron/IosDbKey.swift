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
    private static let lock = NSLock()

    static func get() throws -> String {
        lock.lock()
        defer { lock.unlock() }
        let path = URL(fileURLWithPath: IosAppPaths.mobileDataDirectory()).appendingPathComponent("meron.db")
        return try resolve(load: load, generate: randomHexKey, store: store) {
            guard FileManager.default.fileExists(atPath: path.path) else { return false }
            let file = try FileHandle(forReadingFrom: path)
            defer { try? file.close() }
            let header = try file.read(upToCount: 16) ?? Data()
            return !header.isEmpty && header != Data("SQLite format 3\0".utf8)
        }
    }

    /// Injected operations keep regression tests away from the real profile/keychain.
    static func resolve(
        load: () throws -> String?,
        generate: () throws -> String,
        store: (String) throws -> String,
        hasEncryptedStore: () throws -> Bool
    ) throws -> String {
        if let existing = try load() { return existing }
        guard try !hasEncryptedStore() else {
            throw KeyError.missingStoreKey
        }
        return try store(generate())
    }

    enum KeyError: LocalizedError {
        case keychain(OSStatus)
        case invalidKey
        case missingStoreKey

        var errorDescription: String? {
            switch self {
            case let .keychain(status):
                return "Database keychain access failed (\(status)): \(SecCopyErrorMessageString(status, nil) as String? ?? "Unknown error")"
            case .invalidKey:
                return "The database keychain entry is invalid; it has not been replaced."
            case .missingStoreKey:
                return "The encrypted database's keychain key is missing. Restore the original key before opening the database; no replacement key has been generated."
            }
        }
    }

    private static func load() throws -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else { throw KeyError.keychain(status) }
        guard let data = item as? Data,
              let key = String(data: data, encoding: .utf8),
              key.utf8.count == keyBytes * 2,
              key.utf8.allSatisfy({ (48 ... 57).contains($0) || (97 ... 102).contains($0) || (65 ... 70).contains($0) })
        else {
            throw KeyError.invalidKey
        }
        return key
    }

    private static func store(_ key: String) throws -> String {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: Data(key.utf8),
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        let status = SecItemAdd(query as CFDictionary, nil)
        // Another process may have created the key after our initial read.
        // Never delete or overwrite it; use the persisted winner instead.
        if status == errSecDuplicateItem {
            guard let existing = try load() else { throw KeyError.keychain(status) }
            return existing
        }
        guard status == errSecSuccess else { throw KeyError.keychain(status) }
        return key
    }

    private static func randomHexKey() throws -> String {
        var bytes = [UInt8](repeating: 0, count: keyBytes)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else { throw KeyError.keychain(status) }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}
