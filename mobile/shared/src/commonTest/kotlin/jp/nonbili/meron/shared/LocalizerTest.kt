package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals

class LocalizerTest {
    @Test
    fun mapLocalizerFormatsSimpleNamedArguments() {
        val localizer = MapLocalizer(mapOf("mobile.mail.needsReconnect" to "{account} needs credentials"))

        assertEquals(
            "Personal needs credentials",
            localizer.text("mobile.mail.needsReconnect", mapOf("account" to "Personal")),
        )
    }

    @Test
    fun missingKeysFallBackToStableKey() {
        val localizer = MapLocalizer(emptyMap())

        assertEquals("missing.key", localizer.text("missing.key"))
    }
}
