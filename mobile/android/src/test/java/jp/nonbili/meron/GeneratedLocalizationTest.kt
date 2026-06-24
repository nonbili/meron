package jp.nonbili.meron

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

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
