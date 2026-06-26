package jp.nonbili.meron.ui

import platform.Foundation.NSLocale
import platform.Foundation.NSUserDefaults
import platform.Foundation.localizedStringForLocaleIdentifier

/** iOS in-app locale via the AppleLanguages default plus a stored tag. */
class IosLocaleController(
    private val prefs: AppPreferences,
) : LocaleController {
    private val defaults = NSUserDefaults.standardUserDefaults

    override fun currentLanguageTag(): String = loadAppLanguageTag(prefs)

    override fun apply(tag: String) {
        val normalized = tag.takeIf { it in supportedAppLanguageTags }.orEmpty()
        prefs.putString(APP_LANGUAGE_PREF, normalized)
        if (normalized.isBlank()) {
            defaults.removeObjectForKey("AppleLanguages")
        } else {
            defaults.setObject(listOf(normalized), forKey = "AppleLanguages")
        }
    }

    override fun displayName(tag: String): String {
        val locale = NSLocale(localeIdentifier = tag)
        return locale.localizedStringForLocaleIdentifier(tag).replaceFirstChar { it.uppercaseChar() }
    }
}
