package jp.nonbili.meron

import android.content.Context
import android.content.res.Configuration
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Locale

class AndroidLocalizationContractTest {
    @Test
    fun generatedStringResourcesResolveForEnglishAndJapanese() {
        val context = ApplicationProvider.getApplicationContext<Context>()

        assertEquals("Cancel", context.withLocale(Locale.ENGLISH).getString(R.string.buttons_cancel))
        assertEquals("キャンセル", context.withLocale(Locale.JAPANESE).getString(R.string.buttons_cancel))
    }

    @Test
    fun generatedSpecialLocaleResourceFolderResolvesForBrazilianPortuguese() {
        val context = ApplicationProvider.getApplicationContext<Context>()

        assertEquals("Cancelar", context.withLocale(Locale.forLanguageTag("pt-BR")).getString(R.string.buttons_cancel))
    }

    @Test
    fun generatedIcuHelperContainsPluralMessagesForSupportedLocales() {
        assertEquals("1 file", generatedIcuString("en", "chat.fileItems", mapOf("count" to 1)))
        assertEquals("2 files", generatedIcuString("en", "chat.fileItems", mapOf("count" to 2)))
        assertTrue(generatedIcuString("ar", "chat.fileItems", mapOf("count" to 2)).contains("2"))
    }

    private fun Context.withLocale(locale: Locale): Context {
        val config = Configuration(resources.configuration)
        config.setLocale(locale)
        return createConfigurationContext(config)
    }
}
