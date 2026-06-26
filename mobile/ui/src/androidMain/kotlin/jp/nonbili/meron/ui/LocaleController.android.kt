package jp.nonbili.meron.ui

import android.app.LocaleManager
import android.content.Context
import android.os.Build
import android.os.LocaleList
import java.util.Locale

/** Android in-app locale via per-app language preferences (API 33+) plus a stored tag. */
class AndroidLocaleController(
    private val context: Context,
    private val prefs: AppPreferences,
) : LocaleController {
    override fun currentLanguageTag(): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return loadAppLanguageTag(prefs)
        val tag = context.getSystemService(LocaleManager::class.java).applicationLocales.toLanguageTags()
        val normalized = tag.substringBefore(",").takeIf { it in supportedAppLanguageTags }.orEmpty()
        if (normalized != loadAppLanguageTag(prefs)) {
            prefs.putString(APP_LANGUAGE_PREF, normalized)
        }
        return normalized
    }

    override fun apply(tag: String) {
        val normalized = tag.takeIf { it in supportedAppLanguageTags }.orEmpty()
        prefs.putString(APP_LANGUAGE_PREF, normalized)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.getSystemService(LocaleManager::class.java).applicationLocales =
                if (normalized.isBlank()) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(normalized)
        }
    }

    override fun displayName(tag: String): String {
        val locale = Locale.forLanguageTag(tag)
        return locale.getDisplayName(locale).replaceFirstChar { char ->
            if (char.isLowerCase()) char.titlecase(locale) else char.toString()
        }
    }
}
