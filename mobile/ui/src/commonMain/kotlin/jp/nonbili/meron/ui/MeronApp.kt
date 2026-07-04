package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material.icons.outlined.NotificationsNone
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import kotlin.math.abs

private const val MAILBOX_BLOCKING_WARN_AFTER_MS = 10_000L
private const val MAILBOX_BLOCKING_TIMEOUT_MS = 15_000L

// Hard cap for a blocking load whose sync is still in flight: past the soft
// timeout the loader shows "still syncing" copy, past this one we give up and
// surface the timeout error.
private const val MAILBOX_BLOCKING_HARD_TIMEOUT_MS = 60_000L

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
    incomingOAuthCallbackUrl: String? = null,
    incomingNotificationThreadTarget: NotificationThreadTarget? = null,
    appearanceMode: AppAppearanceMode = loadAppearanceMode(prefs),
    onAppearanceModeChange: (AppAppearanceMode) -> Unit = { saveAppearanceMode(prefs, it) },
    appLanguageTag: String = loadAppLanguageTag(prefs),
    onAppLanguageChange: (String) -> Unit = {},
) {
    val scope = rememberCoroutineScope()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val state =
        remember(core, prefs, kanbanPrefs, services, locale, mobileHost) {
            MeronMobileState(
                scope = scope,
                core = core,
                coreLoaded = coreLoaded,
                prefs = prefs,
                kanbanPrefs = kanbanPrefs,
                services = services,
                locale = locale,
                mobileHost = mobileHost,
            )
        }
    androidx.compose.runtime.CompositionLocalProvider(
        LocalAppLocale provides appLanguageTag.ifBlank { locale.currentLanguageTag().ifBlank { "en" } },
        LocalPlatformServices provides services,
    ) {
        MeronTheme(appearanceMode) {
            MeronMobileScreenContent(
                state = state,
                drawerState = drawerState,
                coreInitJson = coreInitJson,
                incomingMailtoDraft = incomingMailtoDraft,
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

@Composable
private fun MailSelectionTitle(
    selectedCount: Int,
    height: Dp,
) {
    Box(
        modifier = Modifier.fillMaxWidth().height(height),
        contentAlignment = Alignment.CenterStart,
    ) {
        Text(
            "$selectedCount selected",
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
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

private fun String.jsonStringValue(key: String): String {
    val pattern = Regex(""""${Regex.escape(key)}"\s*:\s*"((?:\\.|[^"\\])*)"""")
    return pattern
        .find(this)
        ?.groupValues
        ?.get(1)
        ?.decodeJsonString()
        .orEmpty()
}

private fun String.jsonIntValue(
    key: String,
    defaultValue: Int,
): Int {
    val pattern = Regex(""""${Regex.escape(key)}"\s*:\s*(-?\d+)""")
    return pattern
        .find(this)
        ?.groupValues
        ?.get(1)
        ?.toIntOrNull() ?: defaultValue
}

private fun String.decodeJsonString(): String {
    val out = StringBuilder()
    var index = 0
    while (index < length) {
        val ch = this[index]
        if (ch == '\\' && index + 1 < length) {
            when (val escaped = this[index + 1]) {
                '"', '\\', '/' -> out.append(escaped)
                'b' -> out.append('\b')
                'f' -> out.append('\u000c')
                'n' -> out.append('\n')
                'r' -> out.append('\r')
                't' -> out.append('\t')
                else -> out.append(escaped)
            }
            index += 2
        } else {
            out.append(ch)
            index += 1
        }
    }
    return out.toString()
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
private fun MeronMobileScreenContent(
    state: MeronMobileState,
    drawerState: androidx.compose.material3.DrawerState,
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft?,
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
            val currentRoute = currentBackStackEntry?.destination?.route
            if (currentRoute == null) {
                pendingRoute = targetRoute
            } else if (currentRoute != targetRoute) {
                if (!navController.popBackStack(targetRoute, inclusive = false)) {
                    navController.navigate(targetRoute) {
                        launchSingleTop = true
                    }
                }
            }
        }
        LaunchedEffect(currentBackStackEntry?.destination?.route) {
            val currentRoute = currentBackStackEntry?.destination?.route
            val queuedRoute = pendingRoute
            if (queuedRoute != null && currentRoute != null && currentRoute != queuedRoute) {
                pendingRoute = null
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
            if (!navController.popBackStack()) {
                screen = previousTopScreen
            }
        }
        DisposableEffect(Unit) {
            val handle =
                if (coreLoaded) {
                    core.events().subscribe { event ->
                        when (event.name) {
                            "mail.newMessages" -> {
                                if (!liveMailPushEnabled) {
                                    mobileHost.notifyNewMail(
                                        accountName = event.detailJson.jsonStringValue("accountName"),
                                        from = event.detailJson.jsonStringValue("from"),
                                        subject = event.detailJson.jsonStringValue("subject"),
                                        count = event.detailJson.jsonIntValue("count", 1),
                                        accountId = event.detailJson.jsonStringValue("account"),
                                        folder = event.detailJson.jsonStringValue("folder"),
                                        threadKey = event.detailJson.jsonStringValue("threadKey"),
                                    )
                                }
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    syncCoreThreads(
                                        accountOverride = selectedCoreAccountId,
                                        folderOverride = selectedCoreFolder,
                                        syncFirst = false,
                                    )
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
                                    refreshOpenThreadFor(eventAccount)
                                }
                            }

                            "mail.synced" -> {
                                val eventAccount = event.detailJson.jsonStringValue("account")
                                val eventFolder = event.detailJson.jsonStringValue("folder")
                                scope.launch {
                                    syncCoreThreads(
                                        accountOverride = selectedCoreAccountId,
                                        folderOverride = selectedCoreFolder,
                                        syncFirst = false,
                                    )
                                    refreshKanbanColumnsForMailEvent(eventAccount, eventFolder)
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
        // Foreground refresh for platforms without a real background channel
        // (iOS): re-sync the visible mailbox on the chosen interval while the app
        // is open, and immediately when it returns to the foreground. Both honor
        // "Off" (interval 0). The timer suspends with the app and resumes on
        // return; the foreground signal covers the gap after a long suspension.
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
            LaunchedEffect(coreLoaded) {
                if (!coreLoaded) return@LaunchedEffect
                AppForegroundSignal.events.collect {
                    if (pollIntervalMinutes <= 0) return@collect
                    syncCoreThreads(
                        accountOverride = selectedCoreAccountId,
                        folderOverride = selectedCoreFolder,
                        syncFirst = true,
                    )
                }
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
                            status = "Rust core not packaged."
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
                            status = "Rust core not packaged."
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

        LaunchedEffect(incomingMailtoDraft) {
            incomingMailtoDraft?.let { draft ->
                composeDraftId = ""
                composeDraftSaved = false
                to = draft.to
                cc = draft.cc
                bcc = draft.bcc
                subject = draft.subject
                body = draft.body
                composeReturnScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
                screen = Screen.Compose
                status = "Loaded compose draft from mailto link"
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

        NavHost(navController = navController, startDestination = startRoute) {
            composable(AppRoutes.Thread) {
                ThreadScreen(
                    thread = selectedCoreThread,
                    messages = messages,
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
                    onQuickReplyChange = {
                        quickReplyBody = it
                        quickReplyFailure = ""
                    },
                    quickReplyAttachments = quickReplyAttachments,
                    quickReplyFailure = quickReplyFailure,
                    sendShortcutMode = sendShortcutMode,
                    onQuickReplyAttach = {
                        pickAttachmentInto { picked ->
                            quickReplyAttachments = quickReplyAttachments + picked
                            quickReplyFailure = ""
                        }
                    },
                    onRemoveQuickReplyAttachment = { attachment ->
                        quickReplyAttachments = quickReplyAttachments.filterNot { it.id == attachment.id }
                        quickReplyFailure = ""
                    },
                    onOpenFullReply = ::openQuickReplyInFullEditor,
                    onSendReply = ::sendQuickReply,
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
                    onComposeTo = { email ->
                        composeFromAccountId = selectedCoreThread?.accountId ?: selectedCoreAccountId
                        composeFromEmail = ""
                        to = email
                        cc = ""
                        bcc = ""
                        subject = ""
                        body = ""
                        attachments = emptyList()
                        composeDraftId = ""
                        composeDraftSaved = false
                        composeReturnScreen = Screen.Thread
                        screen = Screen.Compose
                    },
                    onCopyMessageText = { label, value ->
                        services.copyText(label, value)
                        status = "Copied ${label.lowercase()}"
                    },
                    onRetryLoadMessages = { retryOpenThreadLoad() },
                    onMessagesScrolledPast = ::markMessagesReadOnScroll,
                    onViewedToBottom = ::markThreadReadOnScroll,
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
                            composeFromAccountId = key.substring(0, split)
                            composeFromEmail = key.substring(split + 1)
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
                )
            }

            composable(AppRoutes.AddAccount) {
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
                    onHostChange = { host = it },
                    imapPort = imapPort,
                    onImapPortChange = { imapPort = it },
                    smtpHost = smtpHost,
                    onSmtpHostChange = { smtpHost = it },
                    smtpPort = smtpPort,
                    onSmtpPortChange = { smtpPort = it },
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
                    onAddRss = ::addRssAccount,
                )
            }

            composable(AppRoutes.Settings) {
                LaunchedEffect(Unit) { loadStorageUsage() }
                SettingsScreen(
                    onBack = popAppBack,
                    initialAccountId = accountSettingsTargetId,
                    onConsumeInitialAccount = { accountSettingsTargetId = null },
                    initialKanbanBoardId = kanbanSettingsTargetId,
                    onConsumeInitialKanbanBoard = { kanbanSettingsTargetId = null },
                    accounts = coreAccounts,
                    hiddenNavigationAccountIds = hiddenNavigationAccountIds,
                    kanbanBoards = kanbanBoards,
                    activeKanbanBoardId = activeKanbanBoardId,
                    onSaveKanbanBoard = { board, name, avatarUrl, wallpaperPresetId, wallpaperUrl ->
                        updateKanbanBoard(board.id, name, avatarUrl, wallpaperPresetId, wallpaperUrl)
                    },
                    onDeleteKanbanBoard = { board ->
                        deleteKanbanBoard(board.id)
                    },
                    onCreateKanbanBoard = ::createKanbanBoard,
                    onAddMailAccount = {
                        addSection = 0
                        passwordServerSettingsOpen = false
                        previousTopScreen = Screen.Settings
                        screen = Screen.AddAccount
                    },
                    onAddFeedAccount = {
                        addSection = 2
                        passwordServerSettingsOpen = false
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
                    showStarredNav = showStarredNav,
                    onToggleStarredNav = {
                        showStarredNav = !showStarredNav
                        saveAppBoolean(prefs, SHOW_STARRED_NAV_PREF, showStarredNav)
                    },
                    sendShortcutMode = sendShortcutMode,
                    onToggleSendShortcut = {
                        val next = sendShortcutMode.next()
                        sendShortcutMode = next
                        saveSendShortcutMode(prefs, next)
                    },
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
                )
            }

            composable(AppRoutes.Starred) {
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    drawerContent = {
                        MailDrawer(
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            selectedAccountId = selectedCoreAccountId,
                            folders = drawerFolders,
                            currentScreen = screen,
                            showUnreadBadges = showUnreadBadges,
                            showUnifiedInboxNav = showUnifiedInboxNav,
                            showStarredNav = showStarredNav,
                            kanbanBoards = kanbanBoards,
                            activeKanbanBoardId = activeKanbanBoardId,
                            onSelectUnified = {
                                screen = Screen.Mail
                                selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                                syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER, syncFirst = false)
                                scope.launch { drawerState.close() }
                            },
                            onSelectAccount = { account ->
                                screen = Screen.Mail
                                selectCoreMailbox(account.id, INBOX_FOLDER)
                                syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER, syncFirst = false)
                                scope.launch { drawerState.close() }
                            },
                            onSelectStarred = { scope.launch { drawerState.close() } },
                            onSelectKanban = {
                                screen = Screen.Kanban
                                previousTopScreen = Screen.Kanban
                                loadKanbanBoard(refresh = false)
                                scope.launch { drawerState.close() }
                            },
                            onSelectKanbanBoard = { board ->
                                activeKanbanBoardId = board.id
                                saveActiveKanbanBoardId(kanbanPrefs, board.id)
                                screen = Screen.Kanban
                                previousTopScreen = Screen.Kanban
                                loadKanbanBoard(refresh = false)
                                scope.launch { drawerState.close() }
                            },
                            onAddAccount = {
                                addSection = 0
                                passwordServerSettingsOpen = false
                                previousTopScreen = Screen.Starred
                                screen = Screen.AddAccount
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                previousTopScreen = screen
                                screen = Screen.Settings
                                scope.launch { drawerState.close() }
                            },
                            onShowAbout = {
                                showAboutDialog = true
                                scope.launch { drawerState.close() }
                            },
                            googleReauthAccountId = googleReauthAccountId,
                            onReconnectGoogle = {
                                connectGoogleDeviceAccount()
                                scope.launch { drawerState.close() }
                            },
                        )
                    },
                ) {
                    Scaffold(
                        topBar = {
                            TopAppBar(
                                title = {
                                    Column {
                                        Text(tr("mobile.tabs.starred"), fontWeight = FontWeight.SemiBold)
                                        Text(
                                            tr("mobile.mail.starredSubtitle"),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        )
                                    }
                                },
                                navigationIcon = {
                                    IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                        Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                                    }
                                },
                                actions = {
                                    IconButton(onClick = ::loadStarredItems) {
                                        Icon(Icons.Filled.Refresh, contentDescription = tr("mobile.actions.refreshStarred"))
                                    }
                                },
                            )
                        },
                        snackbarHost = { SnackbarHost(snackbarHost) },
                    ) { innerPadding ->
                        Box(Modifier.fillMaxSize().padding(innerPadding)) {
                            if (syncing) {
                                CircularProgressIndicator(Modifier.padding(top = 4.dp).align(Alignment.TopCenter).size(28.dp))
                            }
                            if (starredItems.isEmpty()) {
                                EmptyState(
                                    icon = Icons.Filled.StarBorder,
                                    title = tr("empty.noStarredItems"),
                                    text = tr("empty.noStarredItemsText"),
                                    actionLabel = tr("mobile.actions.refresh"),
                                    onAction = ::loadStarredItems,
                                )
                            } else {
                                StarredItemList(
                                    items = starredItems,
                                    showSenderImages = showSenderImages,
                                    onOpen = ::readStarredItem,
                                    onToggleRead = ::toggleStarredItemRead,
                                    onUnstar = ::unstarStarredItem,
                                    onDelete = ::deleteStarredMailItem,
                                )
                            }
                        }
                    }
                }
            }

            composable(AppRoutes.Kanban) {
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    drawerContent = {
                        MailDrawer(
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            selectedAccountId = selectedCoreAccountId,
                            folders = drawerFolders,
                            currentScreen = screen,
                            showUnreadBadges = showUnreadBadges,
                            showUnifiedInboxNav = showUnifiedInboxNav,
                            showStarredNav = showStarredNav,
                            kanbanBoards = kanbanBoards,
                            activeKanbanBoardId = activeKanbanBoardId,
                            onSelectUnified = {
                                screen = Screen.Mail
                                if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                                    selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                                    syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER)
                                }
                                scope.launch { drawerState.close() }
                            },
                            onSelectAccount = { account ->
                                screen = Screen.Mail
                                if (selectedCoreAccountId != account.id) {
                                    selectCoreMailbox(account.id, INBOX_FOLDER)
                                    syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER)
                                }
                                scope.launch { drawerState.close() }
                            },
                            onSelectStarred = {
                                screen = Screen.Starred
                                previousTopScreen = Screen.Starred
                                loadStarredItems()
                                scope.launch { drawerState.close() }
                            },
                            onSelectKanban = { scope.launch { drawerState.close() } },
                            onSelectKanbanBoard = { board ->
                                if (activeKanbanBoardId != board.id) {
                                    activeKanbanBoardId = board.id
                                    saveActiveKanbanBoardId(kanbanPrefs, board.id)
                                    loadKanbanBoard(refresh = false)
                                }
                                scope.launch { drawerState.close() }
                            },
                            onAddAccount = {
                                addSection = 0
                                passwordServerSettingsOpen = false
                                previousTopScreen = Screen.Kanban
                                screen = Screen.AddAccount
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                previousTopScreen = screen
                                screen = Screen.Settings
                                scope.launch { drawerState.close() }
                            },
                            onShowAbout = {
                                showAboutDialog = true
                                scope.launch { drawerState.close() }
                            },
                            googleReauthAccountId = googleReauthAccountId,
                            onReconnectGoogle = {
                                connectGoogleDeviceAccount()
                                scope.launch { drawerState.close() }
                            },
                        )
                    },
                ) {
                    Scaffold(
                        topBar = {
                            TopAppBar(
                                title = {
                                    if (mailSelectionActive) {
                                        MailSelectionTitle(selectedMailThreads.size, height = 52.dp)
                                    } else {
                                        KanbanHeaderSearchField(
                                            search = kanbanSearch,
                                            searchScope = kanbanSearchScope,
                                            columns = activeKanbanBoard?.columns.orEmpty(),
                                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                                            foldersByAccount = foldersByAccount,
                                            onSearchChange = ::persistKanbanSearch,
                                            onSearchScopeChange = ::persistKanbanSearchScope,
                                            onSearchSubmit = { loadKanbanBoard(refresh = true) },
                                        )
                                    }
                                },
                                navigationIcon = {
                                    if (mailSelectionActive) {
                                        IconButton(onClick = {
                                            selectedMailThreadIds = emptySet()
                                            mailSelectionMenuOpen = false
                                        }) {
                                            Icon(Icons.Filled.Close, contentDescription = tr("mobile.actions.clearSelection"))
                                        }
                                    } else {
                                        IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                            Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                                        }
                                    }
                                },
                                actions = {
                                    if (mailSelectionActive) {
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::archiveOrRemove)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Archive, contentDescription = tr("threads.actions.archiveThread"))
                                        }
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::deleteThread)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Delete, contentDescription = tr("buttons.delete"))
                                        }
                                        Box {
                                            IconButton(onClick = { mailSelectionMenuOpen = true }) {
                                                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"))
                                            }
                                            DropdownMenu(
                                                expanded = mailSelectionMenuOpen,
                                                onDismissRequest = { mailSelectionMenuOpen = false },
                                            ) {
                                                val markRead = selectedMailThreads.any { it.unread }
                                                DropdownMenuItem(
                                                    text = { Text(if (markRead) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread")) },
                                                    leadingIcon = { Icon(Icons.Filled.MarkEmailUnread, contentDescription = null) },
                                                    onClick = {
                                                        mailSelectionMenuOpen = false
                                                        selectedMailThreads
                                                            .filter { it.unread == markRead }
                                                            .forEach(::toggleRead)
                                                        selectedMailThreadIds = emptySet()
                                                    },
                                                )
                                                val star = selectedMailThreads.any { !it.starred }
                                                DropdownMenuItem(
                                                    text = { Text(if (star) tr("chat.star") else tr("chat.unstar")) },
                                                    leadingIcon = {
                                                        Icon(
                                                            if (star) Icons.Filled.StarBorder else Icons.Filled.Star,
                                                            contentDescription = null,
                                                        )
                                                    },
                                                    onClick = {
                                                        mailSelectionMenuOpen = false
                                                        selectedMailThreads
                                                            .filter { it.starred != star }
                                                            .forEach(::toggleStar)
                                                        selectedMailThreadIds = emptySet()
                                                    },
                                                )
                                                val singleSelectedMailThread =
                                                    selectedMailThreads
                                                        .singleOrNull()
                                                        ?.takeUnless { threadIdIsRss(it.id) }
                                                if (singleSelectedMailThread != null) {
                                                    HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                    DropdownMenuItem(
                                                        text = { Text(tr("threads.actions.moveTo")) },
                                                        leadingIcon = { Icon(Icons.Outlined.FolderOpen, contentDescription = null) },
                                                        onClick = {
                                                            mailSelectionMenuOpen = false
                                                            ensureThreadActionFolders(
                                                                thread = singleSelectedMailThread,
                                                                includeAllMailAccounts = false,
                                                            ) {
                                                                selectedMailMoveThread = singleSelectedMailThread
                                                            }
                                                        },
                                                    )
                                                    DropdownMenuItem(
                                                        text = { Text(tr("threads.actions.copyTo")) },
                                                        leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                                                        onClick = {
                                                            mailSelectionMenuOpen = false
                                                            ensureThreadActionFolders(
                                                                thread = singleSelectedMailThread,
                                                                includeAllMailAccounts = true,
                                                            ) {
                                                                selectedMailCopyThread = singleSelectedMailThread
                                                            }
                                                        },
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        Box {
                                            IconButton(onClick = { kanbanMenuOpen = true }) {
                                                Icon(Icons.Filled.MoreVert, contentDescription = tr("kanban.actions.boardOptions"))
                                            }
                                            DropdownMenu(expanded = kanbanMenuOpen, onDismissRequest = { kanbanMenuOpen = false }) {
                                                FilterMode.values().forEach { mode ->
                                                    DropdownMenuItem(
                                                        text = { Text(mode.label()) },
                                                        leadingIcon = {
                                                            RadioButton(
                                                                selected = kanbanFilter == mode,
                                                                onClick = null,
                                                            )
                                                        },
                                                        onClick = {
                                                            kanbanMenuOpen = false
                                                            persistKanbanFilter(mode)
                                                            loadKanbanBoard(refresh = true)
                                                        },
                                                    )
                                                }
                                                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                DropdownMenuItem(
                                                    text = { Text(tr("mobile.actions.refreshBoard")) },
                                                    leadingIcon = { Icon(Icons.Filled.Refresh, contentDescription = null) },
                                                    onClick = {
                                                        kanbanMenuOpen = false
                                                        loadKanbanBoard(refresh = true)
                                                    },
                                                )
                                                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                DropdownMenuItem(
                                                    text = { Text(tr("kanban.actions.addColumn")) },
                                                    leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null) },
                                                    onClick = {
                                                        kanbanMenuOpen = false
                                                        coreAccounts.forEach { account ->
                                                            scope.launch {
                                                                runCatching {
                                                                    withContext(ioDispatcher) {
                                                                        val client = MobileMailCommandClient(core)
                                                                        loadAccountFolders(client, account)
                                                                    }
                                                                }.onSuccess { foldersByAccount = foldersByAccount + (account.id to it) }
                                                            }
                                                        }
                                                        showKanbanColumnDialog = true
                                                    },
                                                )
                                                DropdownMenuItem(
                                                    text = { Text(tr("kanban.actions.boardOptions")) },
                                                    enabled = activeKanbanBoard != null,
                                                    onClick = {
                                                        kanbanMenuOpen = false
                                                        activeKanbanBoard?.let { board ->
                                                            kanbanSettingsTargetId = board.id
                                                            previousTopScreen = Screen.Kanban
                                                            screen = Screen.Settings
                                                        }
                                                    },
                                                )
                                            }
                                        }
                                    }
                                },
                            )
                        },
                        floatingActionButton = {
                            if (coreAccounts.any { !accountSummaryIsRss(it) }) {
                                ExtendedFloatingActionButton(
                                    onClick = ::openCompose,
                                    icon = { Icon(Icons.Filled.Edit, contentDescription = tr("mobile.tabs.compose")) },
                                    text = { Text(tr("mobile.tabs.compose")) },
                                )
                            }
                        },
                        snackbarHost = { SnackbarHost(snackbarHost) },
                    ) { innerPadding ->
                        KanbanScreen(
                            modifier = Modifier.fillMaxSize().padding(innerPadding),
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            accountsLoading = !initialAccountsLoaded || accountsLoading,
                            board = activeKanbanBoard,
                            columns = kanbanColumns,
                            foldersByAccount = foldersByAccount,
                            filter = kanbanFilter,
                            search = kanbanSearch,
                            searchScope = kanbanSearchScope,
                            onOpen = { thread, column -> readCoreThread(thread, sourceFolder = column.folderId) },
                            selectedThreadIds = selectedMailThreadIds,
                            selectionActive = mailSelectionActive,
                            onToggleSelected = { thread ->
                                selectedMailThreadIds =
                                    if (thread.id in selectedMailThreadIds) {
                                        selectedMailThreadIds - thread.id
                                    } else {
                                        selectedMailThreadIds + thread.id
                                    }
                                if (selectedMailThreadIds.isEmpty()) {
                                    mailSelectionMenuOpen = false
                                }
                            },
                            onLongPress = { thread ->
                                selectedMailThreadIds =
                                    if (thread.id in selectedMailThreadIds) {
                                        selectedMailThreadIds
                                    } else {
                                        selectedMailThreadIds + thread.id
                                    }
                            },
                            onToggleStar = ::toggleStar,
                            onRefreshColumn = { loadKanbanColumn(it, refresh = true) },
                            onLoadMoreColumn = ::loadMoreKanbanColumn,
                            onMarkColumnAllRead = ::markKanbanColumnAllRead,
                            onRemoveColumn = ::removeKanbanColumn,
                            onMoveColumn = ::moveKanbanColumn,
                            onSearchColumn = { column ->
                                persistKanbanSearchScope(kanbanColumnKey(column))
                                if (kanbanSearch.isNotBlank()) loadKanbanBoard(refresh = true)
                            },
                            onAddColumn = { showKanbanColumnDialog = true },
                            showSenderImages = showSenderImages,
                            kanbanColumnWidth = kanbanColumnWidth.dp,
                        )
                    }
                }
            }

            composable(AppRoutes.Mail) {
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    drawerContent = {
                        MailDrawer(
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            selectedAccountId = selectedCoreAccountId,
                            folders = drawerFolders,
                            currentScreen = screen,
                            showUnreadBadges = showUnreadBadges,
                            showUnifiedInboxNav = showUnifiedInboxNav,
                            showStarredNav = showStarredNav,
                            kanbanBoards = kanbanBoards,
                            activeKanbanBoardId = activeKanbanBoardId,
                            onSelectUnified = {
                                if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                                    selectCoreMailbox(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                                    syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER)
                                }
                                screen = Screen.Mail
                                scope.launch { drawerState.close() }
                            },
                            onSelectAccount = { account ->
                                if (selectedCoreAccountId != account.id) {
                                    selectCoreMailbox(account.id, INBOX_FOLDER)
                                    syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER)
                                }
                                screen = Screen.Mail
                                scope.launch { drawerState.close() }
                            },
                            onSelectStarred = {
                                screen = Screen.Starred
                                previousTopScreen = Screen.Starred
                                loadStarredItems()
                                scope.launch { drawerState.close() }
                            },
                            onSelectKanban = {
                                screen = Screen.Kanban
                                previousTopScreen = Screen.Kanban
                                loadKanbanBoard(refresh = false)
                                scope.launch { drawerState.close() }
                            },
                            onSelectKanbanBoard = { board ->
                                activeKanbanBoardId = board.id
                                saveActiveKanbanBoardId(kanbanPrefs, board.id)
                                screen = Screen.Kanban
                                previousTopScreen = Screen.Kanban
                                loadKanbanBoard(refresh = false)
                                scope.launch { drawerState.close() }
                            },
                            onAddAccount = {
                                addSection = 0
                                passwordServerSettingsOpen = false
                                previousTopScreen = screen
                                screen = Screen.AddAccount
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                previousTopScreen = screen
                                screen = Screen.Settings
                                scope.launch { drawerState.close() }
                            },
                            onShowAbout = {
                                showAboutDialog = true
                                scope.launch { drawerState.close() }
                            },
                            googleReauthAccountId = googleReauthAccountId,
                            onReconnectGoogle = {
                                connectGoogleDeviceAccount()
                                scope.launch { drawerState.close() }
                            },
                        )
                    },
                ) {
                    Scaffold(
                        topBar = {
                            TopAppBar(
                                title = {
                                    if (mailSelectionActive) {
                                        MailSelectionTitle(selectedMailThreads.size, height = 44.dp)
                                    } else {
                                        MailHeaderSearchField(
                                            search = mailSearch,
                                            onSearchChange = { mailSearch = it },
                                            onSearchSubmit = { syncCoreThreads(syncFirst = false) },
                                        )
                                    }
                                },
                                navigationIcon = {
                                    if (mailSelectionActive) {
                                        IconButton(onClick = {
                                            selectedMailThreadIds = emptySet()
                                            mailSelectionMenuOpen = false
                                        }) {
                                            Icon(Icons.Filled.Close, contentDescription = tr("mobile.actions.clearSelection"))
                                        }
                                    } else {
                                        IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                            Icon(Icons.Filled.Menu, contentDescription = tr("mobile.actions.openNavigation"))
                                        }
                                    }
                                },
                                actions = {
                                    if (mailSelectionActive) {
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::archiveOrRemove)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Archive, contentDescription = tr("threads.actions.archiveThread"))
                                        }
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::deleteThread)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Delete, contentDescription = tr("buttons.delete"))
                                        }
                                        Box {
                                            IconButton(onClick = { mailSelectionMenuOpen = true }) {
                                                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"))
                                            }
                                            DropdownMenu(
                                                expanded = mailSelectionMenuOpen,
                                                onDismissRequest = { mailSelectionMenuOpen = false },
                                            ) {
                                                val markRead = selectedMailThreads.any { it.unread }
                                                DropdownMenuItem(
                                                    text = { Text(if (markRead) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread")) },
                                                    leadingIcon = { Icon(Icons.Filled.MarkEmailUnread, contentDescription = null) },
                                                    onClick = {
                                                        mailSelectionMenuOpen = false
                                                        selectedMailThreads
                                                            .filter { it.unread == markRead }
                                                            .forEach(::toggleRead)
                                                        selectedMailThreadIds = emptySet()
                                                    },
                                                )
                                                val star = selectedMailThreads.any { !it.starred }
                                                DropdownMenuItem(
                                                    text = { Text(if (star) tr("chat.star") else tr("chat.unstar")) },
                                                    leadingIcon = {
                                                        Icon(
                                                            if (star) Icons.Filled.StarBorder else Icons.Filled.Star,
                                                            contentDescription = null,
                                                        )
                                                    },
                                                    onClick = {
                                                        mailSelectionMenuOpen = false
                                                        selectedMailThreads
                                                            .filter { it.starred != star }
                                                            .forEach(::toggleStar)
                                                        selectedMailThreadIds = emptySet()
                                                    },
                                                )
                                                val singleSelectedMailThread =
                                                    selectedMailThreads
                                                        .singleOrNull()
                                                        ?.takeUnless { threadIdIsRss(it.id) }
                                                if (singleSelectedMailThread != null) {
                                                    HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                    DropdownMenuItem(
                                                        text = { Text(tr("threads.actions.moveTo")) },
                                                        leadingIcon = { Icon(Icons.Outlined.FolderOpen, contentDescription = null) },
                                                        onClick = {
                                                            mailSelectionMenuOpen = false
                                                            ensureThreadActionFolders(
                                                                thread = singleSelectedMailThread,
                                                                includeAllMailAccounts = false,
                                                            ) {
                                                                selectedMailMoveThread = singleSelectedMailThread
                                                            }
                                                        },
                                                    )
                                                    DropdownMenuItem(
                                                        text = { Text(tr("threads.actions.copyTo")) },
                                                        leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                                                        onClick = {
                                                            mailSelectionMenuOpen = false
                                                            ensureThreadActionFolders(
                                                                thread = singleSelectedMailThread,
                                                                includeAllMailAccounts = true,
                                                            ) {
                                                                selectedMailCopyThread = singleSelectedMailThread
                                                            }
                                                        },
                                                    )
                                                }
                                            }
                                        }
                                    } else {
                                        Box {
                                            IconButton(onClick = { mailboxMenuOpen = true }) {
                                                Icon(Icons.Filled.MoreVert, contentDescription = tr("threads.actions.title"))
                                            }
                                            DropdownMenu(expanded = mailboxMenuOpen, onDismissRequest = { mailboxMenuOpen = false }) {
                                                val loadedUnread = coreThreads.any { it.unread }
                                                val folderUnreadTotal =
                                                    if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                                                        val includedAccountIds =
                                                            coreAccounts.filter { it.includedInUnified }.map { it.id }.toSet()
                                                        coreFolders
                                                            .filter { it.accountId in includedAccountIds }
                                                            .groupBy { it.accountId }
                                                            .values
                                                            .sumOf { accountFolders -> folderUnread(accountFolders, INBOX_FOLDER) }
                                                    } else {
                                                        folderUnread(
                                                            coreFolders.filter { it.accountId == selectedCoreAccountId },
                                                            selectedCoreFolder,
                                                        )
                                                    }
                                                val showMarkAllRead = folderUnreadTotal > 0 || loadedUnread
                                                val showAccountActions = selectedAccount != null
                                                FilterMode.values().forEach { mode ->
                                                    DropdownMenuItem(
                                                        text = { Text(mode.label()) },
                                                        leadingIcon = {
                                                            RadioButton(
                                                                selected = mailFilter == mode,
                                                                onClick = null,
                                                            )
                                                        },
                                                        onClick = {
                                                            mailboxMenuOpen = false
                                                            mailFilter = mode
                                                            syncCoreThreads(syncFirst = false)
                                                        },
                                                    )
                                                }
                                                if (showMarkAllRead || showAccountActions) {
                                                    HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                }
                                                if (showMarkAllRead) {
                                                    DropdownMenuItem(
                                                        text = { Text(tr("threads.actions.markAllAsRead")) },
                                                        leadingIcon = {
                                                            Icon(Icons.Filled.MarkEmailUnread, contentDescription = null)
                                                        },
                                                        onClick = {
                                                            mailboxMenuOpen = false
                                                            markVisibleMailboxAllRead()
                                                        },
                                                    )
                                                }
                                                if (showAccountActions) {
                                                    if (showMarkAllRead) {
                                                        HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                    }
                                                    DropdownMenuItem(
                                                        text = { Text(tr("settings.account.accountSettings")) },
                                                        onClick = {
                                                            mailboxMenuOpen = false
                                                            accountSettingsTargetId = selectedAccount.id
                                                            previousTopScreen = Screen.Mail
                                                            screen = Screen.Settings
                                                        },
                                                    )
                                                    if (selectedAccount.let(::accountSummaryIsRss)) {
                                                        HorizontalDivider(Modifier.padding(vertical = 4.dp))
                                                        DropdownMenuItem(
                                                            text = { Text(tr("feeds.actions.addFeed")) },
                                                            onClick = {
                                                                mailboxMenuOpen = false
                                                                addFeedUrl = ""
                                                                showAddFeedDialog = true
                                                            },
                                                        )
                                                        DropdownMenuItem(
                                                            text = { Text(tr("common.import")) },
                                                            onClick = {
                                                                mailboxMenuOpen = false
                                                                services.pickFile(
                                                                    listOf("text/xml", "application/xml", "text/*", "*/*"),
                                                                    importOpml,
                                                                )
                                                            },
                                                        )
                                                        DropdownMenuItem(
                                                            text = { Text(tr("common.export")) },
                                                            onClick = {
                                                                mailboxMenuOpen = false
                                                                exportOpmlForSelectedAccount()
                                                            },
                                                        )
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                            )
                        },
                        floatingActionButton = {
                            if (coreAccounts.isNotEmpty()) {
                                ExtendedFloatingActionButton(
                                    onClick = ::openCompose,
                                    icon = { Icon(Icons.Filled.Edit, contentDescription = tr("mobile.tabs.compose")) },
                                    text = { Text(tr("mobile.tabs.compose")) },
                                )
                            }
                        },
                        snackbarHost = { SnackbarHost(snackbarHost) },
                    ) { innerPadding ->
                        Column(Modifier.fillMaxSize().padding(innerPadding)) {
                            val reconnectAccount2 = selectedAccount?.takeIf { it.needsReconnect }
                            when {
                                reconnectAccount2 != null -> {
                                    StatusBanner(
                                        message = "Can't sign in to ${reconnectAccount2.email.ifBlank {
                                            reconnectAccount2.displayName
                                        }}. Update the credentials to reconnect.",
                                        isError = true,
                                        actionLabel = "Reconnect",
                                        onAction = { reconnectAccount(reconnectAccount2) },
                                        onDismiss = null,
                                    )
                                }

                                errorBanner != null -> {
                                    val authLike = isAuthError(errorBanner!!)
                                    StatusBanner(
                                        message = errorBanner!!,
                                        isError = true,
                                        actionLabel = if (authLike && selectedAccount != null) "Reconnect" else "Retry",
                                        onAction = {
                                            if (authLike && selectedAccount != null) {
                                                reconnectAccount(selectedAccount)
                                            } else {
                                                syncCoreThreads()
                                            }
                                        },
                                        onDismiss = { errorBanner = null },
                                    )
                                }

                                !notificationPermissionGranted &&
                                    !notificationBannerDismissed &&
                                    coreAccounts.isNotEmpty() &&
                                    initialThreadsLoaded &&
                                    !syncing -> {
                                    StatusBanner(
                                        message = tr("mobile.mail.notificationsBannerText"),
                                        isError = false,
                                        icon = Icons.Outlined.NotificationsNone,
                                        actionLabel = tr("mobile.mail.notificationsBannerEnable"),
                                        onAction = {
                                            mobileHost.requestNotificationPermission()
                                            notificationPermissionGranted = mobileHost.notificationsEnabled()
                                        },
                                        onDismiss = {
                                            notificationBannerDismissed = true
                                            saveAppBoolean(prefs, NOTIFICATION_BANNER_DISMISSED_PREF, true)
                                        },
                                    )
                                }
                            }
                            val showingBlockingInboxLoad =
                                !initialAccountsLoaded ||
                                    accountsLoading ||
                                    (coreThreads.isEmpty() && (syncing || !initialThreadsLoaded))
                            LaunchedEffect(
                                screen,
                                initialAccountsLoaded,
                                accountsLoading,
                                coreAccounts.size,
                                coreThreads.size,
                                syncing,
                                initialThreadsLoaded,
                                selectedCoreAccountId,
                                selectedCoreFolder,
                            ) {
                                if (screen == Screen.Mail) {
                                    Log.i(
                                        "MailLoad",
                                        "render mail blocking=$showingBlockingInboxLoad initialAccountsLoaded=$initialAccountsLoaded accountsLoading=$accountsLoading accounts=${coreAccounts.size} threads=${coreThreads.size} syncing=$syncing initialThreadsLoaded=$initialThreadsLoaded selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                                    )
                                }
                            }
                            LaunchedEffect(
                                screen,
                                showingBlockingInboxLoad,
                                activeMailboxLoadKey,
                                activeMailboxLoadStartedAtMillis,
                            ) {
                                if (screen != Screen.Mail || !showingBlockingInboxLoad) return@LaunchedEffect
                                delay(MAILBOX_BLOCKING_WARN_AFTER_MS)
                                val stillBlocking =
                                    screen == Screen.Mail &&
                                        (
                                            !initialAccountsLoaded ||
                                                accountsLoading ||
                                                (coreThreads.isEmpty() && (syncing || !initialThreadsLoaded))
                                        )
                                if (stillBlocking && !blockingMailboxLoadWarned) {
                                    blockingMailboxLoadWarned = true
                                    Log.w(
                                        "MailLoad",
                                        "mail UI still blocking after ${MAILBOX_BLOCKING_WARN_AFTER_MS}ms accountsLoaded=$initialAccountsLoaded accountsLoading=$accountsLoading accounts=${coreAccounts.size} threads=${coreThreads.size} syncing=$syncing initialThreadsLoaded=$initialThreadsLoaded activeLoad=$activeMailboxLoadKey selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                                    )
                                }
                                delay(MAILBOX_BLOCKING_TIMEOUT_MS - MAILBOX_BLOCKING_WARN_AFTER_MS)
                                val stillBlocked = {
                                    screen == Screen.Mail &&
                                        coreAccounts.isNotEmpty() &&
                                        coreThreads.isEmpty() &&
                                        (syncing || !initialThreadsLoaded)
                                }
                                if (stillBlocked() && syncing) {
                                    // The sync request is still in flight — a slow
                                    // first sync, not a failure. Switch the loader
                                    // copy and give it until the hard cap before
                                    // treating it as an error.
                                    Log.w(
                                        "MailLoad",
                                        "mail sync still running after ${MAILBOX_BLOCKING_TIMEOUT_MS}ms accounts=${coreAccounts.size} activeLoad=$activeMailboxLoadKey selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                                    )
                                    blockingMailboxLoadSlow = true
                                    delay(MAILBOX_BLOCKING_HARD_TIMEOUT_MS - MAILBOX_BLOCKING_TIMEOUT_MS)
                                }
                                if (stillBlocked()) {
                                    Log.w(
                                        "MailLoad",
                                        "mail UI unblocked accounts=${coreAccounts.size} syncing=$syncing initialThreadsLoaded=$initialThreadsLoaded activeLoad=$activeMailboxLoadKey selectedAccount=$selectedCoreAccountId folder=$selectedCoreFolder",
                                    )
                                    syncing = false
                                    initialThreadsLoaded = true
                                    blockingMailboxLoadSlow = false
                                    errorBanner = "Inbox load timed out. Pull to refresh or tap Retry."
                                }
                            }
                            PullToRefreshBox(
                                isRefreshing = syncing && !showingBlockingInboxLoad,
                                onRefresh = { syncCoreThreads() },
                                modifier = Modifier.fillMaxSize(),
                            ) {
                                when {
                                    !initialAccountsLoaded || accountsLoading -> {
                                        LoadingState("Loading your inbox…")
                                    }

                                    coreAccounts.isEmpty() -> {
                                        EmptyState(
                                            icon = Icons.Filled.PersonAdd,
                                            title = tr("empty.welcomeTitle"),
                                            text = tr("empty.mailOrRssSetupText"),
                                            actionLabel = tr("accounts.actions.addAccount"),
                                            onAction = {
                                                addSection = 0
                                                passwordServerSettingsOpen = false
                                                screen = Screen.AddAccount
                                            },
                                        )
                                    }

                                    coreThreads.isEmpty() && (syncing || !initialThreadsLoaded) -> {
                                        LoadingState(
                                            if (blockingMailboxLoadSlow) {
                                                "Still syncing your inbox… The first sync can take a while."
                                            } else {
                                                "Loading your inbox…"
                                            },
                                        )
                                    }

                                    coreThreads.isEmpty() -> {
                                        EmptyState(
                                            icon = Icons.Outlined.Drafts,
                                            title =
                                                if (mailSearch.isBlank() &&
                                                    mailFilter == FilterMode.All
                                                ) {
                                                    tr("empty.nothingHereYet")
                                                } else {
                                                    tr("empty.noMatchingMail")
                                                },
                                            text =
                                                if (mailSearch.isBlank() && mailFilter == FilterMode.All) {
                                                    tr("empty.pullLatestMessages")
                                                } else {
                                                    tr("empty.adjustSearchFilter")
                                                },
                                            actionLabel = tr("mobile.mail.syncMailbox"),
                                            onAction = ::syncCoreThreads,
                                        )
                                    }

                                    else -> {
                                        MailList(
                                            threads = coreThreads,
                                            accounts = coreAccounts,
                                            canLoadMore = pageableCoreAccounts().isNotEmpty(),
                                            loadingMore = loadingMoreThreads,
                                            onOpen = ::readCoreThread,
                                            onToggleStar = ::toggleStar,
                                            onArchive = ::archiveOrRemove,
                                            onDelete = ::deleteThread,
                                            onCopyFeedUrl = { thread ->
                                                services.copyText("Feed URL", thread.feedUrl)
                                                status = "Copied feed URL"
                                            },
                                            selectedThreadIds = selectedMailThreadIds,
                                            selectionActive = mailSelectionActive,
                                            onToggleSelected = { thread ->
                                                selectedMailThreadIds =
                                                    if (thread.id in selectedMailThreadIds) {
                                                        selectedMailThreadIds - thread.id
                                                    } else {
                                                        selectedMailThreadIds + thread.id
                                                    }
                                                if (selectedMailThreadIds.isEmpty()) {
                                                    mailSelectionMenuOpen = false
                                                }
                                            },
                                            onLongPress = { thread ->
                                                selectedMailThreadIds =
                                                    if (thread.id in selectedMailThreadIds) {
                                                        selectedMailThreadIds
                                                    } else {
                                                        selectedMailThreadIds + thread.id
                                                    }
                                            },
                                            onLoadMore = { userInitiated ->
                                                loadMoreCoreThreads(quiet = !userInitiated)
                                            },
                                            showSenderImages = showSenderImages,
                                            showAccountBadge = selectedCoreAccountId == UNIFIED_ACCOUNT_ID,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if (showAddFeedDialog) {
            AlertDialog(
                onDismissRequest = { showAddFeedDialog = false },
                title = { Text(tr("feeds.actions.addFeed")) },
                text = {
                    OutlinedTextField(
                        value = addFeedUrl,
                        onValueChange = { addFeedUrl = it },
                        label = { Text(tr("feeds.url")) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                },
                confirmButton = {
                    TextButton(onClick = ::addFeedToSelectedRssAccount) {
                        Text(tr("common.add"))
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showAddFeedDialog = false }) {
                        Text(tr("buttons.cancel"))
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
            AlertDialog(
                onDismissRequest = { showKanbanCreateFolderDialog = null },
                title = { Text(tr("folders.create")) },
                text = {
                    OutlinedTextField(
                        value = kanbanFolderNameInput,
                        onValueChange = { kanbanFolderNameInput = it },
                        label = { Text(tr("folders.namePlaceholder")) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                },
                confirmButton = {
                    TextButton(onClick = { createFolderForKanban(account, kanbanFolderNameInput) }) {
                        Text(tr("folders.create"))
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showKanbanCreateFolderDialog = null }) {
                        Text(tr("buttons.cancel"))
                    }
                },
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
