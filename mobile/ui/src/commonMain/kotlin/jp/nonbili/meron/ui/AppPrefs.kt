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

internal fun loadAppearanceMode(prefs: AppPreferences): AppAppearanceMode {
    val stored = prefs.getString(APPEARANCE_MODE_PREF, AppAppearanceMode.Indigo.storageValue)
    return AppAppearanceMode.entries.firstOrNull { it.storageValue == stored && it != AppAppearanceMode.System }
        ?: AppAppearanceMode.Indigo
}

internal fun saveAppearanceMode(
    prefs: AppPreferences,
    mode: AppAppearanceMode,
) = prefs.putString(APPEARANCE_MODE_PREF, mode.storageValue)

internal fun loadSendShortcutMode(prefs: AppPreferences): SendShortcutMode =
    when (prefs.getString(SEND_SHORTCUT_PREF, "mod_enter")) {
        "enter" -> SendShortcutMode.Enter
        else -> SendShortcutMode.ModEnter
    }

internal fun saveSendShortcutMode(
    prefs: AppPreferences,
    mode: SendShortcutMode,
) = prefs.putString(SEND_SHORTCUT_PREF, mode.storageValue())

internal fun loadAppLanguageTag(prefs: AppPreferences): String =
    prefs
        .getString(APP_LANGUAGE_PREF, "")
        .takeIf { it in supportedAppLanguageTags }
        .orEmpty()

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
