package jp.nonbili.meron.ui

// Where mobile settings actually live.
//
// The core `settings` table is authoritative, the same as on desktop. The
// platform store (SharedPreferences / NSUserDefaults) sits in front of it as a
// write-through cache, because two things need these values before the core can
// answer:
//
//   * the first frame. The store is SQLCipher-keyed and the key comes from the
//     Keystore/Keychain, which is the slowest step of startup; theme and text
//     size have to be applied before that finishes.
//   * platform code that never talks to the core at all — the iOS
//     BGTaskScheduler reads the poll interval straight out of UserDefaults to
//     schedule a refresh while the app is not running.
//
// So reads at startup come from the cache (synchronous, always available) and
// writes go to both. Once the core is up, [hydrateSettingsFromCore] reconciles:
// the table wins, which is what makes a restored backup reach these settings
// with no separate channel.
//
// The registry below is deliberately curated. Session and in-flight state (last
// opened folder, pending OAuth handshakes, kanban search text) stays cache-only:
// it is per-device by nature and would be wrong to sync or restore.

/** Which platform-store namespace a setting lives in. */
internal enum class PrefStore(
    val prefix: String,
) {
    App("app"),
    Kanban("kanban"),
}

internal enum class PrefType { Str, Bool, Int, StrSet }

internal data class MobileSetting(
    val store: PrefStore,
    val key: String,
    val type: PrefType,
) {
    /**
     * Row key in the core `settings` table.
     *
     * Namespaced under `mobile.` so these never collide with the desktop keys
     * sharing the table, and carrying the store name so the two platform
     * namespaces stay distinguishable.
     */
    val settingKey: String get() = "mobile.${store.prefix}.$key"
}

internal val mobileSettings =
    listOf(
        // Appearance and language.
        MobileSetting(PrefStore.App, APPEARANCE_MODE_PREF, PrefType.Str),
        MobileSetting(PrefStore.App, APP_LANGUAGE_PREF, PrefType.Str),
        MobileSetting(PrefStore.App, MESSAGE_FONT_SCALE_PREF, PrefType.Int),
        MobileSetting(PrefStore.App, SHOW_SENDER_IMAGES_PREF, PrefType.Bool),
        MobileSetting(PrefStore.App, SHOW_UNREAD_BADGES_PREF, PrefType.Bool),
        // Layout and navigation.
        MobileSetting(PrefStore.App, SHOW_UNIFIED_INBOX_PREF, PrefType.Bool),
        MobileSetting(PrefStore.App, CONVERSATION_LAYOUT_PREF, PrefType.Str),
        MobileSetting(PrefStore.App, SEND_SHORTCUT_PREF, PrefType.Str),
        MobileSetting(PrefStore.App, HIDDEN_NAV_ACCOUNTS_PREF, PrefType.StrSet),
        MobileSetting(PrefStore.App, KANBAN_COLUMN_WIDTH_PREF, PrefType.Int),
        // Sync and notifications.
        MobileSetting(PrefStore.App, LIVE_MAIL_PUSH_PREF, PrefType.Bool),
        MobileSetting(PrefStore.App, BACKGROUND_SYNC_ENABLED_PREF, PrefType.Bool),
        MobileSetting(PrefStore.App, POLL_INTERVAL_MINUTES_PREF, PrefType.Int),
        // Kanban boards, in their own store.
        MobileSetting(PrefStore.Kanban, KANBAN_BOARDS_PREF, PrefType.Str),
        MobileSetting(PrefStore.Kanban, ACTIVE_KANBAN_BOARD_PREF, PrefType.Str),
    )

/**
 * Platform-store key holding the settings the table is behind on, so that
 * survives the process (see [SettingsMirror]).
 *
 * Deliberately *not* a registry entry: it is bookkeeping about the mirror, not a
 * user setting, so it is never mirrored into the table nor carried in a backup.
 */
internal const val PENDING_SETTINGS_PREF = "pending_settings_v1"

/**
 * Core `settings` row holding the app-wide signature.
 *
 * Deliberately *not* a registry entry and *not* namespaced under `mobile.`: it
 * is the same row the desktop writes, so the two agree after a backup restore,
 * and nothing needs it before the core is up (only the composer reads it).
 */
internal const val APP_SIGNATURE_SETTING_KEY = "signature"

/**
 * Core `settings` row holding the app-wide remote-content sender allowlist.
 *
 * Shared with desktop and read by the core itself (it resolves the allowlist
 * when it bakes a message body), so it is neither namespaced under `mobile.`
 * nor a registry entry: nothing needs it before the core is up.
 */
internal const val REMOTE_IMAGE_SENDERS_SETTING_KEY = "remote_image_senders"

private val settingsByPrefKey = mobileSettings.associateBy { it.store to it.key }
private val settingsBySettingKey = mobileSettings.associateBy { it.settingKey }

/** Every core key the host hydrates, for one `app.prefsGet`. */
internal val mobileSettingKeys: List<String> = mobileSettings.map { it.settingKey }

internal fun mobileSettingFor(
    store: PrefStore,
    key: String,
): MobileSetting? = settingsByPrefKey[store to key]

/** The registry entry for a core `settings` row key, if it is one of ours. */
internal fun mobileSettingForSettingKey(settingKey: String): MobileSetting? = settingsBySettingKey[settingKey]

// Two arbitrary, distinct probe defaults. `AppPreferences` has no "contains"
// call, so a key is read twice, asking for a different default each time: the
// answers agree only when a real value is stored, and differ when both reads
// fell through to the default. That separates "unset" from "set to the type
// default" without reserving a sentinel the user might legitimately hold —
// treating an explicit `false` as "never touched" would silently switch
// settings back on.
private const val PROBE_A = "meron-probe-a"
private const val PROBE_B = "meron-probe-b"

/**
 * Read a setting from the platform cache, or `null` if the user has never set
 * it. Used to seed the core table on first run and to answer "is this cached?".
 */
internal fun readCachedSetting(
    prefs: AppPreferences,
    setting: MobileSetting,
): Any? =
    when (setting.type) {
        PrefType.Str -> {
            val value = prefs.getString(setting.key, PROBE_A)
            value.takeIf { it == prefs.getString(setting.key, PROBE_B) }
        }

        PrefType.StrSet -> {
            val value = prefs.getStringSet(setting.key, setOf(PROBE_A))
            value.takeIf { it == prefs.getStringSet(setting.key, setOf(PROBE_B)) }?.toList()
        }

        PrefType.Bool -> {
            val value = prefs.getBoolean(setting.key, false)
            value.takeIf { it == prefs.getBoolean(setting.key, true) }
        }

        PrefType.Int -> {
            val value = prefs.getInt(setting.key, Int.MIN_VALUE)
            value.takeIf { it == prefs.getInt(setting.key, Int.MAX_VALUE) }
        }
    }

/** Every cached setting the user has actually set, keyed by core setting key. */
internal fun collectCachedSettings(
    app: AppPreferences,
    kanban: AppPreferences,
): Map<String, Any> {
    val out = LinkedHashMap<String, Any>()
    for (setting in mobileSettings) {
        val prefs = if (setting.store == PrefStore.Kanban) kanban else app
        readCachedSetting(prefs, setting)?.let { out[setting.settingKey] = it }
    }
    return out
}

/**
 * Write settings from the core table into the platform cache, skipping keys the
 * cache should keep (see `skip`) and anything whose type does not match — a
 * newer build's key, or a value the pref store cannot hold.
 *
 * Returns the keys actually written, so the caller can re-seed the matching UI
 * state without re-reading everything.
 */
internal fun writeSettingsToCache(
    app: AppPreferences,
    kanban: AppPreferences,
    values: Map<String, Any?>,
    skip: Set<String> = emptySet(),
): Map<String, Any> {
    val written = LinkedHashMap<String, Any>()
    for ((settingKey, value) in values) {
        if (settingKey in skip || value == null) continue
        val setting = settingsBySettingKey[settingKey] ?: continue
        val prefs = if (setting.store == PrefStore.Kanban) kanban else app
        val stored: Any? =
            when (setting.type) {
                PrefType.Str -> {
                    (value as? String)?.also { prefs.putString(setting.key, it) }
                }

                PrefType.Bool -> {
                    (value as? Boolean)?.also { prefs.putBoolean(setting.key, it) }
                }

                // JSON numbers may arrive as any numeric type.
                PrefType.Int -> {
                    (value as? Number)?.toInt()?.also { prefs.putInt(setting.key, it) }
                }

                PrefType.StrSet -> {
                    (value as? Collection<*>)
                        ?.filterIsInstance<String>()
                        ?.also { prefs.putStringSet(setting.key, it.toSet()) }
                }
            }
        if (stored != null) written[settingKey] = stored
    }
    return written
}
