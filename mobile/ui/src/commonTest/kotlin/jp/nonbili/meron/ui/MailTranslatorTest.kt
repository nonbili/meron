package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.MessageBody
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MailTranslatorTest {
    @Test
    fun switchesBetweenTheOriginalAndCachedTranslation() =
        runBlocking {
            var calls = 0
            val translator =
                FakeTranslator { text, _ ->
                    assertEquals("Bonjour", text)
                    calls += 1
                    MailTranslation("Good morning", "fr")
                }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Bonjour\n\n> Earlier text", bodyQuoteStart = 9)
            val translations = MessageTranslations(translator, "en", null, this)
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
    fun keepsTheLastTranslationWhileAnotherLanguageLoadsAndOnCancel() =
        runBlocking<Unit> {
            val gate = CompletableDeferred<Unit>()
            val translator =
                FakeTranslator { _, target ->
                    if (target == "de") gate.await()
                    MailTranslation("in $target", "fr")
                }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Bonjour")
            val translations = MessageTranslations(translator, "en", null, this)
            translations.toggle(message)
            yield()
            assertEquals("in en", translations.state(message)?.translatedBody)
            translations.translate(message, german)
            yield()
            assertTrue(translations.state(message)!!.loading)
            assertEquals("in en", translations.state(message)?.translatedBody)
            translations.cancel(message)
            assertFalse(translations.state(message)!!.loading)
            assertEquals(english, translations.state(message)?.target)
            assertEquals("in en", translations.state(message)?.translatedBody)
            gate.complete(Unit)
        }

    @Test
    fun cancellingTheFirstTranslationClearsIt() =
        runBlocking<Unit> {
            val gate = CompletableDeferred<Unit>()
            val translator =
                FakeTranslator { _, _ ->
                    gate.await()
                    MailTranslation("Hello", "fr")
                }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Bonjour")
            val translations = MessageTranslations(translator, "en", null, this)
            translations.toggle(message)
            yield()
            translations.cancel(message)
            assertNull(translations.state(message))
            gate.complete(Unit)
        }

    @Test
    fun reportsTextAlreadyInTheTargetLanguage() =
        runBlocking {
            val translator = FakeTranslator { text, _ -> MailTranslation(text, "en") }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Hello")
            val translations = MessageTranslations(translator, "en", null, this)
            translations.toggle(message)
            yield()
            val state = translations.state(message)!!
            assertTrue(state.result!!.unchanged)
            assertNull(state.translatedBody)
        }

    @Test
    fun reportsWhyATranslationFailed() =
        runBlocking {
            var allowed: Boolean? = null
            val translator =
                FakeTranslator { _, _, allowMetered ->
                    allowed = allowMetered
                    if (!allowMetered) throw MailTranslationException(MailTranslationProblem.MeteredDownload)
                    MailTranslation("Hello", "fr")
                }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Bonjour")
            val translations = MessageTranslations(translator, "en", null, this)
            translations.toggle(message)
            yield()
            val failed = translations.state(message)!!
            assertTrue(failed.failed)
            assertEquals(MailTranslationProblem.MeteredDownload, failed.problem)
            translations.translate(message, failed.target, allowMeteredDownload = true)
            yield()
            assertEquals(true, allowed)
            assertEquals("Hello", translations.state(message)?.translatedBody)
        }

    @Test
    fun cancellingAFailedSwitchRestoresThePreviousTranslation() =
        runBlocking {
            val translator =
                FakeTranslator { _, target ->
                    if (target == "de") throw MailTranslationException(MailTranslationProblem.Offline)
                    MailTranslation("in $target", "fr")
                }
            val message = MessageBody(id = "test", from = "", to = "", subject = "", body = "Bonjour")
            val translations = MessageTranslations(translator, "en", null, this)
            translations.toggle(message)
            yield()
            translations.translate(message, german)
            yield()
            assertTrue(translations.state(message)!!.failed)
            translations.cancel(message)
            val restored = translations.state(message)!!
            assertFalse(restored.failed)
            assertNull(restored.problem)
            assertEquals(english, restored.target)
            assertEquals("in en", restored.translatedBody)
        }

    @Test
    fun remembersTheChosenLanguage() =
        runBlocking {
            val prefs = StringPreferences()
            val translator = FakeTranslator { _, target -> MailTranslation("in $target", "fr") }
            val first = MessageBody(id = "a", from = "", to = "", subject = "", body = "Bonjour")
            val second = MessageBody(id = "b", from = "", to = "", subject = "", body = "Salut")
            val translations = MessageTranslations(translator, "en", prefs, this)
            translations.choose(first, german)
            yield()
            assertEquals("in de", translations.state(first)?.translatedBody)
            MessageTranslations(translator, "en", prefs, this).let { later ->
                later.toggle(second)
                yield()
                assertEquals("in de", later.state(second)?.translatedBody)
            }
        }

    @Test
    fun translatesWhatTheReaderSees() {
        val body = "```\nRead [the guide](https://example.com/guide) now\n```\n\n> quoted"
        val message = MessageBody(id = "test", from = "", to = "", subject = "", body = body, bodyQuoteStart = body.indexOf('>'))
        assertEquals("Read the guide now", message.translatableText())
    }

    @Test
    fun keepsLineBreaksAndRejoinsWrappedProse() =
        runBlocking {
            val wrapped = "This paragraph was wrapped by a mail client at seventy-two columns and"
            val text = "Name : Denso\nCode : 6902\n\n$wrapped\ncontinues here.\n  Indented\n----\nName : Denso"
            val seen = mutableListOf<String>()
            val result =
                translateMailText(text) {
                    seen += it
                    it.uppercase()
                }
            assertEquals(
                "NAME : DENSO\nCODE : 6902\n\n${wrapped.uppercase()} CONTINUES HERE.\n  INDENTED\n----\nNAME : DENSO",
                result,
            )
            assertEquals(listOf("Name : Denso", "Code : 6902", "$wrapped continues here.", "Indented"), seen)
        }

    @Test
    fun doesNotJoinCjkLines() {
        val line = "下記銘柄の配当金が入金されました。".repeat(4).dropLast(1)
        assertEquals(listOf(line, "次の行"), mailTranslationSegments("$line\n次の行"))
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

    private val english = MailTranslationLanguage("en", "English")
    private val german = MailTranslationLanguage("de", "German")

    private inner class FakeTranslator(
        private val onTranslate: suspend (String, String, Boolean) -> MailTranslation,
    ) : MailTranslator {
        constructor(onTranslate: suspend (String, String) -> MailTranslation) : this({ text, target, _ -> onTranslate(text, target) })

        override suspend fun languages(displayLanguage: String) = listOf(english, german)

        override suspend fun translate(
            text: String,
            targetLanguage: String,
            allowMeteredDownload: Boolean,
            onDownloading: () -> Unit,
        ) = onTranslate(text, targetLanguage, allowMeteredDownload)
    }

    private class StringPreferences : AppPreferences {
        private val values = mutableMapOf<String, String>()

        override fun getString(
            key: String,
            default: String,
        ): String = values[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            values[key] = value
        }

        override fun getBoolean(
            key: String,
            default: Boolean,
        ): Boolean = default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {}

        override fun getInt(
            key: String,
            default: Int,
        ): Int = default

        override fun putInt(
            key: String,
            value: Int,
        ) {}

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {}

        override fun remove(key: String) {
            values.remove(key)
        }
    }
}
