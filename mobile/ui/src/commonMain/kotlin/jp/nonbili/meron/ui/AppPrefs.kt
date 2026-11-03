package jp.nonbili.meron.ui

internal fun loadAppBoolean(
    prefs: AppPreferences,
    key: String,
    defaultValue: Boolean,
): Boolean = prefs.getBoolean(key, defaultValue)

internal fun saveAppBoolean(
    prefs: AppPreferences,
    key: String,
    value: Boolean,
) = prefs.putBoolean(key, value)

internal fun loadAppInt(
    prefs: AppPreferences,
    key: String,
    defaultValue: Int,
): Int = prefs.getInt(key, defaultValue)

internal fun saveAppInt(
    prefs: AppPreferences,
    key: String,
    value: Int,
) = prefs.putInt(key, value)

internal fun loadAppStringSet(
    prefs: AppPreferences,
    key: String,
): Set<String> =
    prefs
        .getStringSet(key, emptySet())
        .filter { it.isNotBlank() }
        .toSet()

internal fun saveAppStringSet(
    prefs: AppPreferences,
    key: String,
    value: Set<String>,
) = prefs.putStringSet(key, value.filter { it.isNotBlank() }.toSet())

/**
 * Restore the last top-level screen (Mail/Kanban/Tasks/Calendar/Contacts).
 * Transient screens (Thread/Compose/AddAccount/Settings) are never persisted, so a
 * cold start always lands on a navigable top-level screen.
 */
internal fun loadLastTopScreen(prefs: AppPreferences): Screen =
    when (prefs.getString(LAST_TOP_SCREEN_PREF, "mail")) {
        "kanban" -> Screen.Kanban
        "tasks" -> Screen.Tasks
        "calendar" -> Screen.Calendar
        "contacts" -> Screen.Contacts
        else -> Screen.Mail
    }

internal fun saveLastTopScreen(
    prefs: AppPreferences,
    screen: Screen,
) = prefs.putString(
    LAST_TOP_SCREEN_PREF,
    when (screen) {
        Screen.Kanban -> "kanban"
        Screen.Tasks -> "tasks"
        Screen.Calendar -> "calendar"
        Screen.Contacts -> "contacts"
        else -> "mail"
    },
)

internal fun loadLastMailAccountId(prefs: AppPreferences): String = prefs.getString(LAST_MAIL_ACCOUNT_PREF, UNIFIED_ACCOUNT_ID).ifBlank { UNIFIED_ACCOUNT_ID }

private fun accountMailFolderKey(accountId: String): String = "last_mail_folder_by_account_v1_${accountId.ifBlank { UNIFIED_ACCOUNT_ID }}"

internal fun migrateLegacyMailFolder(prefs: AppPreferences) {
    val legacyFolder = prefs.getString(LAST_MAIL_FOLDER_PREF, "")
    if (legacyFolder.isNotBlank()) {
        val legacyKey = accountMailFolderKey(loadLastMailAccountId(prefs))
        if (prefs.getString(legacyKey, "").isBlank()) prefs.putString(legacyKey, legacyFolder)
        prefs.remove(LAST_MAIL_FOLDER_PREF)
    }
}

internal fun loadMailFolderForAccount(
    prefs: AppPreferences,
    accountId: String,
): String {
    val stored = prefs.getString(accountMailFolderKey(accountId), "")
    if (stored.isNotBlank()) return stored
    return if (accountId.ifBlank { UNIFIED_ACCOUNT_ID } == loadLastMailAccountId(prefs)) {
        prefs.getString(LAST_MAIL_FOLDER_PREF, INBOX_FOLDER).ifBlank { INBOX_FOLDER }
    } else {
        INBOX_FOLDER
    }
}

internal fun loadLastMailFolder(prefs: AppPreferences): String = loadMailFolderForAccount(prefs, loadLastMailAccountId(prefs))

internal fun saveLastMailLocation(
    prefs: AppPreferences,
    accountId: String,
    folder: String,
) {
    migrateLegacyMailFolder(prefs)
    prefs.putString(accountMailFolderKey(accountId), folder.ifBlank { INBOX_FOLDER })
    prefs.putString(LAST_MAIL_ACCOUNT_PREF, accountId.ifBlank { UNIFIED_ACCOUNT_ID })
}

/** Follow a folder rename in the folder remembered for [accountId], without
 *  making it the last-opened account. */
internal fun renameMailFolderForAccount(
    prefs: AppPreferences,
    accountId: String,
    renames: Map<String, String>,
) {
    migrateLegacyMailFolder(prefs)
    val key = accountMailFolderKey(accountId)
    val renamed = renames[prefs.getString(key, "")] ?: return
    prefs.putString(key, renamed)
}

internal fun clearMailFolderForAccount(
    prefs: AppPreferences,
    accountId: String,
) {
    migrateLegacyMailFolder(prefs)
    prefs.remove(accountMailFolderKey(accountId))
}

private fun loadAppTheme(
    prefs: AppPreferences,
    key: String,
    defaultValue: AppTheme,
    customThemes: List<CustomTheme>,
): AppTheme {
    val stored = prefs.getString(key, defaultValue.storageValue)
    return AppAppearanceMode.entries.firstOrNull { it.storageValue == stored }
        ?: customThemes.firstOrNull { it.storageValue == stored }
        ?: defaultValue
}

fun loadThemeChoice(
    prefs: AppPreferences,
    customThemes: List<CustomTheme> = emptyList(),
): ThemeChoice {
    val defaults = ThemeChoice()
    return ThemeChoice(
        fixed = loadAppTheme(prefs, APPEARANCE_MODE_PREF, defaults.fixed, customThemes),
        followSystem = prefs.getBoolean(THEME_FOLLOW_SYSTEM_PREF, defaults.followSystem),
        // A pick stored under the wrong appearance would never show; use the default.
        light = loadAppTheme(prefs, LIGHT_THEME_PREF, defaults.light, customThemes).takeUnless { it.isDark } ?: defaults.light,
        dark = loadAppTheme(prefs, DARK_THEME_PREF, defaults.dark, customThemes).takeIf { it.isDark } ?: defaults.dark,
    )
}

fun loadCustomThemes(prefs: AppPreferences): List<CustomTheme> = parseCustomThemes(prefs.getString(CUSTOM_THEMES_PREF, ""))

fun saveCustomThemes(
    prefs: AppPreferences,
    themes: List<CustomTheme>,
) = prefs.putString(CUSTOM_THEMES_PREF, serializeCustomThemes(themes))

fun saveThemeChoice(
    prefs: AppPreferences,
    choice: ThemeChoice,
) {
    prefs.putString(APPEARANCE_MODE_PREF, choice.fixed.storageValue)
    prefs.putBoolean(THEME_FOLLOW_SYSTEM_PREF, choice.followSystem)
    prefs.putString(LIGHT_THEME_PREF, choice.light.storageValue)
    prefs.putString(DARK_THEME_PREF, choice.dark.storageValue)
}

internal fun loadSendShortcutMode(prefs: AppPreferences): SendShortcutMode =
    when (prefs.getString(SEND_SHORTCUT_PREF, "mod_enter")) {
        "enter" -> SendShortcutMode.Enter
        else -> SendShortcutMode.ModEnter
    }

internal fun saveSendShortcutMode(
    prefs: AppPreferences,
    mode: SendShortcutMode,
) = prefs.putString(SEND_SHORTCUT_PREF, mode.storageValue())

internal fun loadConversationLayout(prefs: AppPreferences): ConversationLayout =
    when (prefs.getString(CONVERSATION_LAYOUT_PREF, "chat")) {
        "traditional" -> ConversationLayout.Traditional
        else -> ConversationLayout.Chat
    }

internal fun saveConversationLayout(
    prefs: AppPreferences,
    layout: ConversationLayout,
) = prefs.putString(CONVERSATION_LAYOUT_PREF, layout.storageValue())

internal fun loadMessageFontScale(prefs: AppPreferences): Int = coerceMessageFontScale(loadAppInt(prefs, MESSAGE_FONT_SCALE_PREF, DEFAULT_MESSAGE_FONT_SCALE))

internal fun saveMessageFontScale(
    prefs: AppPreferences,
    scale: Int,
) = saveAppInt(prefs, MESSAGE_FONT_SCALE_PREF, coerceMessageFontScale(scale))

internal fun loadAppLanguageTag(prefs: AppPreferences): String =
    prefs
        .getString(APP_LANGUAGE_PREF, "")
        .takeIf { it in supportedAppLanguageTags }
        .orEmpty()

/**
 * Store the in-app language. Unsupported tags normalize to blank ("follow the
 * system"), which is what the OS-level per-app locale also means when unset.
 */
internal fun saveAppLanguageTag(
    prefs: AppPreferences,
    tag: String,
) = prefs.putString(APP_LANGUAGE_PREF, tag.takeIf { it in supportedAppLanguageTags }.orEmpty())

/**
 * The language to display, given the platform's answer and our stored tag.
 *
 * A non-null platform answer always wins — including `""`, which means the user
 * chose "system default" in the OS and any stored language must stop applying.
 */
internal fun resolveAppLanguageTag(
    systemTag: String?,
    storedTag: String,
): String = systemTag ?: storedTag

/** Whether the platform's answer differs from what we have stored. */
internal fun appLanguageNeedsPersisting(
    systemTag: String?,
    storedTag: String,
): Boolean = systemTag != null && systemTag != storedTag

/**
 * Pick the catalog language for a device locale such as `fr-FR` or `zh-Hant-TW`,
 * falling back to English when nothing matches.
 *
 * Mirrors `resolveI18nLanguageFromWebLocale` in the desktop frontend, so the same
 * device resolves to the same translation on both — only the tag spelling differs
 * (mobile hyphenates where desktop uses underscores).
 */
internal fun resolveDeviceLanguageTag(deviceTag: String): String {
    val parts = deviceTag.replace('_', '-').split('-').filter { it.isNotEmpty() }
    val language = parts.firstOrNull()?.lowercase() ?: return "en"
    return when (language) {
        // Script wins when stated; otherwise the region decides which Chinese.
        "zh" -> {
            val region = parts.getOrNull(1)?.uppercase()
            when {
                parts.any { it.equals("Hans", ignoreCase = true) } -> "zh-Hans"
                parts.any { it.equals("Hant", ignoreCase = true) } -> "zh-Hant"
                region in setOf("TW", "HK", "MO", "CHT") -> "zh-Hant"
                else -> "zh-Hans"
            }
        }

        "pt" -> {
            if (parts.getOrNull(1)?.uppercase() == "BR") "pt-BR" else "pt"
        }

        else -> {
            language.takeIf { it in supportedAppLanguageTags } ?: "en"
        }
    }
}

internal fun savePendingOAuthFlow(
    prefs: AppPreferences,
    flow: PendingOAuthFlow,
) {
    prefs.putString(OAUTH_PENDING_PROVIDER_PREF, flow.provider)
    prefs.putString(OAUTH_PENDING_STATE_PREF, flow.state)
    prefs.putString(OAUTH_PENDING_VERIFIER_PREF, flow.verifier)
    prefs.putString(OAUTH_PENDING_REDIRECT_URI_PREF, flow.redirectUri)
    prefs.putString(OAUTH_PENDING_EMAIL_PREF, flow.email)
}

internal fun loadPendingOAuthFlow(prefs: AppPreferences): PendingOAuthFlow? {
    val provider = prefs.getString(OAUTH_PENDING_PROVIDER_PREF, "")
    val state = prefs.getString(OAUTH_PENDING_STATE_PREF, "")
    val verifier = prefs.getString(OAUTH_PENDING_VERIFIER_PREF, "")
    val redirectUri = prefs.getString(OAUTH_PENDING_REDIRECT_URI_PREF, "")
    val email = prefs.getString(OAUTH_PENDING_EMAIL_PREF, "")
    if (provider.isBlank() || state.isBlank() || verifier.isBlank() || redirectUri.isBlank()) return null
    return PendingOAuthFlow(provider, state, verifier, redirectUri, email)
}

internal fun clearPendingOAuthFlow(prefs: AppPreferences) {
    prefs.remove(OAUTH_PENDING_PROVIDER_PREF)
    prefs.remove(OAUTH_PENDING_STATE_PREF)
    prefs.remove(OAUTH_PENDING_VERIFIER_PREF)
    prefs.remove(OAUTH_PENDING_REDIRECT_URI_PREF)
    prefs.remove(OAUTH_PENDING_EMAIL_PREF)
}
