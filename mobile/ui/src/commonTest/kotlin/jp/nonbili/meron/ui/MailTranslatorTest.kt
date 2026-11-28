package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.MessageBody
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class MailTranslatorTest {
    @Test
    fun switchesBetweenTheOriginalAndCachedTranslation() =
        runBlocking {
            var calls = 0
            val english = MailTranslationLanguage("en", "English")
            val translator =
                object : MailTranslator {
                    override suspend fun languages() = listOf(english)

                    override suspend fun translate(
                        text: String,
                        targetLanguage: String,
                    ): MailTranslation {
                        assertEquals("Bonjour", text)
                        calls += 1
                        return MailTranslation("Good morning", "fr")
                    }
                }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Bonjour\n\n> Earlier text", bodyQuoteStart = 9)
            val translations = MessageTranslations(translator, "en", this)
            translations.toggle(message)
            assertNull(translations.state(message)?.translatedBody)
            yield()
            assertEquals("Good morning", translations.state(message)?.translatedBody)
            translations.toggle(message)
            assertNull(translations.state(message)?.translatedBody)
            translations.toggle(message)
            assertEquals("Good morning", translations.state(message)?.translatedBody)
            assertEquals(1, calls)
            assertNull(translations.state(message.copy(body = "Bonsoir")))
            assertNull(translations.state(message.copy(bodyQuoteStart = null)))
        }

    @Test
    fun prefersTheExactScriptOrRegion() {
        val simplified = MailTranslationLanguage("zh-Hans", "Simplified Chinese")
        val traditional = MailTranslationLanguage("zh-Hant", "Traditional Chinese")
        assertEquals(traditional, defaultMailTranslationTarget(listOf(simplified, traditional), "zh_Hant"))
    }

    @Test
    fun usesABaseTagWhenTheEngineCombinesRegionalVariants() {
        val english = MailTranslationLanguage("en", "English")
        val portuguese = MailTranslationLanguage("pt", "Portuguese")
        assertEquals(portuguese, defaultMailTranslationTarget(listOf(english, portuguese), "pt-BR"))
    }

    @Test
    fun avoidsAnUnsupportedScriptWhenChoosingTheDefault() {
        val english = MailTranslationLanguage("en", "English")
        val simplified = MailTranslationLanguage("zh-Hans", "Simplified Chinese")
        assertEquals(english, defaultMailTranslationTarget(listOf(simplified, english), "zh_Hant"))
        assertEquals(english, defaultMailTranslationTarget(listOf(simplified, english), "zh-TW"))
        assertEquals(simplified, defaultMailTranslationTarget(listOf(english, simplified), "zh-Hans-CN"))
    }

    @Test
    fun usesDeviceLanguageEvenWithoutAMatchingAppCatalog() {
        val english = MailTranslationLanguage("en", "English")
        val thai = MailTranslationLanguage("th", "Thai")
        assertEquals(thai, defaultMailTranslationTarget(listOf(english, thai), "th-TH"))
    }

    @Test
    fun matchesTranslationEngineLanguageAliases() {
        val english = MailTranslationLanguage("en", "English")
        val norwegian = MailTranslationLanguage("no", "Norwegian")
        val tagalog = MailTranslationLanguage("tl", "Tagalog")
        val languages = listOf(english, norwegian, tagalog)
        assertEquals(norwegian, defaultMailTranslationTarget(languages, "nb-NO"))
        assertEquals(tagalog, defaultMailTranslationTarget(languages, "fil-PH"))
    }

    @Test
    fun fallsBackToAnAvailableLanguage() {
        val english = MailTranslationLanguage("en", "English")
        val french = MailTranslationLanguage("fr", "French")
        assertEquals(english, defaultMailTranslationTarget(listOf(french, english), "et"))
        assertEquals(french, defaultMailTranslationTarget(listOf(french), "et"))
        assertNull(defaultMailTranslationTarget(emptyList(), "et"))
    }
}
