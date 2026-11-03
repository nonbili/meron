package jp.nonbili.meron

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.util.Locale

class GeneratedLocalizationTest {
    @Test
    fun generatedAndroidXmlContainsEnglishAndJapaneseStrings() {
        assertEquals(
            "Cancel",
            generatedStringValue("src/main/res/values/strings.xml", "buttons_cancel"),
        )
        assertEquals(
            "キャンセル",
            generatedStringValue("src/main/res/values-ja/strings.xml", "buttons_cancel"),
        )
    }

    @Test
    fun generatedAndroidXmlUsesSpecialBrazilianPortugueseResourceFolder() {
        assertEquals(
            "Cancelar",
            generatedStringValue("src/main/res/values-pt-rBR/strings.xml", "buttons_cancel"),
        )
    }

    @Test
    fun generatedIcuHelperContainsPluralMessagesForSupportedLocales() {
        assertEquals("1 file", generatedIcuString("en", "chat.fileItems", mapOf("count" to 1)))
        assertEquals("2 files", generatedIcuString("en", "chat.fileItems", mapOf("count" to 2)))
        assertTrue(generatedIcuString("ar", "chat.fileItems", mapOf("count" to 2)).contains("2"))
        assertEquals("1 件の新しいメッセージ", generatedIcuString("ja", "notify.newMessageCount", mapOf("count" to 1)))
        assertEquals("3 件の新しいメッセージ", generatedIcuString("ja", "notify.newMessageCount", mapOf("count" to 3)))
    }

    @Test
    fun russianDeviceLocalesResolveToTheRussianCatalog() {
        assertEquals("ru", catalogLanguageTag(Locale.forLanguageTag("ru")))
        assertEquals("ru", catalogLanguageTag(Locale.forLanguageTag("ru-RU")))
    }

    @Test
    fun russianResourcesAndNotificationsUseRussianPluralForms() {
        assertEquals("Отмена", generatedStringValue("src/main/res/values-ru/strings.xml", "buttons_cancel"))
        val endings =
            mapOf(
                0 to "новых писем",
                1 to "новое письмо",
                2 to "новых письма",
                4 to "новых письма",
                5 to "новых писем",
                11 to "новых писем",
                12 to "новых писем",
                14 to "новых писем",
                21 to "новое письмо",
                22 to "новых письма",
                25 to "новых писем",
                111 to "новых писем",
            )
        for ((count, ending) in endings) {
            for (locale in listOf("ru", "ru-RU", "ru_RU")) {
                assertEquals(
                    "$count $ending",
                    generatedIcuString(locale, "notify.newMessageCount", mapOf("count" to count)),
                )
            }
        }
    }

    private fun generatedStringValue(
        resourcePath: String,
        name: String,
    ): String {
        val xml = File(resourcePath).readText()
        val regex = Regex("""<string name="$name">(.+)</string>""")
        return regex.find(xml)?.groupValues?.get(1)
            ?: error("Missing $name in $resourcePath")
    }
}
