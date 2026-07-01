package jp.nonbili.meron

import android.app.LocaleManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.LocaleList
import android.provider.OpenableColumns
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.parseMailtoUrl
import java.util.Locale

private const val APP_PREFS = "meron_app"
private const val APP_LANGUAGE_PREF = "app_language"
internal const val INBOX_FOLDER = "inbox"
internal const val LIVE_MAIL_PUSH_PREF = "live_mail_push_v1"
internal const val BACKGROUND_SYNC_ENABLED_PREF = "background_sync_enabled_v1"
private val supportedAppLanguageTags =
    setOf("ar", "de", "el", "en", "es", "et", "fr", "it", "ja", "ko", "lv", "pl", "pt", "pt-BR", "sv", "tr", "vi", "zh-Hans", "zh-Hant")

internal fun loadAppLanguageTag(context: Context): String =
    context
        .getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .getString(APP_LANGUAGE_PREF, "")
        .orEmpty()
        .takeIf { it in supportedAppLanguageTags }
        .orEmpty()

internal fun loadAppBoolean(
    context: Context,
    key: String,
    defaultValue: Boolean,
): Boolean =
    context
        .getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .getBoolean(key, defaultValue)

internal fun syncAppLanguageFromSystemSetting(context: Context): String {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return loadAppLanguageTag(context)
    val tag = context.getSystemService(LocaleManager::class.java).applicationLocales.toLanguageTags()
    val normalized = tag.substringBefore(",").takeIf { it in supportedAppLanguageTags }.orEmpty()
    if (normalized != loadAppLanguageTag(context)) {
        context
            .getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(APP_LANGUAGE_PREF, normalized)
            .apply()
    }
    return normalized
}

internal fun localizedAppContext(base: Context): Context {
    val tag = loadAppLanguageTag(base)
    if (tag.isBlank()) return base
    val locale = Locale.forLanguageTag(tag)
    Locale.setDefault(locale)
    val configuration = Configuration(base.resources.configuration)
    configuration.setLocale(locale)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
        configuration.setLocales(LocaleList(locale))
    }
    return base.createConfigurationContext(configuration)
}

internal fun Context.displayNameFor(uri: Uri): String {
    contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
        if (cursor.moveToFirst()) {
            val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (index >= 0) {
                val value = cursor.getString(index)
                if (!value.isNullOrBlank()) return value
            }
        }
    }
    return uri.lastPathSegment?.substringAfterLast('/') ?: "attachment"
}

internal fun Intent.toMailtoDraft(): ComposeDraft? {
    val uri = data?.toString() ?: return null
    if (action != Intent.ACTION_SENDTO && action != Intent.ACTION_VIEW) return null
    return parseMailtoUrl(uri)
}

internal fun Intent.toOAuthCallbackUrl(): String? {
    val uri = data?.toString() ?: return null
    return uri.takeIf { isPotentialOAuthCallbackUrl(it) || isOAuthCallbackUrl(it) }
}
