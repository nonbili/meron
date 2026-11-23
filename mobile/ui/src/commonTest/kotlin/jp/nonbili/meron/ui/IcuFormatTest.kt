package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class IcuFormatTest {
    @Test
    fun pluralAfterLeadingTextUsesNestedArgument() {
        assertEquals(
            "Every week, 10 times",
            localizedString("en", "calendar.repeat.summaryCount", mapOf("rule" to "Every week", "count" to 10)),
        )
        assertEquals(
            "Every week, once",
            localizedString("en", "calendar.repeat.summaryCount", mapOf("rule" to "Every week", "count" to 1)),
        )
    }

    @Test
    fun multiplePluralsReplacePound() {
        assertEquals(
            "1 event to import into Work. 2 duplicates and 0 unsupported or invalid entries will be skipped.",
            localizedString(
                "en",
                "calendar.file.preview",
                mapOf("imported" to 1, "duplicates" to 2, "skipped" to 0, "calendar" to "Work"),
            ),
        )
        assertEquals(
            "Imported 3 events. 1 duplicate and 1 unsupported or invalid entry skipped.",
            localizedString("en", "calendar.file.imported", mapOf("imported" to 3, "duplicates" to 1, "skipped" to 1)),
        )
    }

    @Test
    fun extraPluralCategories() {
        assertEquals(
            "3 wydarzenia",
            formatPolishProbe(3),
        )
        assertEquals("5 wydarzeń", formatPolishProbe(5))
        assertEquals("1 wydarzenie", formatPolishProbe(1))
    }

    @Test
    fun zeroIsSingularInFrenchAndBrazilianPortuguese() {
        val template = "{n, plural, one {# item} other {# items}}"
        assertEquals("0 item", localizedString("fr", template, mapOf("n" to 0)))
        assertEquals("0 item", localizedString("pt-BR", template, mapOf("n" to 0)))
        assertEquals("0 items", localizedString("pt", template, mapOf("n" to 0)))
        assertEquals("0 items", localizedString("en", template, mapOf("n" to 0)))
    }

    @Test
    fun negativeExactSelectors() {
        val template = "{n, plural, =-1 {negative} other {# other}}"
        assertEquals("negative", localizedString("en", template, mapOf("n" to -1)))
        assertEquals("3 other", localizedString("en", template, mapOf("n" to 3)))
        assertEquals("other", localizedString("ru", "{n, plural, other {other}}", mapOf("n" to Long.MIN_VALUE)))
    }

    private fun formatPolishProbe(count: Int): String =
        localizedString("pl", "probe {n, plural, one {# wydarzenie} few {# wydarzenia} many {# wydarzeń} other {# wydarzenia}}", mapOf("n" to count))
            .removePrefix("probe ")
}
