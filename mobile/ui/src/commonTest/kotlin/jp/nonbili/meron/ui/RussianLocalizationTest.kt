package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class RussianLocalizationTest {
    @Test
    fun russianCatalogFormatsPlaceholdersAndPlurals() {
        assertEquals("Отмена", localizedString("ru", "buttons.cancel"))
        assertEquals("Версия 1.2.3", localizedString("ru", "about.version", mapOf("version" to "1.2.3")))
        val endings =
            mapOf(
                0 to "писем",
                1 to "письмо",
                2 to "письма",
                4 to "письма",
                5 to "писем",
                11 to "писем",
                12 to "писем",
                14 to "писем",
                21 to "письмо",
                22 to "письма",
                25 to "писем",
                101 to "письмо",
                111 to "писем",
            )
        for ((count, ending) in endings) {
            for (locale in listOf("ru", "ru-RU", "ru_RU")) {
                assertEquals("$count $ending", localizedString(locale, "chat.messagesCount", mapOf("count" to count)))
            }
        }
        assertEquals(
            "Не удалось обновить 22 аккаунта",
            localizedString("ru", "mobile.refresh.failed", mapOf("failed" to 22)),
        )
        assertEquals("Cancel", localizedString("en", "buttons.cancel"))
    }
}
