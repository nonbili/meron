package jp.nonbili.meron.ui

import android.app.LocaleManager
import android.content.Context
import android.content.res.Resources
import android.os.Build
import android.os.LocaleList
import java.util.Locale

/** Android in-app locale via per-app language preferences (API 33+). */
class AndroidLocaleController(
    private val context: Context,
) : LocaleController {
    override fun systemLanguageTag(): String? {
        // Below 13 there is no per-app language at all, so we own the choice.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return null
        val tag = context.getSystemService(LocaleManager::class.java).applicationLocales.toLanguageTags()
        // Empty means the user picked "System default" there — an answer, not a
        // shrug, so it is returned as "" rather than null.
        return tag.substringBefore(",").takeIf { it in supportedAppLanguageTags }.orEmpty()
    }

    override fun applySystem(tag: String) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val normalized = tag.takeIf { it in supportedAppLanguageTags }.orEmpty()
        context.getSystemService(LocaleManager::class.java).applicationLocales =
            if (normalized.isBlank()) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(normalized)
    }

    // Resources.getSystem() rather than Locale.getDefault(): the latter reports the
    // per-app locale when one is set, which would make this echo the app's own
    // choice instead of the device's.
    override fun deviceLanguageTag(): String {
        val configuration = Resources.getSystem().configuration
        // Configuration.locales arrived in API 24 (the project's minSdk).
        val locale =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                configuration.locales.takeIf { !it.isEmpty }?.get(0)
            } else {
                @Suppress("DEPRECATION")
                configuration.locale
            }
        return locale?.toLanguageTag().orEmpty()
    }

    override fun displayName(tag: String): String {
        val locale = Locale.forLanguageTag(tag)
        return locale.getDisplayName(locale).replaceFirstChar { char ->
            if (char.isLowerCase()) char.titlecase(locale) else char.toString()
        }
    }
}
