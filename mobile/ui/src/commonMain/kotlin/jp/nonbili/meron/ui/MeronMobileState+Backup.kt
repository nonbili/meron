package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.ExportBackupParams
import jp.nonbili.meron.shared.ImportBackupParams
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.coercePollIntervalMinutes
import jp.nonbili.meron.shared.parseBackupExportResponse
import jp.nonbili.meron.shared.parseBackupImportResponse
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// Backup / restore of the app's configuration: accounts and their connection
// settings, per-account prefs, RSS subscriptions and the settings table. Cached
// mail is not included — it re-syncs from the server — so the file stays small
// and moves between phone and desktop.
//
// The passphrase flow has two entry points because encryption is discovered at
// different times on each side: exporting asks up front (the user chooses),
// restoring asks only once the chosen file turns out to be encrypted.

/** Which half of the flow the passphrase sheet is collecting for. */
internal enum class BackupPassphraseMode { Export, Restore }

/**
 * Catalog lookup outside a composition. `tr` needs `LocalAppLocale`, which these
 * state functions have no access to, so resolve the tag from prefs the same way
 * the root composable does.
 */
internal fun MeronMobileState.trs(
    key: String,
    args: Map<String, Any?> = emptyMap(),
): String = localizedString(loadAppLanguageTag(prefs).ifBlank { "en" }, key, args)

/**
 * Serialize a backup and hand it to the platform save-file picker.
 *
 * `passphrase` encrypts the document; `includeSecrets` additionally embeds
 * account passwords and OAuth tokens, which the core refuses without one.
 */
internal fun MeronMobileState.exportBackup(
    includeSecrets: Boolean,
    passphrase: String,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    backupBusy = true
    scope.launch {
        // Settings mirror asynchronously, so a preference changed moments ago (or
        // one whose write failed while offline) may still be ahead of the table.
        // Flush first or the backup captures the previous value — and if the
        // flush cannot drain (a read-only or full database), fail rather than
        // hand back a file that quietly holds the old settings.
        if (!settingsMirror.flush()) {
            status = trs("settings.backup.exportFailed")
            backupBusy = false
            return@launch
        }
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).exportBackup(
                    ExportBackupParams(
                        includeSecrets = includeSecrets,
                        passphrase = passphrase,
                        appVersion = mobileHost.appVersionName,
                        platform = platformName,
                    ),
                )
            }
        }.onSuccess { response ->
            val document = parseBackupExportResponse(response)
            if (document.isBlank()) {
                status = trs("settings.backup.exportFailed")
            } else {
                backupPassphraseMode = null
                pendingBackupExport = document
                launchBackupExport(backupFileName())
                status = trs("settings.backup.exported")
            }
        }.onFailure {
            status = "${trs("settings.backup.exportFailed")}: ${it.message}"
        }
        backupBusy = false
    }
}

/**
 * Restore from a file the user picked.
 *
 * An encrypted file comes back as `needsPassphrase` rather than an error: the
 * document is kept in [MeronMobileState.pendingBackupRestore] so the retry
 * decrypts it without making the user find the file again.
 */
internal fun MeronMobileState.importBackup(
    document: String,
    passphrase: String = "",
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    if (document.isBlank()) {
        status = trs("settings.backup.restoreFailed")
        return
    }
    val generation = ++backupRestoreGeneration
    backupBusy = true
    scope.launch {
        // Wait out an in-flight write that would otherwise land on top of the
        // restored rows, but keep the queue: this call may turn out to be a
        // passphrase probe, a wrong passphrase, or an unreadable file, none of
        // which touch the table. The force-hydrate below drops the queue once a
        // restore has actually happened.
        settingsMirror.awaitIdle()
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).importBackup(
                    ImportBackupParams(backup = document, passphrase = passphrase),
                )
            }
        }.onSuccess { response ->
            if (generation != backupRestoreGeneration) return@onSuccess
            val result = parseBackupImportResponse(response)
            if (result.needsPassphrase) {
                // Keep the document so the retry doesn't re-open the picker.
                pendingBackupRestore = document
                backupPassphraseError = ""
                backupPassphraseMode = BackupPassphraseMode.Restore
            } else {
                // Reject any reads launched before this restore. Hydration below
                // suspends, so invalidating first prevents an old response from
                // briefly repainting pre-restore accounts/proxy/signature.
                invalidateBackupReloads()
                closeBackupPassphrase()
                // Preferences the core carried but cannot write: appearance,
                // language and the rest are ours to put back.
                // Mobile settings are rows in the same table, so the restore has
                // already written them; hydrating pulls them into the cache and
                // into this state.
                // force: the restore has just overwritten the table wholesale, so
                // it wins even over settings the user edited earlier this session
                // (which a normal hydrate would protect).
                val rehydrated =
                    hydrateSettingsFromCore(
                        prefs.cacheStore(),
                        kanbanPrefs.cacheStore(),
                        settingsMirror,
                        force = true,
                    )
                applyHydratedSettings(rehydrated)
                // On Android 13+ the OS owns the per-app language, and the initial
                // hydrate treats it as authoritative — so a restored language only
                // sticks (and only reaches Android's own resource strings, such as
                // notification text) once it is pushed back to the platform.
                if (settingKeyFor(APP_LANGUAGE_PREF) in rehydrated) {
                    locale.applySystem(loadAppLanguageTag(prefs))
                }
                // Everything hydrated is applied live except appearance and
                // language: the host holds those as `remember` values in the
                // Activity / view controller, so only they need a restart, and
                // only then is it worth saying so.
                val needsRestart =
                    rehydrated.keys.any {
                        it == settingKeyFor(APPEARANCE_MODE_PREF) || it == settingKeyFor(APP_LANGUAGE_PREF)
                    }
                status =
                    when {
                        needsRestart -> {
                            trs("settings.backup.restoredRestart")
                        }

                        result.accounts == 0 && result.skipped > 0 -> {
                            trs("settings.backup.restoredNothingNew")
                        }

                        else -> {
                            trs("settings.backup.restored", mapOf("count" to result.accounts))
                        }
                    }
                // Restored rows are in the store but not in this state: reload
                // accounts (which re-seeds selection, folders and boards) and
                // the app-wide proxy, which the socket layer reads separately.
                awaitBackupReloads(generation)
            }
        }.onFailure {
            if (generation != backupRestoreGeneration) return@onFailure
            val message = it.message.orEmpty()
            // A wrong passphrase keeps the sheet open for a retype; anything
            // else is a real failure and closes it.
            if (message.contains("wrong passphrase") && backupPassphraseMode == BackupPassphraseMode.Restore) {
                backupPassphraseError = trs("settings.backup.wrongPassphrase")
            } else {
                closeBackupPassphrase()
                status = "${trs("settings.backup.restoreFailed")}: $message"
            }
        }
        if (generation == backupRestoreGeneration) backupBusy = false
    }
}

private suspend fun MeronMobileState.awaitBackupReloads(restoreGeneration: Int) {
    repeat(2) {
        val accountJob = listAccounts()
        val accountGeneration = accountLoadGeneration
        val proxyJob = loadAppProxy()
        val proxyGeneration = proxyLoadGeneration
        val signatureJob = loadAppSignature()
        val signatureGeneration = appSignatureLoadGeneration
        val remoteSendersJob = loadRemoteImageSenders()
        listOfNotNull(accountJob, proxyJob, signatureJob, remoteSendersJob).joinAll()
        val reloadSuperseded =
            accountGeneration != accountLoadGeneration ||
                proxyGeneration != proxyLoadGeneration ||
                signatureGeneration != appSignatureLoadGeneration
        if (restoreGeneration != backupRestoreGeneration || !reloadSuperseded) return
    }
}

/** Restore using the passphrase just typed into the sheet. */
internal fun MeronMobileState.retryBackupRestore(passphrase: String) {
    importBackup(pendingBackupRestore, passphrase)
}

/** Dismiss the passphrase sheet and drop whatever it was working on. */
internal fun MeronMobileState.closeBackupPassphrase() {
    backupPassphraseMode = null
    backupPassphraseError = ""
    pendingBackupRestore = ""
}

/**
 * Name offered to the save-file picker. Dated so successive backups don't
 * silently overwrite each other, and `.json` because the file is plain JSON
 * whose envelope stays readable even when the payload is encrypted.
 */
internal fun backupFileName(nowMillis: Long = currentTimeMillis()): String = "meron-backup-${isoDate(nowMillis)}.json"

/**
 * `YYYY-MM-DD` in UTC for an epoch-millis instant.
 *
 * Written out rather than delegated to a platform formatter because this only
 * ever labels a filename: a fixed, locale-independent form is what's wanted,
 * and it keeps the helper in commonMain and directly testable.
 */
internal fun isoDate(nowMillis: Long): String {
    // Civil-from-days (Howard Hinnant's algorithm), shifting the era to
    // 0000-03-01 so leap days land at the end of a 400-year cycle.
    val days = floorDiv(nowMillis, 86_400_000L)
    val z = days + 719_468L
    val era = floorDiv(z, 146_097L)
    val dayOfEra = z - era * 146_097L
    val yearOfEra = (dayOfEra - dayOfEra / 1460 + dayOfEra / 36_524 - dayOfEra / 146_096) / 365
    val year = yearOfEra + era * 400
    val dayOfYear = dayOfEra - (365 * yearOfEra + yearOfEra / 4 - yearOfEra / 100)
    val shiftedMonth = (5 * dayOfYear + 2) / 153
    val day = dayOfYear - (153 * shiftedMonth + 2) / 5 + 1
    val month = if (shiftedMonth < 10) shiftedMonth + 3 else shiftedMonth - 9
    val calendarYear = if (month <= 2) year + 1 else year
    return "$calendarYear-${pad2(month)}-${pad2(day)}"
}

private fun floorDiv(
    value: Long,
    divisor: Long,
): Long {
    val quotient = value / divisor
    return if (value % divisor != 0L && (value xor divisor) < 0) quotient - 1 else quotient
}

private fun pad2(value: Long): String = if (value < 10) "0$value" else "$value"

/**
 * Re-seed the settings this state holds after [hydrateSettingsFromCore] found
 * the authoritative table disagreeing with the platform cache — most often
 * because a backup was just restored.
 *
 * The cache has already been updated, so each field is simply re-read through
 * the same loader that seeded it at construction; that keeps coercion and
 * default handling in one place instead of duplicating it per key.
 *
 * Appearance and language are not here: the host owns those (they are `remember`
 * values in the Activity / view controller), so they apply on the next launch.
 */
internal fun MeronMobileState.applyHydratedSettings(changed: Map<String, Any>) {
    for (settingKey in changed.keys) {
        when (settingKey) {
            settingKeyFor(SHOW_UNREAD_BADGES_PREF) -> {
                showUnreadBadges = loadAppBoolean(prefs, SHOW_UNREAD_BADGES_PREF, true)
            }

            settingKeyFor(SHOW_UNIFIED_INBOX_PREF) -> {
                showUnifiedInboxNav = loadAppBoolean(prefs, SHOW_UNIFIED_INBOX_PREF, true)
            }

            settingKeyFor(SHOW_SENDER_IMAGES_PREF) -> {
                showSenderImages = loadAppBoolean(prefs, SHOW_SENDER_IMAGES_PREF, false)
            }

            settingKeyFor(LIVE_MAIL_PUSH_PREF) -> {
                liveMailPushEnabled = loadAppBoolean(prefs, LIVE_MAIL_PUSH_PREF, false)
                mobileHost.syncLiveMailPush(liveMailPushEnabled)
            }

            settingKeyFor(BACKGROUND_SYNC_ENABLED_PREF) -> {
                backgroundSyncEnabled = loadAppBoolean(prefs, BACKGROUND_SYNC_ENABLED_PREF, true)
                mobileHost.syncBackgroundRefresh(backgroundSyncEnabled)
            }

            settingKeyFor(POLL_INTERVAL_MINUTES_PREF) -> {
                pollIntervalMinutes =
                    coercePollIntervalMinutes(loadAppInt(prefs, POLL_INTERVAL_MINUTES_PREF, 15))
            }

            settingKeyFor(SEND_SHORTCUT_PREF) -> {
                sendShortcutMode = loadSendShortcutMode(prefs)
            }

            settingKeyFor(CONVERSATION_LAYOUT_PREF) -> {
                conversationLayout = loadConversationLayout(prefs)
            }

            settingKeyFor(MESSAGE_FONT_SCALE_PREF) -> {
                messageFontScale = loadMessageFontScale(prefs)
            }

            settingKeyFor(KANBAN_COLUMN_WIDTH_PREF) -> {
                kanbanColumnWidth =
                    loadAppInt(prefs, KANBAN_COLUMN_WIDTH_PREF, KANBAN_COLUMN_DEFAULT_WIDTH)
                        .coerceIn(KANBAN_COLUMN_MIN_WIDTH, KANBAN_COLUMN_MAX_WIDTH)
            }

            settingKeyFor(HIDDEN_NAV_ACCOUNTS_PREF) -> {
                hiddenNavigationAccountIds = loadAppStringSet(prefs, HIDDEN_NAV_ACCOUNTS_PREF)
            }

            settingKeyFor(KANBAN_BOARDS_PREF, PrefStore.Kanban) -> {
                kanbanBoards = parseKanbanBoards(kanbanPrefs.getString(KANBAN_BOARDS_PREF, ""))
            }

            settingKeyFor(ACTIVE_KANBAN_BOARD_PREF, PrefStore.Kanban) -> {
                activeKanbanBoardId = loadActiveKanbanBoardId(kanbanPrefs)
            }
        }
    }
}

private fun settingKeyFor(
    key: String,
    store: PrefStore = PrefStore.App,
): String = "mobile.${store.prefix}.$key"
