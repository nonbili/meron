package jp.nonbili.meron

import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.content.res.Resources
import android.net.Uri
import android.os.Build
import android.os.LocaleList
import android.provider.OpenableColumns
import android.util.Base64
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.parseMailtoUrl
import java.util.Locale
import java.util.UUID

private const val APP_PREFS = "meron_app"

// The in-app language, as the settings screen stores it: same namespace and key
// as the shared UI's APP_LANGUAGE_PREF (mobile/ui AppModels.kt), because that
// write is the only record of the choice below Android 13.
private const val APP_LANGUAGE_PREF = "app_language_v1"
internal const val INBOX_FOLDER = "inbox"
internal const val LIVE_MAIL_PUSH_PREF = "live_mail_push_v1"
internal const val BACKGROUND_SYNC_ENABLED_PREF = "background_sync_enabled_v1"
private val supportedAppLanguageTags =
    setOf("ar", "de", "el", "en", "es", "et", "fr", "it", "ja", "ko", "lv", "pl", "pt", "pt-BR", "ru", "sv", "tr", "vi", "zh-Hans", "zh-Hant")

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

/** The catalog tag for a resolved locale. The generated ICU tables are keyed by
 *  script for Chinese (`zh-Hans`/`zh-Hant`), which `toLanguageTag()` never
 *  yields for a plain `zh-CN` device locale, so a raw tag would miss the
 *  catalog and fall through to English. */
internal fun catalogLanguageTag(locale: Locale): String {
    val tag = locale.toLanguageTag()
    if (tag in supportedAppLanguageTags) return tag
    if (locale.language == "zh") {
        val traditional = locale.script == "Hant" || locale.country in setOf("TW", "HK", "MO")
        return if (traditional) "zh-Hant" else "zh-Hans"
    }
    return locale.language.takeIf { it in supportedAppLanguageTags } ?: "en"
}

/**
 * The device's own locale. Read from the system resources rather than
 * `Locale.getDefault()`, which reports whatever [localizedAppContext] last
 * installed and so could never answer "what would the system have used".
 */
private fun systemLocale(): Locale {
    val configuration = Resources.getSystem().configuration
    val locale =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            configuration.locales.takeIf { !it.isEmpty }?.get(0)
        } else {
            @Suppress("DEPRECATION")
            configuration.locale
        }
    return locale ?: Locale.ENGLISH
}

/**
 * [base] with the in-app language applied, for code that resolves resources
 * outside an Activity (notifications posted from services and workers).
 *
 * From Android 13 the platform owns this: the per-app locale it applies covers
 * every context in the process and stays correct even when a background
 * component starts the process, so it is returned untouched rather than pinned
 * to whatever tag the app happened to store last.
 */
internal fun localizedAppContext(base: Context): Context {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) return base
    val tag = loadAppLanguageTag(base)
    if (tag.isBlank()) {
        // "Follow the system" has to undo the setDefault below: the process
        // outlives the choice, and anything formatting through
        // Locale.getDefault() (dates, times) would keep speaking the language
        // the user just left.
        Locale.setDefault(systemLocale())
        return base
    }
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

internal fun Intent.toSharedComposeDraft(context: Context): ComposeDraft? {
    val mailtoDraft = toMailtoDraft()
    if (mailtoDraft != null) return mailtoDraft
    if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) return null

    val subject = getStringExtra(Intent.EXTRA_SUBJECT).orEmpty()
    val text =
        getCharSequenceExtra(Intent.EXTRA_TEXT)
            ?.toString()
            ?: getStringExtra(Intent.EXTRA_HTML_TEXT).orEmpty()
    val attachments = streamUris().mapNotNull { it.toDraftAttachment(context, type) }
    if (subject.isBlank() && text.isBlank() && attachments.isEmpty()) return null

    return ComposeDraft(subject = subject, body = text, attachments = attachments)
}

@Suppress("DEPRECATION")
private fun Intent.streamUris(): List<Uri> {
    val uris = mutableListOf<Uri>()
    getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.let { uris += it }
    getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)?.let { uris += it }
    val clip = clipData
    if (clip != null) {
        for (index in 0 until clip.itemCount) {
            clip.getItemAt(index).uri?.let { uris += it }
        }
    }
    return uris.distinctBy { it.toString() }
}

private fun Uri.toDraftAttachment(
    context: Context,
    intentMimeType: String?,
): DraftAttachment? =
    runCatching {
        val bytes = context.contentResolver.openInputStream(this)?.use { it.readBytes() } ?: return null
        val mimeType =
            context.contentResolver.getType(this)
                ?: intentMimeType?.takeUnless { it == "*/*" }
                ?: "application/octet-stream"
        val displayName = context.displayNameFor(this)
        DraftAttachment(
            id = UUID.randomUUID().toString(),
            displayName = displayName,
            mimeType = mimeType,
            sizeBytes = bytes.size.toLong(),
            dataBase64 = Base64.encodeToString(bytes, Base64.NO_WRAP),
        )
    }.getOrNull()

internal fun Intent.toOAuthCallbackUrl(): String? {
    val uri = data?.toString() ?: return null
    return uri.takeIf { isPotentialOAuthCallbackUrl(it) || isOAuthCallbackUrl(it) }
}
