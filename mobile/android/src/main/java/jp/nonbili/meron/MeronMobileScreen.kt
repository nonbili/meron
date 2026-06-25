package jp.nonbili.meron

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Base64
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
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
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.FileProvider
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
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
import jp.nonbili.meron.shared.SharedMobileContract
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
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.URL
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID
import kotlin.math.abs

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MeronMobileScreen(
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft?,
    incomingOAuthCallbackUrl: String?,
    appearanceMode: AppAppearanceMode,
    onAppearanceModeChange: (AppAppearanceMode) -> Unit,
) {
    val context = LocalContext.current
    val appVersion = remember { appVersionName(context) }
    val scope = rememberCoroutineScope()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val state = remember { MeronMobileState(context, scope) }
    with(state) {
        val notificationPermissionLauncher =
            rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
                notificationPermissionGranted = granted
                status = if (granted) "Notifications enabled" else "Notifications are disabled"
            }
        val googleAccountPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
                val name = result.data?.getStringExtra(android.accounts.AccountManager.KEY_ACCOUNT_NAME)
                if (result.resultCode == android.app.Activity.RESULT_OK && !name.isNullOrBlank()) {
                    onGoogleDeviceAccountPicked(name)
                } else {
                    status = "No Google account selected"
                }
            }
        launchGoogleAccountPicker = { googleAccountPicker.launch(GoogleAccountManagerAuth.chooseAccountIntent()) }
        val opmlImportPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
                if (uri != null) {
                    runCatching {
                        context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    runCatching {
                        context.contentResolver.openInputStream(uri).use { stream ->
                            stream?.readBytes()?.decodeToString().orEmpty()
                        }
                    }.onSuccess { opml ->
                        val accountId = selectedCoreAccountId
                        if (accountId == UNIFIED_ACCOUNT_ID || accountId.isBlank()) {
                            status = "Select an RSS account first."
                            return@onSuccess
                        }
                        if (!MeronCoreNative.isLoaded()) {
                            status = "Rust core not packaged."
                            return@onSuccess
                        }
                        scope.launch {
                            runCatching {
                                withContext(Dispatchers.IO) {
                                    val client = MobileMailCommandClient(JniMeronCore())
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
                                coreThreads = parseThreadListResponse(threadsJson)
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
        val opmlExportPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/xml")) { uri ->
                if (uri != null) {
                    runCatching {
                        context.contentResolver.openOutputStream(uri).use { stream ->
                            stream?.write(pendingOpmlExport.toByteArray())
                        }
                    }.onSuccess {
                        status = "Exported OPML"
                        pendingOpmlExport = ""
                    }.onFailure {
                        status = "OPML export failed: ${it.message}"
                    }
                }
            }
        val attachmentSavePicker =
            rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
                val attachment = pendingAttachmentSave
                pendingAttachmentSave = null
                if (uri != null && attachment != null) {
                    scope.launch {
                        runCatching {
                            withContext(Dispatchers.IO) {
                                val response = MobileMailCommandClient(JniMeronCore()).readAttachment(AttachmentReadParams(attachment.key))
                                val data = parseAttachmentDataResponse(response)
                                if (data.isBlank()) error("Attachment data is empty")
                                val bytes = Base64.decode(data, Base64.DEFAULT)
                                context.contentResolver.openOutputStream(uri).use { stream ->
                                    stream?.write(bytes) ?: error("Could not open destination")
                                }
                            }
                        }.onSuccess {
                            status = "Saved ${attachment.filename.ifBlank { "attachment" }}"
                        }.onFailure {
                            status = "Attachment save failed: ${it.message}"
                        }
                    }
                }
            }
        val attachmentPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
                if (uri != null) {
                    runCatching {
                        context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    runCatching {
                        context.contentResolver.openInputStream(uri).use { stream ->
                            val bytes = stream?.readBytes() ?: ByteArray(0)
                            val name = context.displayNameFor(uri)
                            DraftAttachment(
                                id = uri.toString(),
                                displayName = name,
                                mimeType = context.contentResolver.getType(uri) ?: "application/octet-stream",
                                sizeBytes = bytes.size.toLong(),
                                dataBase64 = Base64.encodeToString(bytes, Base64.NO_WRAP),
                            )
                        }
                    }.onSuccess {
                        attachments = attachments + it
                        status = "Attached ${it.displayName}"
                    }.onFailure {
                        status = "Attachment failed: ${it.message}"
                    }
                }
            }
        val quickReplyAttachmentPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
                if (uri != null) {
                    runCatching {
                        context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    runCatching {
                        context.contentResolver.openInputStream(uri).use { stream ->
                            val bytes = stream?.readBytes() ?: ByteArray(0)
                            val name = context.displayNameFor(uri)
                            DraftAttachment(
                                id = uri.toString(),
                                displayName = name,
                                mimeType = context.contentResolver.getType(uri) ?: "application/octet-stream",
                                sizeBytes = bytes.size.toLong(),
                                dataBase64 = Base64.encodeToString(bytes, Base64.NO_WRAP),
                            )
                        }
                    }.onSuccess {
                        quickReplyAttachments = quickReplyAttachments + it
                        status = "Attached ${it.displayName}"
                    }.onFailure {
                        status = "Attachment failed: ${it.message}"
                    }
                }
            }
        val accountMediaPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
                val target = accountMediaUploadTarget
                accountMediaUploadTarget = null
                if (uri != null && target != null) {
                    runCatching {
                        context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    runCatching {
                        context.contentResolver.openInputStream(uri).use { stream ->
                            val bytes = stream?.readBytes() ?: ByteArray(0)
                            AccountMediaFileParams(
                                accountId = target.account.id,
                                filename = context.displayNameFor(uri),
                                mime = context.contentResolver.getType(uri) ?: "application/octet-stream",
                                data = Base64.encodeToString(bytes, Base64.NO_WRAP),
                            )
                        }
                    }.onSuccess { params ->
                        if (!MeronCoreNative.isLoaded()) {
                            status = "Rust core not packaged."
                            return@onSuccess
                        }
                        scope.launch {
                            runCatching {
                                withContext(Dispatchers.IO) {
                                    val client = MobileMailCommandClient(JniMeronCore())
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
        val kanbanBoardMediaPicker =
            rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
                val target = kanbanBoardMediaTarget
                kanbanBoardMediaTarget = null
                if (uri != null && target != null) {
                    runCatching {
                        context.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    val mediaUrl = uri.toString()
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
        launchOpmlExport = { opmlExportPicker.launch(it) }
        launchAttachmentSave = { attachmentSavePicker.launch(it) }

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
                screen = Screen.Compose
                status = "Loaded compose draft from mailto link"
            }
        }

        LaunchedEffect(incomingOAuthCallbackUrl) {
            incomingOAuthCallbackUrl?.let { rawUrl ->
                runCatching {
                    parseOAuthCallbackUrlForRedirect(
                        rawUrl = rawUrl,
                        expectedState = oauthState,
                        redirectUri = oauthRedirectUri.trim(),
                    )
                }.onSuccess { result ->
                    if (result != null) {
                        oauthAuthorizationCode = result.code
                        addSection = 1
                        screen = Screen.AddAccount
                        status = "Finishing ${oauthProvider.replaceFirstChar { it.uppercase() }} sign-in..."
                        exchangeOAuthCode()
                    }
                }.onFailure {
                    status = "OAuth callback failed: ${it.message}"
                }
            }
        }

        // Load persisted accounts once on startup so they survive app restarts.
        LaunchedEffect(Unit) {
            if (MeronCoreNative.isLoaded() && coreAccounts.isEmpty()) {
                listAccounts()
                loadStorageUsage()
            } else if (!MeronCoreNative.isLoaded()) {
                initialAccountsLoaded = true
            }
        }

        // Once accounts are known, surface whatever the local store already holds so
        // a cold start shows the cached inbox instead of an empty "Nothing here yet".
        // A server sync still happens on pull-to-refresh / "Sync now".
        LaunchedEffect(coreAccounts) {
            if (coreAccounts.isNotEmpty() && coreThreads.isEmpty()) {
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
                    kanbanColumns.values.flatMap { it.threads }.map { it.id }.toSet()
                } else {
                    coreThreads.map { it.id }.toSet()
                }
            val retainedThreadIds = selectedMailThreadIds.intersect(visibleThreadIds)
            if (retainedThreadIds.size != selectedMailThreadIds.size) {
                selectedMailThreadIds = retainedThreadIds
            }
        }

        // Hardware back from a sub-screen returns to the inbox instead of exiting.
        BackHandler(enabled = (screen == Screen.Mail || screen == Screen.Kanban) && mailSelectionActive) {
            selectedMailThreadIds = emptySet()
            mailSelectionMenuOpen = false
        }
        BackHandler(enabled = screen != Screen.Mail && !(screen == Screen.Kanban && mailSelectionActive)) {
            screen =
                if (screen == Screen.Thread || screen == Screen.Compose || screen == Screen.AddAccount ||
                    screen == Screen.Settings
                ) {
                    previousTopScreen
                } else {
                    Screen.Mail
                }
        }

        when (screen) {
            Screen.Thread -> {
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
                    onBack = { screen = previousTopScreen },
                    onArchive = {
                        selectedCoreThread?.let {
                            archiveOrRemove(it)
                            screen = previousTopScreen
                        }
                    },
                    onDelete = {
                        selectedCoreThread?.let {
                            deleteThread(it)
                            screen = previousTopScreen
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
                                screen = previousTopScreen
                            }
                        }
                    },
                    onCreateFolderAndMove = { name ->
                        selectedCoreThread?.let { thread ->
                            createFolderAndMoveThread(thread, name) {
                                screen = previousTopScreen
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
                    onQuickReplyAttach = { quickReplyAttachmentPicker.launch(arrayOf("*/*")) },
                    onRemoveQuickReplyAttachment = { attachment ->
                        quickReplyAttachments = quickReplyAttachments.filterNot { it.id == attachment.id }
                        quickReplyFailure = ""
                    },
                    onOpenFullReply = ::openQuickReplyInFullEditor,
                    onSendReply = ::sendQuickReply,
                    onForward = { openMessageCompose(it, forward = true) },
                    onEditAsNew = { openMessageCompose(it, forward = false) },
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
                        screen = Screen.Compose
                    },
                    onCopyMessageText = { label, value ->
                        copyToClipboard(context, label, value)
                        status = "Copied ${label.lowercase()}"
                    },
                )
            }

            Screen.Compose -> {
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
                    onAttach = { attachmentPicker.launch(arrayOf("*/*")) },
                    onClearAttachments = { attachments = emptyList() },
                    sendShortcutMode = sendShortcutMode,
                    onSaveDraft = ::saveComposeDraft,
                    onDiscardDraft = ::discardComposeDraft,
                    onSend = ::sendMail,
                    onBack = { screen = previousTopScreen },
                )
            }

            Screen.AddAccount -> {
                AddAccountScreen(
                    onBack = { screen = previousTopScreen },
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
                    onAutodiscover = ::autodiscoverPasswordAccount,
                    onAddPassword = ::addPasswordAccount,
                    oauthProvider = oauthProvider,
                    onOauthProviderChange = { oauthProvider = it },
                    oauthEmail = oauthEmail,
                    onOauthEmailChange = { oauthEmail = it },
                    oauthClientId = oauthClientId,
                    onOauthClientIdChange = { oauthClientId = it },
                    oauthClientSecret = oauthClientSecret,
                    onOauthClientSecretChange = { oauthClientSecret = it },
                    oauthRedirectUri = oauthRedirectUri,
                    onOauthRedirectUriChange = { oauthRedirectUri = it },
                    oauthAuthorizationCode = oauthAuthorizationCode,
                    oauthAccessToken = oauthAccessToken,
                    onOauthAccessTokenChange = { oauthAccessToken = it },
                    oauthRefreshToken = oauthRefreshToken,
                    onOauthRefreshTokenChange = { oauthRefreshToken = it },
                    oauthExpiresAt = oauthExpiresAt,
                    onOauthExpiresAtChange = { oauthExpiresAt = it },
                    onLaunchOAuth = ::launchOAuthFlow,
                    onConnectGoogleDeviceAccount = ::connectGoogleDeviceAccount,
                    onExchangeOAuth = ::exchangeOAuthCode,
                    onAddOAuth = ::addOAuthAccount,
                    rssFeedUrl = rssFeedUrl,
                    onRssFeedUrlChange = { rssFeedUrl = it },
                    rssDisplayName = rssDisplayName,
                    onRssDisplayNameChange = { rssDisplayName = it },
                    onAddRss = ::addRssAccount,
                    diagnostics = coreStatus(coreInitJson),
                )
            }

            Screen.Settings -> {
                LaunchedEffect(Unit) { loadStorageUsage() }
                SettingsScreen(
                    onBack = { screen = previousTopScreen },
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
                        previousTopScreen = Screen.Settings
                        screen = Screen.AddAccount
                    },
                    onAddFeedAccount = {
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
                        accountMediaPicker.launch(arrayOf("image/*"))
                    },
                    onPickAccountWallpaper = { account ->
                        accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = true)
                        accountMediaPicker.launch(arrayOf("image/*"))
                    },
                    onPickKanbanBoardAvatar = { board ->
                        kanbanBoardMediaTarget = KanbanBoardMediaTarget(board, wallpaper = false)
                        kanbanBoardMediaPicker.launch(arrayOf("image/*"))
                    },
                    onPickKanbanBoardWallpaper = { board ->
                        kanbanBoardMediaTarget = KanbanBoardMediaTarget(board, wallpaper = true)
                        kanbanBoardMediaPicker.launch(arrayOf("image/*"))
                    },
                    onMoveAccountUp = { account -> moveAccount(account, -1) },
                    onMoveAccountDown = { account -> moveAccount(account, 1) },
                    onRemoveAccount = ::removeAccount,
                    appearanceMode = appearanceMode,
                    onAppearanceModeChange = onAppearanceModeChange,
                    showSenderImages = showSenderImages,
                    onToggleSenderImages = {
                        showSenderImages = !showSenderImages
                        saveAppBoolean(context, SHOW_SENDER_IMAGES_PREF, showSenderImages)
                    },
                    showUnreadBadges = showUnreadBadges,
                    onToggleUnreadBadges = {
                        showUnreadBadges = !showUnreadBadges
                        saveAppBoolean(context, SHOW_UNREAD_BADGES_PREF, showUnreadBadges)
                    },
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    onToggleUnifiedInboxNav = {
                        showUnifiedInboxNav = !showUnifiedInboxNav
                        saveAppBoolean(context, SHOW_UNIFIED_INBOX_PREF, showUnifiedInboxNav)
                    },
                    showStarredNav = showStarredNav,
                    onToggleStarredNav = {
                        showStarredNav = !showStarredNav
                        saveAppBoolean(context, SHOW_STARRED_NAV_PREF, showStarredNav)
                    },
                    sendShortcutMode = sendShortcutMode,
                    onToggleSendShortcut = {
                        val next = sendShortcutMode.next()
                        sendShortcutMode = next
                        saveSendShortcutMode(context, next)
                    },
                    kanbanColumnWidth = kanbanColumnWidth,
                    onCycleKanbanColumnWidth = {
                        val next = nextKanbanColumnWidth(kanbanColumnWidth)
                        kanbanColumnWidth = next
                        saveAppInt(context, KANBAN_COLUMN_WIDTH_PREF, next)
                    },
                    notificationsNeedPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted,
                    onEnableNotifications = { notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS) },
                    onRefreshBackground = {
                        AndroidBackgroundSyncScheduler.runOnce(context)
                        status = "Queued background refresh"
                    },
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    onRefreshStorage = { loadStorageUsage(showStatus = true) },
                    onClearStorageCache = ::clearStorageCache,
                    appVersion = appVersion,
                    onShowAbout = { showAboutDialog = true },
                )
            }

            Screen.Starred -> {
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    drawerContent = {
                        MailDrawer(
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            selectedAccountId = selectedCoreAccountId,
                            folders = coreFolders,
                            currentScreen = screen,
                            showUnreadBadges = showUnreadBadges,
                            showUnifiedInboxNav = showUnifiedInboxNav,
                            showStarredNav = showStarredNav,
                            kanbanBoards = kanbanBoards,
                            activeKanbanBoardId = activeKanbanBoardId,
                            onSelectUnified = {
                                screen = Screen.Mail
                                syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER, syncFirst = false)
                                scope.launch { drawerState.close() }
                            },
                            onSelectAccount = { account ->
                                screen = Screen.Mail
                                selectedCoreAccountId = account.id
                                selectedCoreFolder = INBOX_FOLDER
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
                                saveActiveKanbanBoardId(context, board.id)
                                screen = Screen.Kanban
                                previousTopScreen = Screen.Kanban
                                loadKanbanBoard(refresh = false)
                                scope.launch { drawerState.close() }
                            },
                            onAddAccount = {
                                addSection = 0
                                previousTopScreen = Screen.Starred
                                screen = Screen.AddAccount
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                previousTopScreen = screen
                                screen = Screen.Settings
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
                                        Text(stringResource(R.string.mobile_tabs_starred), fontWeight = FontWeight.SemiBold)
                                        Text(
                                            stringResource(R.string.mobile_mail_starred_subtitle),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        )
                                    }
                                },
                                navigationIcon = {
                                    IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                        Icon(Icons.Filled.Menu, contentDescription = stringResource(R.string.mobile_actions_open_navigation))
                                    }
                                },
                                actions = {
                                    IconButton(onClick = ::loadStarredItems) {
                                        Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.mobile_actions_refresh_starred))
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
                                    title = stringResource(R.string.empty_no_starred_items),
                                    text = stringResource(R.string.empty_no_starred_items_text),
                                    actionLabel = stringResource(R.string.mobile_actions_refresh),
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

            Screen.Kanban -> {
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    drawerContent = {
                        MailDrawer(
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            selectedAccountId = selectedCoreAccountId,
                            folders = coreFolders,
                            currentScreen = screen,
                            showUnreadBadges = showUnreadBadges,
                            showUnifiedInboxNav = showUnifiedInboxNav,
                            showStarredNav = showStarredNav,
                            kanbanBoards = kanbanBoards,
                            activeKanbanBoardId = activeKanbanBoardId,
                            onSelectUnified = {
                                screen = Screen.Mail
                                if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                                    selectedCoreAccountId = UNIFIED_ACCOUNT_ID
                                    selectedCoreFolder = INBOX_FOLDER
                                    coreThreads = emptyList()
                                    selectedCoreThread = null
                                    messages = emptyList()
                                    syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER)
                                }
                                scope.launch { drawerState.close() }
                            },
                            onSelectAccount = { account ->
                                screen = Screen.Mail
                                if (selectedCoreAccountId != account.id) {
                                    selectedCoreAccountId = account.id
                                    selectedCoreFolder = INBOX_FOLDER
                                    coreFolders = emptyList()
                                    coreThreads = emptyList()
                                    selectedCoreThread = null
                                    messages = emptyList()
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
                                    saveActiveKanbanBoardId(context, board.id)
                                    loadKanbanBoard(refresh = false)
                                }
                                scope.launch { drawerState.close() }
                            },
                            onAddAccount = {
                                addSection = 0
                                previousTopScreen = Screen.Kanban
                                screen = Screen.AddAccount
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                previousTopScreen = screen
                                screen = Screen.Settings
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
                                        Text(
                                            "${selectedMailThreads.size} selected",
                                            fontWeight = FontWeight.SemiBold,
                                        )
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
                                            Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.mobile_actions_clear_selection))
                                        }
                                    } else {
                                        IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                            Icon(Icons.Filled.Menu, contentDescription = stringResource(R.string.mobile_actions_open_navigation))
                                        }
                                    }
                                },
                                actions = {
                                    if (mailSelectionActive) {
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::archiveOrRemove)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Archive, contentDescription = stringResource(R.string.threads_actions_archive_thread))
                                        }
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::deleteThread)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.buttons_delete))
                                        }
                                        Box {
                                            IconButton(onClick = { mailSelectionMenuOpen = true }) {
                                                Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.chat_more_actions))
                                            }
                                            DropdownMenu(
                                                expanded = mailSelectionMenuOpen,
                                                onDismissRequest = { mailSelectionMenuOpen = false },
                                            ) {
                                                val markRead = selectedMailThreads.any { it.unread }
                                                DropdownMenuItem(
                                                    text = { Text(if (markRead) stringResource(R.string.threads_actions_mark_as_read) else stringResource(R.string.threads_actions_mark_as_unread)) },
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
                                                    text = { Text(if (star) stringResource(R.string.chat_star) else stringResource(R.string.chat_unstar)) },
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
                                                    DropdownMenuItem(
                                                        text = { Text(stringResource(R.string.threads_actions_move_to)) },
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
                                                        text = { Text(stringResource(R.string.threads_actions_copy_to)) },
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
                                                Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.kanban_actions_board_options))
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
                                                DropdownMenuItem(
                                                    text = { Text(stringResource(R.string.mobile_actions_refresh_board)) },
                                                    leadingIcon = { Icon(Icons.Filled.Refresh, contentDescription = null) },
                                                    onClick = {
                                                        kanbanMenuOpen = false
                                                        loadKanbanBoard(refresh = true)
                                                    },
                                                )
                                                DropdownMenuItem(
                                                    text = { Text(stringResource(R.string.kanban_actions_add_column)) },
                                                    leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null) },
                                                    onClick = {
                                                        kanbanMenuOpen = false
                                                        coreAccounts.forEach { account ->
                                                            scope.launch {
                                                                runCatching {
                                                                    withContext(Dispatchers.IO) {
                                                                        val client = MobileMailCommandClient(JniMeronCore())
                                                                        loadAccountFolders(client, account)
                                                                    }
                                                                }.onSuccess { foldersByAccount = foldersByAccount + (account.id to it) }
                                                            }
                                                        }
                                                        showKanbanColumnDialog = true
                                                    },
                                                )
                                                DropdownMenuItem(
                                                    text = { Text(stringResource(R.string.kanban_actions_board_options)) },
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
                                    icon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                                    text = { Text(stringResource(R.string.mobile_tabs_compose)) },
                                )
                            }
                        },
                        snackbarHost = { SnackbarHost(snackbarHost) },
                    ) { innerPadding ->
                        KanbanScreen(
                            modifier = Modifier.fillMaxSize().padding(innerPadding),
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            board = activeKanbanBoard,
                            columns = kanbanColumns,
                            foldersByAccount = foldersByAccount,
                            filter = kanbanFilter,
                            search = kanbanSearch,
                            searchScope = kanbanSearchScope,
                            onOpen = ::readCoreThread,
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

            Screen.Mail -> {
                ModalNavigationDrawer(
                    drawerState = drawerState,
                    drawerContent = {
                        MailDrawer(
                            accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                            selectedAccountId = selectedCoreAccountId,
                            folders = coreFolders,
                            currentScreen = screen,
                            showUnreadBadges = showUnreadBadges,
                            showUnifiedInboxNav = showUnifiedInboxNav,
                            showStarredNav = showStarredNav,
                            kanbanBoards = kanbanBoards,
                            activeKanbanBoardId = activeKanbanBoardId,
                            onSelectUnified = {
                                if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID) {
                                    selectedCoreAccountId = UNIFIED_ACCOUNT_ID
                                    selectedCoreFolder = INBOX_FOLDER
                                    coreThreads = emptyList()
                                    selectedCoreThread = null
                                    messages = emptyList()
                                    syncCoreThreads(accountOverride = UNIFIED_ACCOUNT_ID, folderOverride = INBOX_FOLDER)
                                }
                                screen = Screen.Mail
                                scope.launch { drawerState.close() }
                            },
                            onSelectAccount = { account ->
                                if (selectedCoreAccountId != account.id) {
                                    selectedCoreAccountId = account.id
                                    selectedCoreFolder = INBOX_FOLDER
                                    coreFolders = emptyList()
                                    coreThreads = emptyList()
                                    selectedCoreThread = null
                                    messages = emptyList()
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
                                saveActiveKanbanBoardId(context, board.id)
                                screen = Screen.Kanban
                                previousTopScreen = Screen.Kanban
                                loadKanbanBoard(refresh = false)
                                scope.launch { drawerState.close() }
                            },
                            onAddAccount = {
                                addSection = 0
                                previousTopScreen = screen
                                screen = Screen.AddAccount
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                previousTopScreen = screen
                                screen = Screen.Settings
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
                                        Text(
                                            "${selectedMailThreads.size} selected",
                                            fontWeight = FontWeight.SemiBold,
                                        )
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
                                            Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.mobile_actions_clear_selection))
                                        }
                                    } else {
                                        IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                            Icon(Icons.Filled.Menu, contentDescription = stringResource(R.string.mobile_actions_open_navigation))
                                        }
                                    }
                                },
                                actions = {
                                    if (mailSelectionActive) {
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::archiveOrRemove)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Archive, contentDescription = stringResource(R.string.threads_actions_archive_thread))
                                        }
                                        IconButton(onClick = {
                                            selectedMailThreads.forEach(::deleteThread)
                                            selectedMailThreadIds = emptySet()
                                        }) {
                                            Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.buttons_delete))
                                        }
                                        Box {
                                            IconButton(onClick = { mailSelectionMenuOpen = true }) {
                                                Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.chat_more_actions))
                                            }
                                            DropdownMenu(
                                                expanded = mailSelectionMenuOpen,
                                                onDismissRequest = { mailSelectionMenuOpen = false },
                                            ) {
                                                val markRead = selectedMailThreads.any { it.unread }
                                                DropdownMenuItem(
                                                    text = { Text(if (markRead) stringResource(R.string.threads_actions_mark_as_read) else stringResource(R.string.threads_actions_mark_as_unread)) },
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
                                                    text = { Text(if (star) stringResource(R.string.chat_star) else stringResource(R.string.chat_unstar)) },
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
                                                    DropdownMenuItem(
                                                        text = { Text(stringResource(R.string.threads_actions_move_to)) },
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
                                                        text = { Text(stringResource(R.string.threads_actions_copy_to)) },
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
                                                Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.threads_actions_title))
                                            }
                                            DropdownMenu(expanded = mailboxMenuOpen, onDismissRequest = { mailboxMenuOpen = false }) {
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
                                                if (coreThreads.any { it.unread }) {
                                                    DropdownMenuItem(
                                                        text = { Text(stringResource(R.string.threads_actions_mark_all_as_read)) },
                                                        leadingIcon = {
                                                            Icon(Icons.Filled.MarkEmailUnread, contentDescription = null)
                                                        },
                                                        onClick = {
                                                            mailboxMenuOpen = false
                                                            markVisibleMailboxAllRead()
                                                        },
                                                    )
                                                }
                                                if (selectedAccount != null) {
                                                    DropdownMenuItem(
                                                        text = { Text(stringResource(R.string.settings_account_account_settings)) },
                                                        onClick = {
                                                            mailboxMenuOpen = false
                                                            accountSettingsTargetId = selectedAccount.id
                                                            previousTopScreen = Screen.Mail
                                                            screen = Screen.Settings
                                                        },
                                                    )
                                                    if (selectedAccount.let(::accountSummaryIsRss)) {
                                                        DropdownMenuItem(
                                                            text = { Text(stringResource(R.string.feeds_actions_add_feed)) },
                                                            onClick = {
                                                                mailboxMenuOpen = false
                                                                addFeedUrl = ""
                                                                showAddFeedDialog = true
                                                            },
                                                        )
                                                        DropdownMenuItem(
                                                            text = { Text(stringResource(R.string.common_import)) },
                                                            onClick = {
                                                                mailboxMenuOpen = false
                                                                opmlImportPicker.launch(arrayOf("text/xml", "application/xml", "text/*", "*/*"))
                                                            },
                                                        )
                                                        DropdownMenuItem(
                                                            text = { Text(stringResource(R.string.common_export)) },
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
                                    icon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                                    text = { Text(stringResource(R.string.mobile_tabs_compose)) },
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
                            }
                            PullToRefreshBox(
                                isRefreshing = syncing,
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
                                            title = "Welcome to Meron",
                                            text = "Add a mail or RSS account to start reading your inbox.",
                                            actionLabel = "Add account",
                                            onAction = {
                                                addSection = 0
                                                screen = Screen.AddAccount
                                            },
                                        )
                                    }

                                    coreThreads.isEmpty() && (syncing || !initialThreadsLoaded) -> {
                                        LoadingState("Loading your inbox…")
                                    }

                                    coreThreads.isEmpty() -> {
                                        EmptyState(
                                            icon = Icons.Outlined.Drafts,
                                            title =
                                                if (mailSearch.isBlank() &&
                                                    mailFilter == FilterMode.All
                                                ) {
                                                    "Nothing here yet"
                                                } else {
                                                    "No matching mail"
                                                },
                                            text =
                                                if (mailSearch.isBlank() && mailFilter == FilterMode.All) {
                                                    "Pull in your latest messages from the server."
                                                } else {
                                                    "Adjust the search or filter, then try again."
                                                },
                                            actionLabel = "Sync now",
                                            onAction = ::syncCoreThreads,
                                        )
                                    }

                                    else -> {
                                        MailList(
                                            threads = coreThreads,
                                            canLoadMore =
                                                if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                                                    mailboxAccountCursors.isNotEmpty()
                                                } else {
                                                    mailboxCursor.isNotBlank()
                                                },
                                            loadingMore = loadingMoreThreads,
                                            onOpen = ::readCoreThread,
                                            onToggleStar = ::toggleStar,
                                            onArchive = ::archiveOrRemove,
                                            onDelete = ::deleteThread,
                                            onCopyFeedUrl = { thread ->
                                                copyToClipboard(context, "Feed URL", thread.feedUrl)
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
                                            onLoadMore = ::loadMoreCoreThreads,
                                            showSenderImages = showSenderImages,
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
                title = { Text("Add feed") },
                text = {
                    OutlinedTextField(
                        value = addFeedUrl,
                        onValueChange = { addFeedUrl = it },
                        label = { Text("Feed URL") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                },
                confirmButton = {
                    TextButton(onClick = ::addFeedToSelectedRssAccount) {
                        Text("Add")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showAddFeedDialog = false }) {
                        Text("Cancel")
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
                packageName = context.packageName,
                coreProtocolVersion = if (MeronCoreNative.isLoaded()) MeronCoreNative.protocolVersion() else 0,
                sharedProtocolVersion = SharedMobileContract.protocolVersion,
                onOpenUrl = { url ->
                    context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                },
                onDismiss = { showAboutDialog = false },
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
                onAddColumn = {
                    addKanbanColumn(it)
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
                title = { Text("Create folder") },
                text = {
                    OutlinedTextField(
                        value = kanbanFolderNameInput,
                        onValueChange = { kanbanFolderNameInput = it },
                        label = { Text("Folder name") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                },
                confirmButton = {
                    TextButton(onClick = { createFolderForKanban(account, kanbanFolderNameInput) }) {
                        Text("Create")
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showKanbanCreateFolderDialog = null }) {
                        Text("Cancel")
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
                    copyToClipboard(context, "Feed URL", thread.feedUrl)
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
