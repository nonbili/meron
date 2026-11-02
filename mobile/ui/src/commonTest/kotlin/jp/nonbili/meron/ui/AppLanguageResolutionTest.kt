package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/**
 * The platform's answer about the in-app language has three states, and
 * collapsing the last two is a real bug: Android 13+ reports "system default" as
 * an empty tag, which is an authoritative choice rather than an absent one.
 */
class AppLanguageResolutionTest {
    @Test
    fun noPlatformOpinionLeavesTheStoredTagInCharge() {
        // Android below 13 and iOS: nothing owns the setting but us.
        assertEquals("ja", resolveAppLanguageTag(systemTag = null, storedTag = "ja"))
        assertEquals("", resolveAppLanguageTag(systemTag = null, storedTag = ""))
        assertFalse(appLanguageNeedsPersisting(systemTag = null, storedTag = "ja"))
    }

    @Test
    fun aPlatformChosenLanguageWins() {
        assertEquals("ja", resolveAppLanguageTag(systemTag = "ja", storedTag = ""))
        assertEquals("ja", resolveAppLanguageTag(systemTag = "ja", storedTag = "fr"))
        assertTrue(appLanguageNeedsPersisting(systemTag = "ja", storedTag = "fr"))
    }

    /**
     * The regression this guards: pick Japanese in the app, then set the app's
     * language back to "System default" in Android settings. The empty tag has to
     * override the stored "ja", not be read as "the platform has no opinion".
     */
    @Test
    fun resettingToSystemDefaultOverridesAStoredLanguage() {
        assertEquals("", resolveAppLanguageTag(systemTag = "", storedTag = "ja"))
        assertTrue(
            appLanguageNeedsPersisting(systemTag = "", storedTag = "ja"),
            "the reset has to be written, or it comes back on the next launch",
        )
    }

    @Test
    fun anUnchangedPlatformChoiceIsNotRewritten() {
        assertFalse(appLanguageNeedsPersisting(systemTag = "ja", storedTag = "ja"))
        assertFalse(appLanguageNeedsPersisting(systemTag = "", storedTag = ""))
    }

    // ---- Device locale fallback ---------------------------------------------
    //
    // What "system default" resolves to once no language is chosen for the app.
    // These mirror resolveI18nLanguageFromWebLocale in the desktop frontend, so
    // the same device gets the same translation on desktop and mobile.

    @Test
    fun aRegionalLocaleFallsBackToItsLanguage() {
        assertEquals("ru", resolveDeviceLanguageTag("ru-RU"))
        assertEquals("ru", resolveDeviceLanguageTag("ru_BY"))
        assertEquals("fr", resolveDeviceLanguageTag("fr-FR"))
        assertEquals("de", resolveDeviceLanguageTag("de-AT"))
        assertEquals("ja", resolveDeviceLanguageTag("ja-JP"))
        assertEquals("en", resolveDeviceLanguageTag("en-GB"))
        // A bare language works too.
        assertEquals("ko", resolveDeviceLanguageTag("ko"))
    }

    @Test
    fun portugueseSplitsBrazilFromTheRest() {
        assertEquals("pt-BR", resolveDeviceLanguageTag("pt-BR"))
        assertEquals("pt", resolveDeviceLanguageTag("pt-PT"))
        assertEquals("pt", resolveDeviceLanguageTag("pt"))
    }

    @Test
    fun chineseUsesTheScriptThenTheRegion() {
        assertEquals("zh-Hans", resolveDeviceLanguageTag("zh-Hans-CN"))
        assertEquals("zh-Hant", resolveDeviceLanguageTag("zh-Hant-TW"))
        // No script: the region decides, and traditional regions are the exception.
        assertEquals("zh-Hant", resolveDeviceLanguageTag("zh-TW"))
        assertEquals("zh-Hant", resolveDeviceLanguageTag("zh-HK"))
        assertEquals("zh-Hans", resolveDeviceLanguageTag("zh-CN"))
        assertEquals("zh-Hans", resolveDeviceLanguageTag("zh"))
    }

    @Test
    fun anUntranslatedOrMissingLocaleFallsBackToEnglish() {
        assertEquals("en", resolveDeviceLanguageTag("nl-NL"))
        assertEquals("en", resolveDeviceLanguageTag("is"))
        assertEquals("en", resolveDeviceLanguageTag(""))
    }

    @Test
    fun theSeparatorAndCasingOfTheDeviceTagDoNotMatter() {
        // Android hands back BCP-47, but a JVM-style underscore tag must not
        // silently resolve to English.
        assertEquals("pt-BR", resolveDeviceLanguageTag("pt_BR"))
        assertEquals("fr", resolveDeviceLanguageTag("FR-fr"))
        assertEquals("zh-Hant", resolveDeviceLanguageTag("zh_hant_tw"))
    }
}
