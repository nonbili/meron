@testable import Meron
import XCTest

final class LocalizationContractTests: XCTestCase {
    func testGeneratedStringCatalogResolvesEnglishAndJapanese() throws {
        let bundle = try XCTUnwrap(Bundle(identifier: "jp.nonbili.meron"))

        XCTAssertEqual(
            bundle.localizedString(forKey: "buttons.cancel", value: nil, table: nil, localeIdentifier: "en"),
            "Cancel"
        )
        XCTAssertEqual(
            bundle.localizedString(forKey: "buttons.cancel", value: nil, table: nil, localeIdentifier: "ja"),
            "キャンセル"
        )
    }

    func testGeneratedSpecialLocaleIdentifierResolvesBrazilianPortuguese() throws {
        let bundle = try XCTUnwrap(Bundle(identifier: "jp.nonbili.meron"))

        XCTAssertEqual(
            bundle.localizedString(forKey: "buttons.cancel", value: nil, table: nil, localeIdentifier: "pt-BR"),
            "Cancelar"
        )
    }

    func testGeneratedCatalogHelperFormatsPlaceholdersAndPlurals() throws {
        let bundle = try XCTUnwrap(Bundle(identifier: "jp.nonbili.meron"))

        XCTAssertEqual(
            localizedCatalogString(
                "about.version",
                localeIdentifier: "en",
                args: ["version": "1.2.3"],
                bundle: bundle
            ),
            "Version 1.2.3"
        )
        XCTAssertEqual(
            localizedCatalogString(
                "chat.fileItems",
                localeIdentifier: "en",
                args: ["count": 1],
                bundle: bundle
            ),
            "1 file"
        )
        XCTAssertEqual(
            localizedCatalogString(
                "chat.fileItems",
                localeIdentifier: "en",
                args: ["count": 2],
                bundle: bundle
            ),
            "2 files"
        )
    }

    func testIosAppLanguageOptionsUseGeneratedCatalogLocaleIdentifiers() {
        XCTAssertEqual(iosNormalizedAppLanguageTag("pt-BR"), "pt-BR")
        XCTAssertEqual(iosNormalizedAppLanguageTag("zh-Hans"), "zh-Hans")
        XCTAssertEqual(iosNormalizedAppLanguageTag("pt_BR"), "")
        XCTAssertEqual(iosAppLocale("ja").identifier, "ja")
        XCTAssertEqual(iosAppLanguageDisplayName("pt-BR"), "Português (Brasil)")
    }

}

private extension Bundle {
    func localizedString(
        forKey key: String,
        value: String?,
        table: String?,
        localeIdentifier: String
    ) -> String {
        guard
            let path = path(forResource: localeIdentifier, ofType: "lproj"),
            let localizedBundle = Bundle(path: path)
        else {
            return localizedString(forKey: key, value: value, table: table)
        }
        return localizedBundle.localizedString(forKey: key, value: value, table: table)
    }
}
