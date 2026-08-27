package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.ListItem
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.navigation.NavBackStackEntry
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import jp.nonbili.meron.shared.AccountAliasParams
import jp.nonbili.meron.shared.AccountAliasesParams
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.nextPollIntervalMinutes
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import kotlin.math.abs

internal const val MAILBOX_BLOCKING_WARN_AFTER_MS = 10_000L
internal const val MAILBOX_BLOCKING_TIMEOUT_MS = 15_000L

// Hard cap for a blocking load whose sync is still in flight: past the soft
// timeout the loader shows "still syncing" copy, past this one we give up and
// surface the timeout error.
internal const val MAILBOX_BLOCKING_HARD_TIMEOUT_MS = 60_000L

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MeronApp(
    core: MeronCore,
    coreLoaded: Boolean,
    prefs: AppPreferences,
    kanbanPrefs: AppPreferences,
    services: PlatformServices,
    locale: LocaleController,
    mobileHost: MobileHost = DefaultMobileHost(),
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft? = null,
    onMailtoDraftConsumed: () -> Unit = {},
    incomingOAuthCallbackUrl: String? = null,
    incomingNotificationThreadTarget: NotificationThreadTarget? = null,
    /**
     * Notified after the language has been stored and pushed to the OS, for hosts
     * that must react — Android recreates its Activity to reload resources.
     * Persistence is not the host's job: doing it there would bypass the
     * write-through store and never reach the `settings` table.
     */
    onLanguageApplied: (String) -> Unit = {},
) {
    val scope = rememberCoroutineScope()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    // The core `settings` table is authoritative for these; the platform stores
    // stay in front of it as a write-through cache so the first frame can paint
    // before the keyed DB is open. See MobileSettings.kt.
    val coreLoadedNow = rememberUpdatedState(coreLoaded)
    val mirror = remember(core, prefs) { SettingsMirror(core, journal = prefs) { coreLoadedNow.value } }
    val backedPrefs =
        remember(prefs, mirror) { CoreBackedPreferences(prefs, PrefStore.App, scope, mirror) }
    val backedKanbanPrefs =
        remember(kanbanPrefs, mirror) {
            CoreBackedPreferences(kanbanPrefs, PrefStore.Kanban, scope, mirror)
        }
    val state =
        remember(core, backedPrefs, backedKanbanPrefs, services, locale, mobileHost) {
            MeronMobileState(
                scope = scope,
                core = core,
                coreLoaded = coreLoaded,
                prefs = backedPrefs,
                kanbanPrefs = backedKanbanPrefs,
                services = services,
                locale = locale,
                mobileHost = mobileHost,
                settingsMirror = mirror,
            )
        }
    // Appearance and language live here rather than in the hosts, so their writes
    // go through `backedPrefs` like every other setting. `putString` stages
    // synchronously, which is what makes them survive the Activity recreation the
    // language change triggers.
    var appearanceMode by remember(backedPrefs) { mutableStateOf(loadAppearanceMode(backedPrefs)) }
    // Android 13+ lets the language be changed from system settings while the app
    // is not running; that choice outranks the stored tag.
    val systemLanguageTag = remember(locale) { locale.systemLanguageTag() }
    var appLanguageTag by
        remember(backedPrefs, systemLanguageTag) {
            mutableStateOf(resolveAppLanguageTag(systemLanguageTag, loadAppLanguageTag(backedPrefs)))
        }
    LaunchedEffect(systemLanguageTag) {
        // Persisted as an effect rather than during composition, and through the
        // write-through store, so an externally chosen language reaches the table
        // instead of being read back as stale on the next hydrate. A reset to
        // "system default" persists too — it stores blank, clearing the old tag.
        if (appLanguageNeedsPersisting(systemLanguageTag, loadAppLanguageTag(backedPrefs))) {
            saveAppLanguageTag(backedPrefs, systemLanguageTag.orEmpty())
        }
    }
    val deviceLanguageTag = remember(locale) { resolveDeviceLanguageTag(locale.deviceLanguageTag()) }
    val onAppearanceModeChange: (AppAppearanceMode) -> Unit = { mode ->
        appearanceMode = mode
        saveAppearanceMode(backedPrefs, mode)
    }
    val onAppLanguageChange: (String) -> Unit = { tag ->
        appLanguageTag = tag
        saveAppLanguageTag(backedPrefs, tag)
        locale.applySystem(tag)
        onLanguageApplied(tag)
    }

    // Reconcile once the core is up: the table wins, which is what lets a
    // restored backup reach these settings without a separate channel.
    LaunchedEffect(coreLoaded) {
        if (!coreLoaded) return@LaunchedEffect
        val changed = hydrateSettingsFromCore(prefs, kanbanPrefs, mirror)
        if (changed.isNotEmpty()) state.applyHydratedSettings(changed)
    }
    // Appearance and language are persisted by the host (and, for language, by
    // LocaleController, which normalizes the tag first), so the write-through
    // decorator never sees those writes — yet both are registry settings. Mirror
    // the cache after the host has stored it, or the table keeps the old value
    // and the next hydrate pulls it back over the user's choice.
    androidx.compose.runtime.CompositionLocalProvider(
        // Blank means "follow the system", so fall back to the device's language
        // rather than to English.
        LocalAppLocale provides appLanguageTag.ifBlank { deviceLanguageTag },
        LocalPlatformServices provides services,
    ) {
        MeronTheme(appearanceMode = appearanceMode, messageFontScale = state.messageFontScale) {
            MeronMobileScreenContent(
                state = state,
                drawerState = drawerState,
                coreInitJson = coreInitJson,
                incomingMailtoDraft = incomingMailtoDraft,
                onMailtoDraftConsumed = onMailtoDraftConsumed,
                incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
                incomingNotificationThreadTarget = incomingNotificationThreadTarget,
                appearanceMode = appearanceMode,
                onAppearanceModeChange = onAppearanceModeChange,
                appLanguageTag = appLanguageTag,
                onAppLanguageChange = onAppLanguageChange,
                packageName = mobileHost.packageName,
                appVersion = mobileHost.appVersionName,
                coreProtocolVersion = mobileHost.coreProtocolVersion,
            )
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
private fun PickedFile.toDraftAttachment(): DraftAttachment =
    DraftAttachment(
        id = name,
        displayName = name.ifBlank { "attachment" },
        mimeType = mimeType.ifBlank { "application/octet-stream" },
        sizeBytes = bytes.size.toLong(),
        dataBase64 = Base64.Default.encode(bytes),
    )

// Mutating the back stack while a NavHost transition is still running can make
// AnimatedContent recompose an already-destroyed NavBackStackEntry, crashing in
// its ViewModelStore. Only pop/navigate once the current entry has settled.
internal fun canMutateBackStack(entryState: Lifecycle.State?): Boolean = entryState == null || entryState.isAtLeast(Lifecycle.State.RESUMED)

// Waits for the entry to leave its transition; returns false if it was
// destroyed instead (the back stack changed, so the caller's navigation
// request is stale and must be dropped).
private suspend fun NavBackStackEntry.awaitSettled(): Boolean {
    val state =
        lifecycle.currentStateFlow.first {
            it == Lifecycle.State.DESTROYED || it.isAtLeast(Lifecycle.State.RESUMED)
        }
    return state != Lifecycle.State.DESTROYED
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
private fun MeronMobileScreenContent(
    state: MeronMobileState,
    drawerState: androidx.compose.material3.DrawerState,
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft?,
    onMailtoDraftConsumed: () -> Unit,
    incomingOAuthCallbackUrl: String?,
    incomingNotificationThreadTarget: NotificationThreadTarget?,
    appearanceMode: AppAppearanceMode,
    onAppearanceModeChange: (AppAppearanceMode) -> Unit,
    appLanguageTag: String,
    onAppLanguageChange: (String) -> Unit,
    packageName: String,
    appVersion: String,
    coreProtocolVersion: Int,
) {
    with(state) {
        val navController = rememberNavController()
        val currentBackStackEntry by navController.currentBackStackEntryAsState()
        val startRoute = remember { screen.route() }
        var pendingRoute by remember { mutableStateOf<String?>(null) }
        LaunchedEffect(screen) {
            val targetRoute = screen.route()
            val currentEntry = currentBackStackEntry
            val currentRoute = currentEntry?.destination?.route
            if (currentRoute == null) {
                pendingRoute = targetRoute
            } else if (currentRoute != targetRoute) {
                if (!currentEntry.awaitSettled()) return@LaunchedEffect
                if (!navController.popBackStack(targetRoute, inclusive = false)) {
                    navController.navigate(targetRoute) {
                        launchSingleTop = true
                    }
                }
            }
        }
        LaunchedEffect(currentBackStackEntry?.destination?.route) {
            val currentEntry = currentBackStackEntry
            val currentRoute = currentEntry?.destination?.route
            val queuedRoute = pendingRoute
            if (queuedRoute != null && currentRoute != null && currentRoute != queuedRoute) {
                pendingRoute = null
                if (!currentEntry.awaitSettled()) return@LaunchedEffect
                navController.navigate(queuedRoute) {
                    launchSingleTop = true
                }
                return@LaunchedEffect
            }
            pendingRoute = null
            val destinationScreen = appRouteToScreen(currentRoute)
            if (destinationScreen != null && screen != destinationScreen) {
                screen = destinationScreen
            }
        }
        val popAppBack: () -> Unit = {
            if (canMutateBackStack(navController.currentBackStackEntry?.lifecycle?.currentState)) {
                if (screen == Screen.Thread) {
                    flushQuickReplyAutosave()
                }
                if (!navController.popBackStack()) {
                    screen = previousTopScreen
                }
            }
        }
        DisposableEffect(Unit) {
            val handle =
                if (coreLoaded) {
                    core.events().subscribe { event ->
                        when (event.name) {
                            "mail.newMessages" -> {
                                if (!liveMailPushEnabled) {
                                    mobileHost.notifyNewMail(event.detailJson)
                                }
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, eventFolder)
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            "mail.synced" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, eventFolder)
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // A mailbox changed outside the UI — a notification's
                            // Archive or Mark as read. The mail is already gone
                            // from the server, so the open list is stale in
                            // exactly the way a sync would have corrected.
                            "mail.mailboxChanged" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, eventFolder)
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // A Sent copy the provider filed seconds after SMTP
                            // returned, too late for the reload the send itself
                            // ran. It adds a message to its conversation, so the
                            // row's count (which spans folders) is one short. It
                            // names no folder: the row sits in whichever mailbox
                            // holds the conversation, so every column of the
                            // account re-reads.
                            "mail.sentCopyCached" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                scope.launch {
                                    reloadVisibleMailboxFor(eventAccount, "")
                                    refreshKanbanColumnsForAccount(eventAccount)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            // The deferred sync tail (body prefetch, Sent/Drafts
                            // headers) never changes the open folder's thread
                            // list, so only the open thread needs a refresh —
                            // reloading the list here would reset pagination.
                            "mail.tailSynced" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                scope.launch {
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            "error" -> {
                                // Watcher/runtime failures are regular core events,
                                // not `log` events. Keep them visible in Logcat and
                                // the redacted on-device diagnostic log.
                                Log.e("core/event", event.detailJson.jsonStringValue("message"))
                            }

                            "log" -> {
                                // Surface Rust core logs through the platform logger
                                // (os_log / Logcat); they'd otherwise be invisible
                                // on device.
                                val tag = "core/" + event.detailJson.jsonStringValue("tag")
                                val message = event.detailJson.jsonStringValue("message")
                                when (event.detailJson.jsonStringValue("level")) {
                                    "DEBUG" -> Log.d(tag, message)
                                    "INFO" -> Log.i(tag, message)
                                    "WARN" -> Log.w(tag, message)
                                    else -> Log.e(tag, message)
                                }
                                // A core panic aborts the process right after
                                // this line, so mark it here; the JVM crash
                                // handler never runs for a native abort.
                                if (tag == "core/panic") {
                                    mobileHost.noteCoreCrash(message)
                                }
                            }
                        }
                    }
                } else {
                    null
                }
            onDispose { handle?.close() }
        }
        LaunchedEffect(liveMailPushEnabled) {
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
        }
        LaunchedEffect(backgroundSyncEnabled) {
            mobileHost.syncBackgroundRefresh(backgroundSyncEnabled)
        }
        // Foreground poll for platforms without a real background channel (iOS):
        // re-sync the visible mailbox on the chosen interval while the app is
        // open. Honors "Off" (interval 0). The timer suspends with the app and
        // resumes on return; the foreground signal below covers the gap after a
        // long suspension.
        if (!mobileHost.supportsBackgroundPush) {
            LaunchedEffect(pollIntervalMinutes, coreLoaded) {
                if (!coreLoaded || pollIntervalMinutes <= 0) return@LaunchedEffect
                while (true) {
                    delay(pollIntervalMinutes * 60_000L)
                    syncCoreThreads(
                        accountOverride = selectedCoreAccountId,
                        folderOverride = selectedCoreFolder,
                        syncFirst = true,
                    )
                }
            }
        }
        // Refresh the visible mailbox whenever the app comes back to the
        // foreground. Something else kept mail arriving while it was away —
        // Android's background worker, iOS's poll — and on Android the activity
        // may have been stopped for all of it, leaving the list showing what was
        // on screen when the user left even though the store has moved on. A
        // store re-read is enough to close that gap, so only platforms that have
        // to fetch for themselves (iOS, and only when its poll is on) go to the
        // network here.
        LaunchedEffect(coreLoaded, pollIntervalMinutes) {
            if (!coreLoaded) return@LaunchedEffect
            AppForegroundSignal.events.collect {
                syncCoreThreads(
                    accountOverride = selectedCoreAccountId,
                    folderOverride = selectedCoreFolder,
                    syncFirst = !mobileHost.supportsBackgroundPush && pollIntervalMinutes > 0,
                )
            }
        }
        LaunchedEffect(Unit) {
            NotificationPermissionSignal.events.collect {
                notificationPermissionGranted = mobileHost.notificationsEnabled()
            }
        }
        val importOpml: (PickedFile?) -> Unit = { picked ->
            if (picked != null) {
                runCatching { picked.bytes.decodeToString() }
                    .onSuccess { opml ->
                        val accountId = selectedCoreAccountId
                        if (accountId == UNIFIED_ACCOUNT_ID || accountId.isBlank()) {
                            status = "Select an RSS account first."
                            return@onSuccess
                        }
                        if (!coreLoaded) {
                            status = coreUnavailableMessage
                            return@onSuccess
                        }
                        scope.launch {
                            runCatching {
                                withContext(ioDispatcher) {
                                    val client = MobileMailCommandClient(core)
                                    val importJson = client.importOpml(ImportOpmlParams(accountId = accountId, opml = opml))
                                    client.syncRss(SyncRssParams(accountId = accountId))
                                    val foldersJson = client.listFolders(FolderListParams(accountId = accountId))
                                    val threadsJson =
                                        client.listThreads(
                                            ThreadListParams(
                                                accountId = accountId,
                                                folderId = INBOX_FOLDER,
                                                query = mailSearch.trim(),
                                                filter = mailFilter.protocolValue(),
                                            ),
                                        )
                                    Triple(importJson, foldersJson, threadsJson)
                                }
                            }.onSuccess { (importJson, foldersJson, threadsJson) ->
                                val imported = parseOpmlImportCountResponse(importJson)
                                coreFolders = parseFolderListResponse(foldersJson)
                                coreThreads = withLocalDraftFlags(parseThreadListResponse(threadsJson))
                                selectedCoreFolder = INBOX_FOLDER
                                visibleMailboxKey =
                                    mailboxCacheKey(accountId, INBOX_FOLDER, mailSearch, mailFilter)
                                mailboxPageDepth = MAILBOX_PAGE_SIZE
                                status = if (imported == 0) "No new feeds imported" else "Imported $imported feed(s)"
                            }.onFailure {
                                status = "OPML import failed: ${it.message}"
                            }
                        }
                    }.onFailure {
                        status = "OPML file read failed: ${it.message}"
                    }
            }
        }
        launchOpmlExport = { fileName ->
            services.saveFile(pendingOpmlExport.encodeToByteArray(), fileName, "text/xml")
            pendingOpmlExport = ""
            status = "Exported OPML"
        }
        launchBackupExport = { fileName ->
            services.saveFile(pendingBackupExport.encodeToByteArray(), fileName, "application/json")
            pendingBackupExport = ""
        }
        // Restore starts at the file picker; the core decides whether what came
        // back needs a passphrase.
        val pickBackupFile: () -> Unit = {
            services.pickFile(listOf("application/json", "text/plain", "*/*")) { picked ->
                if (picked != null) {
                    runCatching { picked.bytes.decodeToString() }
                        .onSuccess { document -> importBackup(document) }
                        .onFailure { status = "${trs("settings.backup.restoreFailed")}: ${it.message}" }
                }
            }
        }
        launchAttachmentSave = { fileName ->
            val attachment = pendingAttachmentSave
            pendingAttachmentSave = null
            if (attachment != null) {
                scope.launch {
                    runCatching {
                        withContext(ioDispatcher) { readAttachmentBytes(attachment) }
                    }.onSuccess { bytes ->
                        services.saveFile(bytes, fileName, attachment.mimeType.ifBlank { "application/octet-stream" })
                        status = "Saved ${attachment.filename.ifBlank { "attachment" }}"
                    }.onFailure {
                        status = "Attachment save failed: ${it.message}"
                    }
                }
            }
        }
        val pickAttachmentInto: ((DraftAttachment) -> Unit) -> Unit = { onPicked ->
            services.pickFile(listOf("*/*")) { picked ->
                if (picked != null) {
                    runCatching { picked.toDraftAttachment() }
                        .onSuccess {
                            onPicked(it)
                            status = "Attached ${it.displayName}"
                        }.onFailure {
                            status = "Attachment failed: ${it.message}"
                        }
                }
            }
        }
        val pickAccountMedia: () -> Unit = {
            val target = accountMediaUploadTarget
            accountMediaUploadTarget = null
            if (target != null) {
                services.pickImage { picked ->
                    if (picked == null) return@pickImage
                    runCatching {
                        AccountMediaFileParams(
                            accountId = target.account.id,
                            filename = picked.name,
                            mime = picked.mimeType.ifBlank { "application/octet-stream" },
                            data = Base64.Default.encode(picked.bytes),
                        )
                    }.onSuccess { params ->
                        if (!coreLoaded) {
                            status = coreUnavailableMessage
                            return@onSuccess
                        }
                        scope.launch {
                            runCatching {
                                withContext(ioDispatcher) {
                                    val client = MobileMailCommandClient(core)
                                    val uploadJson =
                                        if (target.wallpaper) {
                                            client.writeAccountChatWallpaperFile(params)
                                        } else {
                                            client.writeAccountAvatarFile(params)
                                        }
                                    val mediaUrl = parseMediaFileUrlResponse(uploadJson)
                                    if (mediaUrl.isBlank()) error("Media upload returned no URL")
                                    if (target.wallpaper) {
                                        client.setAccountChatWallpaper(AccountChatWallpaperParams(target.account.id, customUrl = mediaUrl))
                                    } else {
                                        client.setAccountAvatar(AccountAvatarParams(target.account.id, mediaUrl))
                                    }
                                    client.listAccounts()
                                }
                            }.onSuccess {
                                accountJson = it
                                coreAccounts = parseAccountListResponse(it)
                                status = if (target.wallpaper) "Updated chat wallpaper" else "Updated avatar"
                            }.onFailure {
                                status = "Media upload failed: ${it.message}"
                            }
                        }
                    }.onFailure {
                        status = "Media read failed: ${it.message}"
                    }
                }
            }
        }
        val pickKanbanBoardMedia: () -> Unit = {
            val target = kanbanBoardMediaTarget
            kanbanBoardMediaTarget = null
            if (target != null) {
                services.pickImage { picked ->
                    if (picked == null) return@pickImage
                    val mediaUrl = "data:${picked.mimeType.ifBlank { "application/octet-stream" }};base64,${Base64.Default.encode(picked.bytes)}"
                    if (target.wallpaper) {
                        updateKanbanBoard(
                            target.board.id,
                            target.board.name,
                            target.board.avatarUrl,
                            "",
                            mediaUrl,
                        )
                        status = "Updated board wallpaper"
                    } else {
                        updateKanbanBoard(
                            target.board.id,
                            target.board.name,
                            mediaUrl,
                            target.board.wallpaperPresetId,
                            target.board.wallpaperUrl,
                        )
                        status = "Updated board avatar"
                    }
                }
            }
        }

        LaunchedEffect(storageClearConfirming) {
            if (storageClearConfirming) {
                delay(4_000)
                storageClearConfirming = false
            }
        }

        LaunchedEffect(status) {
            if (status.isNotBlank()) {
                snackbarHost.showSnackbar(status)
            }
        }

        // A `mailto:` link can arrive before the core has answered with the
        // accounts and the app-wide signature — on a cold start it usually
        // does. Composing then would seed a draft from an empty account list
        // and record that it has no signature, so this waits for both.
        LaunchedEffect(incomingMailtoDraft, initialAccountsLoaded, appSignatureLoaded) {
            if (!initialAccountsLoaded || !appSignatureLoaded) return@LaunchedEffect
            incomingMailtoDraft?.let { draft ->
                openMailtoCompose(draft) {
                    onMailtoDraftConsumed()
                    status = "Loaded compose draft"
                }
            }
        }

        LaunchedEffect(incomingOAuthCallbackUrl) {
            incomingOAuthCallbackUrl?.let { rawUrl ->
                handleOAuthCallback(rawUrl)
            }
        }

        LaunchedEffect(incomingNotificationThreadTarget) {
            incomingNotificationThreadTarget?.let(::openNotificationThread)
        }

        // Load persisted accounts once on startup so they survive app restarts.
        LaunchedEffect(Unit) {
            if (coreLoaded && coreAccounts.isEmpty()) {
                listAccounts()
            } else if (!coreLoaded) {
                initialAccountsLoaded = true
            }
            // The proxy lives in the core store, so it needs a read of its own.
            loadAppProxy()
            loadAppSignature()
        }

        // Once accounts are known, surface whatever the local store already holds so
        // a cold start shows the cached inbox instead of an empty "Nothing here yet".
        // A server sync still happens on pull-to-refresh / "Sync now".
        LaunchedEffect(coreAccounts) {
            if (coreAccounts.isNotEmpty() && coreThreads.isEmpty() && activeMailboxLoadKey == null) {
                syncCoreThreads(syncFirst = false)
            }
        }

        LaunchedEffect(coreAccounts, activeKanbanBoardId) {
            if (coreAccounts.isNotEmpty() && screen == Screen.Kanban) {
                loadKanbanBoard(refresh = false)
            }
        }

        val selectedAccount = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }
        val selectedAccountIsRss = selectedAccount != null && accountSummaryIsRss(selectedAccount)
        val selectedThreadAccount = selectedCoreThread?.accountId?.let { accountId -> coreAccounts.firstOrNull { it.id == accountId } }
        val selectedThreadAccountId = selectedThreadAccount?.id.orEmpty()
        val drawerFolders = foldersByAccount.values.flatten().ifEmpty { coreFolders }
        val selectedThreadPreferHtml =
            selectedThreadAccount?.let { account ->
                conversationHtmlOverrides[account.id] ?: account.conversationHtml
            } ?: true
        val activeKanbanBoard = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: kanbanBoards.firstOrNull()
        val appBarTitle = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) "Unified inbox" else "Inbox"
        val appBarSubtitle =
            if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                "All accounts"
            } else {
                selectedAccount?.email?.ifBlank { selectedAccount.displayName }.orEmpty()
            }
        val selectedMailThreads =
            (coreThreads + kanbanColumns.values.flatMap { it.threads })
                .distinctBy { it.id }
                .filter { it.id in selectedMailThreadIds }
        val mailSelectionActive = selectedMailThreadIds.isNotEmpty()

        LaunchedEffect(screen, coreThreads, kanbanColumns) {
            val visibleThreadIds =
                if (screen == Screen.Kanban) {
                    kanbanColumns.values
                        .flatMap { it.threads }
                        .map { it.id }
                        .toSet()
                } else {
                    coreThreads.map { it.id }.toSet()
                }
            val retainedThreadIds = selectedMailThreadIds.intersect(visibleThreadIds)
            if (retainedThreadIds.size != selectedMailThreadIds.size) {
                selectedMailThreadIds = retainedThreadIds
            }
        }

        // MailRouteContent leaves composition while a conversation is open.
        // Keep its per-mailbox list states above the NavHost so returning to
        // the thread list restores the position that was visible on open.
        val mailListStates = remember { mutableMapOf<MailboxCacheKey, MailListPosition>() }

        NavHost(navController = navController, startDestination = startRoute) {
            composable(AppRoutes.Thread) {
                // Without a summary there is nothing to show and nothing to
                // load, so leave rather than sit on an endless spinner (the
                // thread was moved away, or the route was restored without it).
                LaunchedEffect(selectedCoreThread == null) {
                    if (selectedCoreThread == null) popAppBack()
                }
                ThreadScreen(
                    thread = selectedCoreThread,
                    messages = visibleThreadMessages(),
                    accountEmail = selectedThreadAccount?.email.orEmpty(),
                    wallpaperPresetId = selectedThreadAccount?.chatWallpaperPresetId.orEmpty(),
                    wallpaperCustomUrl =
                        selectedThreadAccount
                            ?.takeIf { it.chatWallpaperKind == "custom" }
                            ?.chatWallpaperUrl
                            .orEmpty(),
                    preferHtml = selectedThreadPreferHtml,
                    onPreferHtmlChange = { preferHtml ->
                        if (selectedThreadAccountId.isNotBlank()) {
                            conversationHtmlOverrides = conversationHtmlOverrides + (selectedThreadAccountId to preferHtml)
                        }
                    },
                    onBack = popAppBack,
                    onArchive = {
                        selectedCoreThread?.let {
                            archiveOrRemove(it)
                            popAppBack()
                        }
                    },
                    onDelete = {
                        selectedCoreThread?.let {
                            deleteThread(it)
                            popAppBack()
                        }
                    },
                    onToggleStar = {
                        selectedCoreThread?.let { t ->
                            toggleStar(t)
                            selectedCoreThread = t.copy(starred = !t.starred)
                        }
                    },
                    moveFolders =
                        selectedCoreThread
                            ?.let { thread -> foldersByAccount[thread.accountId].orEmpty() }
                            .orEmpty(),
                    copyFolders =
                        coreAccounts
                            .filterNot { accountSummaryIsRss(it) }
                            .flatMap { account -> foldersByAccount[account.id].orEmpty() },
                    onMoveToFolder = { folder ->
                        selectedCoreThread?.let { thread ->
                            moveThreadToFolder(thread, folder.name) {
                                popAppBack()
                            }
                        }
                    },
                    onCreateFolderAndMove = { name ->
                        selectedCoreThread?.let { thread ->
                            createFolderAndMoveThread(thread, name) {
                                popAppBack()
                            }
                        }
                    },
                    onCopyToFolder = { folder ->
                        selectedCoreThread?.let { thread ->
                            copyThreadToFolder(thread, folder)
                        }
                    },
                    quickReplyBody = quickReplyBody,
                    canLoadOlder = messageCursor.isNotBlank(),
                    loadingOlder = loadingMoreMessages,
                    onLoadOlder = ::loadMoreThreadMessages,
                    onQuickReplyChange = ::onQuickReplyBodyChange,
                    quickReplyAttachments = quickReplyAttachments,
                    quickReplyFailure = quickReplyFailure,
                    quickReplyHasContent = !quickReplyIsBlank(),
                    quickReplySending = quickReplySendInFlight,
                    sendShortcutMode = sendShortcutMode,
                    conversationLayout = conversationLayout,
                    onQuickReplyAttach = {
                        pickAttachmentInto { picked ->
                            quickReplyAttachments = quickReplyAttachments + picked
                            ++quickReplyGeneration
                            quickReplyFailure = ""
                            autoSaveQuickReplyDraft()
                        }
                    },
                    onRemoveQuickReplyAttachment = { attachment ->
                        quickReplyAttachments = quickReplyAttachments.filterNot { it.id == attachment.id }
                        ++quickReplyGeneration
                        quickReplyFailure = ""
                        if (quickReplyIsBlank()) {
                            discardQuickReplyDraftIfEmpty()
                        } else {
                            autoSaveQuickReplyDraft()
                        }
                    },
                    onOpenFullReply = ::openQuickReplyInFullEditor,
                    onSendReply = ::sendQuickReply,
                    onRetryReply = ::retryQuickReplySend,
                    quickReplyFromIdentities = quickReplyIdentities(),
                    quickReplySelectedFrom = selectedQuickReplyIdentity(),
                    onSelectQuickReplyFrom = { identity ->
                        quickReplyFrom = identity.email
                        ++quickReplyGeneration
                        // The saved draft carries the From header, so re-save it
                        // against the newly chosen identity instead of waiting
                        // for the next keystroke's debounce.
                        if (quickReplyDraftSaved) autoSaveQuickReplyDraft()
                    },
                    onForward = { openMessageCompose(it, forward = true) },
                    onEditAsNew = { openMessageCompose(it, forward = false) },
                    onOpenDraft = { message ->
                        selectedCoreThread?.let { thread ->
                            openDraftCompose(message, thread, returnScreen = Screen.Thread)
                        }
                    },
                    onToggleMessageRead = ::toggleMessageRead,
                    onToggleMessageStarred = ::toggleMessageStarred,
                    onDeleteMessage = ::deleteMessage,
                    onOpenAttachment = ::openMessageAttachment,
                    onSaveAttachment = ::saveMessageAttachment,
                    onShareImageAttachment = ::shareImageAttachment,
                    onCopyImageAttachment = ::copyImageAttachment,
                    loadImageAttachment = { attachment ->
                        val cacheKey = attachment.key.ifBlank { attachment.url }
                        loadCachedImageBitmap(cacheKey) {
                            if (attachment.key.isNotBlank()) {
                                withContext(ioDispatcher) { decodeImageBitmap(readAttachmentBytes(attachment)) }
                            } else {
                                loadImageBitmapRef(attachment.url)
                            }
                        }
                    },
                    onComposeTo = { email ->
                        openComposeTo(email, selectedCoreThread?.accountId ?: selectedCoreAccountId)
                    },
                    onCopyMessageText = { label, value ->
                        services.copyText(label, value)
                        status = "Copied ${label.lowercase()}"
                    },
                    onRetryLoadMessages = { retryOpenThreadLoad() },
                    onMessagesRead = ::markMessagesReadOnScroll,
                    onViewedToBottom = ::markThreadReadOnScroll,
                    snackbarHost = snackbarHost,
                )
            }

            composable(AppRoutes.Compose) {
                BackHandler(
                    onBack = { closeCompose() },
                )
                ComposeScreen(
                    sendIdentities = composeIdentityCandidates(),
                    selectedFromKey = selectedComposeIdentity()?.let { identityKey(it) }.orEmpty(),
                    onFromChange = { key ->
                        val split = key.indexOf('|')
                        if (split > 0) {
                            changeComposeIdentity(key.substring(0, split), key.substring(split + 1))
                        }
                    },
                    to = to,
                    onToChange = {
                        to = it
                        loadRecipientSuggestions("to", it)
                    },
                    cc = cc,
                    onCcChange = {
                        cc = it
                        loadRecipientSuggestions("cc", it)
                    },
                    bcc = bcc,
                    onBccChange = {
                        bcc = it
                        loadRecipientSuggestions("bcc", it)
                    },
                    subject = subject,
                    onSubjectChange = { subject = it },
                    body = body,
                    onBodyChange = { body = it },
                    attachments = attachments,
                    recipientSuggestionField = recipientSuggestionField,
                    recipientSuggestions = recipientSuggestions,
                    onRecipientFocus = { field, value -> loadRecipientSuggestions(field, value) },
                    onAcceptRecipientSuggestion = ::acceptRecipientSuggestion,
                    onAttach = {
                        pickAttachmentInto { picked ->
                            attachments = attachments + picked
                        }
                    },
                    onClearAttachments = { attachments = emptyList() },
                    onRemoveAttachment = { draftAttachment ->
                        attachments = attachments.filter { it.id != draftAttachment.id }
                    },
                    sendShortcutMode = sendShortcutMode,
                    onSaveDraft = ::saveComposeDraft,
                    onAutoSaveDraft = ::autoSaveComposeDraft,
                    onDiscardDraft = ::discardComposeDraft,
                    onSend = ::sendMail,
                    onBack = ::closeCompose,
                    sending = composeSendInFlight,
                    snackbarHost = { SnackbarHost(snackbarHost) },
                )
            }

            composable(AppRoutes.AddAccount) {
                LaunchedEffect(Unit) {
                    if (rssDisplayName.isBlank()) {
                        rssDisplayName = nextRssAccountDisplayName(coreAccounts)
                    }
                }
                AddAccountScreen(
                    onBack = popAppBack,
                    initialSection = addSection,
                    displayName = displayName,
                    onDisplayNameChange = { displayName = it },
                    senderName = senderName,
                    onSenderNameChange = { senderName = it },
                    email = email,
                    onEmailChange = { email = it },
                    username = username,
                    onUsernameChange = { username = it },
                    password = password,
                    onPasswordChange = { password = it },
                    host = host,
                    onHostChange = {
                        host = it
                        hostTouched = it.isNotBlank()
                    },
                    imapPort = imapPort,
                    onImapPortChange = {
                        imapPort = it
                        imapSecurity = mailSecurityAfterPortEdit(imapSecurity, imapSecurityTouched, it)
                        imapPortTouched = it.isNotBlank()
                    },
                    imapSecurity = imapSecurity,
                    onImapSecurityChange = {
                        imapSecurity = it
                        imapSecurityTouched = true
                    },
                    smtpHost = smtpHost,
                    onSmtpHostChange = {
                        smtpHost = it
                        smtpHostTouched = it.isNotBlank()
                    },
                    smtpPort = smtpPort,
                    onSmtpPortChange = {
                        smtpPort = it
                        smtpSecurity = mailSecurityAfterPortEdit(smtpSecurity, smtpSecurityTouched, it)
                        smtpPortTouched = it.isNotBlank()
                    },
                    smtpSecurity = smtpSecurity,
                    onSmtpSecurityChange = {
                        smtpSecurity = it
                        smtpSecurityTouched = true
                    },
                    serverSettingsOpen = passwordServerSettingsOpen,
                    onServerSettingsOpenChange = { passwordServerSettingsOpen = it },
                    onAutodiscover = ::autodiscoverPasswordAccount,
                    onEmailBlur = { autodiscoverPasswordAccount(auto = true) },
                    onAddPassword = ::addPasswordAccount,
                    oauthAuthorizationCode = oauthAuthorizationCode,
                    onLaunchOAuth = {
                        oauthProvider = "outlook"
                        launchOAuthFlow()
                    },
                    onConnectGoogleDeviceAccount = {
                        oauthProvider = "gmail"
                        connectGoogleDeviceAccount()
                    },
                    rssFeedUrl = rssFeedUrl,
                    onRssFeedUrlChange = { rssFeedUrl = it },
                    rssDisplayName = rssDisplayName,
                    onRssDisplayNameChange = { rssDisplayName = it },
                    rssAccountAdding = rssAccountAdding,
                    onAddRss = ::addRssAccount,
                )
            }

            composable(AppRoutes.Settings) {
                LaunchedEffect(Unit) { loadStorageUsage() }
                SettingsScreen(
                    onBack = popAppBack,
                    initialGeneral = settingsGeneralTarget,
                    onConsumeInitialGeneral = { settingsGeneralTarget = false },
                    initialAccountId = accountSettingsTargetId,
                    initialAccountProxy =
                        accountSettingsTargetId != null &&
                            accountSettingsTargetId == accountSettingsProxyTargetId,
                    onConsumeInitialAccount = {
                        accountSettingsTargetId = null
                        accountSettingsProxyTargetId = null
                    },
                    initialKanbanBoardId = kanbanSettingsTargetId,
                    onConsumeInitialKanbanBoard = { kanbanSettingsTargetId = null },
                    accounts = coreAccounts,
                    hiddenNavigationAccountIds = hiddenNavigationAccountIds,
                    kanbanBoards = kanbanBoards,
                    onSaveKanbanBoard = { board, name, avatarUrl, wallpaperPresetId, wallpaperUrl ->
                        updateKanbanBoard(board.id, name, avatarUrl, wallpaperPresetId, wallpaperUrl)
                    },
                    onDeleteKanbanBoard = { board ->
                        deleteKanbanBoard(board.id)
                    },
                    onCreateKanbanBoard = ::createKanbanBoard,
                    onAddMailAccount = {
                        resetPasswordAccountForm()
                        addSection = 0
                        previousTopScreen = Screen.Settings
                        screen = Screen.AddAccount
                    },
                    onAddFeedAccount = {
                        resetPasswordAccountForm()
                        addSection = 2
                        previousTopScreen = Screen.Settings
                        screen = Screen.AddAccount
                    },
                    onSaveAccountSettings = {
                        account,
                        displayName,
                        senderName,
                        avatarUrl,
                        wallpaperPresetId,
                        loadRemoteImages,
                        conversationHtml,
                        includedInUnified,
                        showInNavigation,
                        muted,
                        paused,
                        interval,
                        aliases,
                        ->
                        setAccountNavigationVisible(account, showInNavigation)
                        saveAccountSettings(
                            account,
                            displayName,
                            senderName,
                            avatarUrl,
                            wallpaperPresetId,
                            loadRemoteImages,
                            conversationHtml,
                            includedInUnified,
                            muted,
                            paused,
                            interval,
                            aliases,
                        )
                    },
                    onPickAccountAvatar = { account ->
                        accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = false)
                        pickAccountMedia()
                    },
                    onPickAccountWallpaper = { account ->
                        accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = true)
                        pickAccountMedia()
                    },
                    onPickKanbanBoardAvatar = { board ->
                        kanbanBoardMediaTarget = KanbanBoardMediaTarget(board, wallpaper = false)
                        pickKanbanBoardMedia()
                    },
                    onPickKanbanBoardWallpaper = { board ->
                        kanbanBoardMediaTarget = KanbanBoardMediaTarget(board, wallpaper = true)
                        pickKanbanBoardMedia()
                    },
                    onMoveAccountUp = { account -> moveAccount(account, -1) },
                    onMoveAccountDown = { account -> moveAccount(account, 1) },
                    onRemoveAccount = ::removeAccount,
                    appearanceMode = appearanceMode,
                    onAppearanceModeChange = onAppearanceModeChange,
                    appLanguageTag = appLanguageTag,
                    onAppLanguageChange = onAppLanguageChange,
                    showSenderImages = showSenderImages,
                    onToggleSenderImages = {
                        showSenderImages = !showSenderImages
                        saveAppBoolean(prefs, SHOW_SENDER_IMAGES_PREF, showSenderImages)
                    },
                    showUnreadBadges = showUnreadBadges,
                    onToggleUnreadBadges = {
                        showUnreadBadges = !showUnreadBadges
                        saveAppBoolean(prefs, SHOW_UNREAD_BADGES_PREF, showUnreadBadges)
                    },
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    onToggleUnifiedInboxNav = {
                        showUnifiedInboxNav = !showUnifiedInboxNav
                        saveAppBoolean(prefs, SHOW_UNIFIED_INBOX_PREF, showUnifiedInboxNav)
                    },
                    sendShortcutMode = sendShortcutMode,
                    onToggleSendShortcut = {
                        val next = sendShortcutMode.next()
                        sendShortcutMode = next
                        saveSendShortcutMode(prefs, next)
                    },
                    conversationLayout = conversationLayout,
                    onToggleConversationLayout = {
                        val next = conversationLayout.next()
                        conversationLayout = next
                        saveConversationLayout(prefs, next)
                    },
                    messageFontScale = messageFontScale,
                    onMessageFontScaleChange = { scale ->
                        messageFontScale = coerceMessageFontScale(scale)
                        saveMessageFontScale(prefs, messageFontScale)
                    },
                    appProxy = appProxy,
                    onSaveAppProxy = ::saveAppProxy,
                    appSignatureHtml = appSignatureHtml,
                    onSaveAppSignature = ::saveAppSignature,
                    onSaveAccountSignature = ::saveAccountSignature,
                    onSaveAccountProxy = ::saveAccountProxy,
                    onSaveAccountServerSettings = ::saveAccountServerSettings,
                    kanbanColumnWidth = kanbanColumnWidth,
                    onCycleKanbanColumnWidth = {
                        val next = nextKanbanColumnWidth(kanbanColumnWidth)
                        kanbanColumnWidth = next
                        saveAppInt(prefs, KANBAN_COLUMN_WIDTH_PREF, next)
                    },
                    notificationsNeedPermission = !notificationPermissionGranted,
                    onEnableNotifications = {
                        mobileHost.requestNotificationPermission()
                        notificationPermissionGranted = mobileHost.notificationsEnabled()
                    },
                    supportsBackgroundPush = mobileHost.supportsBackgroundPush,
                    liveMailPushEnabled = liveMailPushEnabled,
                    onToggleLiveMailPush = {
                        val next = !liveMailPushEnabled
                        liveMailPushEnabled = next
                        saveAppBoolean(prefs, LIVE_MAIL_PUSH_PREF, next)
                        mobileHost.syncLiveMailPush(next)
                        status = if (next) "Live mail push enabled" else "Live mail push disabled"
                    },
                    backgroundSyncEnabled = backgroundSyncEnabled,
                    onToggleBackgroundSync = {
                        val next = !backgroundSyncEnabled
                        backgroundSyncEnabled = next
                        saveAppBoolean(prefs, BACKGROUND_SYNC_ENABLED_PREF, next)
                        mobileHost.syncBackgroundRefresh(next)
                        status = if (next) "Background sync enabled" else "Background sync disabled"
                    },
                    onRefreshBackground = {
                        mobileHost.runBackgroundRefreshOnce()
                        status = "Queued background refresh"
                    },
                    readDiagnosticLog = { mobileHost.readDiagnosticLog() },
                    onShareDiagnosticLog = { mobileHost.shareDiagnosticLog() },
                    pollIntervalMinutes = pollIntervalMinutes,
                    onCyclePollInterval = {
                        val next = nextPollIntervalMinutes(pollIntervalMinutes)
                        pollIntervalMinutes = next
                        saveAppInt(prefs, POLL_INTERVAL_MINUTES_PREF, next)
                    },
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    onRefreshStorage = { loadStorageUsage(showStatus = true) },
                    onClearStorageCache = ::clearStorageCache,
                    onExportBackup = {
                        backupPassphraseError = ""
                        backupPassphraseMode = BackupPassphraseMode.Export
                    },
                    onRestoreBackup = pickBackupFile,
                    backupBusy = backupBusy,
                )
                backupPassphraseMode?.let { mode ->
                    BackupPassphraseDialog(
                        mode = mode,
                        busy = backupBusy,
                        error = backupPassphraseError,
                        onDismiss = ::closeBackupPassphrase,
                        onConfirm = { passphrase, includeSecrets ->
                            when (mode) {
                                BackupPassphraseMode.Export -> exportBackup(includeSecrets, passphrase)
                                BackupPassphraseMode.Restore -> retryBackupRestore(passphrase)
                            }
                        },
                    )
                }
            }

            composable(AppRoutes.Kanban) {
                KanbanRouteContent(
                    state = state,
                    drawerState = drawerState,
                    drawerFolders = drawerFolders,
                    mailSelectionActive = mailSelectionActive,
                    selectedMailThreads = selectedMailThreads,
                    activeKanbanBoard = activeKanbanBoard,
                )
            }

            composable(AppRoutes.Mail) {
                MailRouteContent(
                    state = state,
                    drawerState = drawerState,
                    drawerFolders = drawerFolders,
                    selectedAccount = selectedAccount,
                    selectedAccountIsRss = selectedAccountIsRss,
                    mailSelectionActive = mailSelectionActive,
                    selectedMailThreads = selectedMailThreads,
                    importOpml = importOpml,
                    mailListStates = mailListStates,
                )
            }
        }

        if (showAddFeedDialog) {
            AddFeedDialog(
                url = addFeedUrl,
                onUrlChange = {
                    addFeedUrl = it
                    addFeedError = ""
                },
                error = addFeedError,
                submitting = addFeedSubmitting,
                onAdd = ::addFeedToSelectedRssAccount,
                onDismiss = {
                    if (!addFeedSubmitting) {
                        showAddFeedDialog = false
                        addFeedError = ""
                    }
                },
            )
        }

        selectedMailMoveThread?.let { thread ->
            MoveThreadDialog(
                thread = thread,
                folders =
                    foldersByAccount[thread.accountId]
                        .orEmpty()
                        .filterNot { folder -> folder.name.equals(thread.folder, ignoreCase = true) },
                onMove = { folder ->
                    selectedMailMoveThread = null
                    moveThreadToFolder(thread, folder.name) {
                        selectedMailThreadIds = emptySet()
                    }
                },
                onCreateAndMove = { name ->
                    selectedMailMoveThread = null
                    createFolderAndMoveThread(thread, name) {
                        selectedMailThreadIds = emptySet()
                    }
                },
                onDismiss = { selectedMailMoveThread = null },
            )
        }

        selectedMailCopyThread?.let { thread ->
            CopyThreadDialog(
                thread = thread,
                accounts = coreAccounts.filterNot { accountSummaryIsRss(it) },
                folders =
                    coreAccounts
                        .filterNot { accountSummaryIsRss(it) }
                        .flatMap { account -> foldersByAccount[account.id].orEmpty() },
                onCopy = { folder ->
                    selectedMailCopyThread = null
                    copyThreadToFolder(thread, folder)
                    selectedMailThreadIds = emptySet()
                },
                onDismiss = { selectedMailCopyThread = null },
            )
        }

        pendingEmptyFolder?.let { target ->
            EmptyFolderDialog(
                folderName = target.folderName,
                onConfirm = {
                    pendingEmptyFolder = null
                    emptyMailFolder(
                        accountId = target.accountId,
                        folderId = target.folderId,
                        column = target.column,
                    )
                },
                onDismiss = { pendingEmptyFolder = null },
            )
        }

        pendingDeleteFolder?.let { target ->
            DeleteFolderDialog(
                folderName = target.folderName,
                nested = target.nested,
                onConfirm = {
                    pendingDeleteFolder = null
                    deleteMailFolder(
                        accountId = target.accountId,
                        folderId = target.folderId,
                        column = target.column,
                    )
                },
                onDismiss = { pendingDeleteFolder = null },
            )
        }

        certPrompt?.let { prompt ->
            CertificateTrustDialog(
                server = "${prompt.host}:${prompt.port}",
                certificate = prompt.certificate,
                busy = certPromptBusy,
                onTrust = ::trustPromptedCertificate,
                onDismiss = ::dismissCertificatePrompt,
            )
        }

        if (showAboutDialog) {
            AboutDialog(
                appVersion = appVersion,
                onOpenUrl = services::openUrl,
                onShowChangelog = {
                    showAboutDialog = false
                    showChangelogDialog = true
                    loadChangelog()
                },
                onDismiss = { showAboutDialog = false },
            )
        }

        if (pendingCrashReport.isNotBlank()) {
            CrashReportDialog(
                summary = pendingCrashReport,
                onSend = {
                    pendingCrashReport = ""
                    mobileHost.clearPendingCrashReport()
                    mobileHost.shareDiagnosticLog()
                },
                onDismiss = {
                    pendingCrashReport = ""
                    mobileHost.clearPendingCrashReport()
                },
            )
        }

        if (showChangelogDialog) {
            ChangelogDialog(
                releases = changelog,
                loading = changelogLoading,
                error = changelogError,
                onDismiss = { showChangelogDialog = false },
            )
        }

        imagePreview?.let { preview ->
            ImagePreviewDialog(
                preview = preview,
                onShare = { shareImagePreview(preview) },
                onCopy = { copyImagePreview(preview) },
                onDismiss = { imagePreview = null },
            )
        }

        if (showKanbanColumnDialog && screen == Screen.Kanban) {
            KanbanColumnDialog(
                accounts = coreAccounts,
                board = activeKanbanBoard,
                foldersByAccount = foldersByAccount,
                onApply = {
                    applyKanbanColumns(it)
                    showKanbanColumnDialog = false
                },
                onCreateFolder = {
                    showKanbanCreateFolderDialog = it
                    kanbanFolderNameInput = ""
                },
                onDismiss = { showKanbanColumnDialog = false },
            )
        }

        showKanbanCreateFolderDialog?.let { account ->
            KanbanCreateFolderDialog(
                name = kanbanFolderNameInput,
                delimiter = folderTreeDelimiter(foldersByAccount[account.id].orEmpty()),
                onNameChange = { kanbanFolderNameInput = it },
                onCreate = { createFolderForKanban(account, kanbanFolderNameInput) },
                onDismiss = { showKanbanCreateFolderDialog = null },
            )
        }

        kanbanActionThread?.let { thread ->
            KanbanThreadActionDialog(
                thread = thread,
                board = activeKanbanBoard,
                accounts = coreAccounts,
                onDismiss = { kanbanActionThread = null },
                onOpen = {
                    kanbanActionThread = null
                    readCoreThread(thread)
                },
                onToggleStar = {
                    kanbanActionThread = null
                    toggleStar(thread)
                },
                onToggleRead = {
                    kanbanActionThread = null
                    toggleRead(thread)
                },
                onArchive = {
                    kanbanActionThread = null
                    archiveOrRemove(thread)
                },
                onDelete = {
                    kanbanActionThread = null
                    deleteThread(thread)
                },
                onCopyFeedUrl = {
                    kanbanActionThread = null
                    services.copyText("Feed URL", thread.feedUrl)
                    status = "Copied feed URL"
                },
                onMove = { target ->
                    kanbanActionThread = null
                    moveThreadToColumn(thread, target)
                },
            )
        }
    }
}
