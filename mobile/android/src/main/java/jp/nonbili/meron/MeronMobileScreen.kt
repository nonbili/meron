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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Inbox
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
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.ListItem
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.DrawerValue
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.AnnotatedString
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
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
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
    val snackbarHost = remember { SnackbarHostState() }

    // Lab/test defaults only seed the form in debug builds; release builds start
    // empty so users see placeholder hints instead of fake credentials.
    val labDefaults = BuildConfig.DEBUG
    var host by remember { mutableStateOf(if (labDefaults) "10.0.2.2" else "") }
    var email by remember { mutableStateOf(if (labDefaults) "user1@mail.localhost" else "") }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf(if (labDefaults) "user1password" else "") }
    var displayName by remember { mutableStateOf(if (labDefaults) "Local Test" else "") }
    var senderName by remember { mutableStateOf(if (labDefaults) "Local Test" else "") }
    var imapPort by remember { mutableStateOf("993") }
    var smtpHost by remember { mutableStateOf(if (labDefaults) "10.0.2.2" else "") }
    var smtpPort by remember { mutableStateOf("465") }
    var oauthProvider by remember { mutableStateOf("gmail") }
    var oauthEmail by remember { mutableStateOf(if (labDefaults) "me@gmail.com" else "") }
    var oauthAccessToken by remember { mutableStateOf("") }
    var oauthRefreshToken by remember { mutableStateOf("") }
    var oauthExpiresAt by remember { mutableStateOf("0") }
    var oauthClientId by remember { mutableStateOf("") }
    var oauthClientSecret by remember { mutableStateOf("") }
    var oauthRedirectUri by remember { mutableStateOf(defaultOAuthRedirectUri()) }
    var oauthState by remember { mutableStateOf(UUID.randomUUID().toString()) }
    var oauthVerifier by remember { mutableStateOf(UUID.randomUUID().toString() + UUID.randomUUID().toString()) }
    var oauthAuthorizationCode by remember { mutableStateOf("") }
    var rssFeedUrl by remember { mutableStateOf(if (labDefaults) "https://example.com/feed.xml" else "") }
    var rssDisplayName by remember { mutableStateOf(if (labDefaults) "Example Feed" else "") }
    var accountJson by remember { mutableStateOf("") }
    var coreAccounts by remember { mutableStateOf(emptyList<AccountSummary>()) }
    var selectedCoreAccountId by remember { mutableStateOf(UNIFIED_ACCOUNT_ID) }
    var coreFolders by remember { mutableStateOf(emptyList<FolderSummary>()) }
    var foldersByAccount by remember { mutableStateOf(emptyMap<String, List<FolderSummary>>()) }
    var selectedCoreFolder by remember { mutableStateOf(INBOX_FOLDER) }
    var mailSearch by remember { mutableStateOf("") }
    var mailFilter by remember { mutableStateOf(FilterMode.All) }
    var coreThreads by remember { mutableStateOf(emptyList<ThreadSummary>()) }
    var mailboxCursor by remember { mutableStateOf("") }
    var mailboxAccountCursors by remember { mutableStateOf(emptyMap<String, String>()) }
    var loadingMoreThreads by remember { mutableStateOf(false) }
    var starredItems by remember { mutableStateOf(emptyList<StarredItemSummary>()) }
    var selectedCoreThread by remember { mutableStateOf<ThreadSummary?>(null) }
    var conversationHtmlOverrides by remember { mutableStateOf(emptyMap<String, Boolean>()) }
    var previousTopScreen by remember { mutableStateOf(Screen.Mail) }
    var kanbanBoards by remember { mutableStateOf(loadKanbanBoards(context, emptyList())) }
    var activeKanbanBoardId by remember { mutableStateOf(loadActiveKanbanBoardId(context)) }
    var kanbanColumns by remember { mutableStateOf(emptyMap<String, KanbanColumnState>()) }
    var kanbanFilter by remember { mutableStateOf(loadKanbanFilter(context)) }
    var kanbanSearch by remember { mutableStateOf(loadKanbanSearch(context)) }
    var kanbanActionThread by remember { mutableStateOf<ThreadSummary?>(null) }
    var showKanbanBoardDialog by remember { mutableStateOf(false) }
    var showKanbanColumnDialog by remember { mutableStateOf(false) }
    var showKanbanCreateFolderDialog by remember { mutableStateOf<AccountSummary?>(null) }
    var kanbanBoardNameInput by remember { mutableStateOf("") }
    var kanbanBoardAvatarInput by remember { mutableStateOf("") }
    var kanbanBoardWallpaperPresetInput by remember { mutableStateOf("") }
    var kanbanBoardWallpaperUrlInput by remember { mutableStateOf("") }
    var kanbanFolderNameInput by remember { mutableStateOf("") }
    var to by remember { mutableStateOf("") }
    var composeFromAccountId by remember { mutableStateOf("") }
    var composeFromEmail by remember { mutableStateOf("") }
    var composeDraftId by remember { mutableStateOf("") }
    var composeDraftSaved by remember { mutableStateOf(false) }
    var composeInReplyTo by remember { mutableStateOf("") }
    var composeReferences by remember { mutableStateOf("") }
    var subject by remember { mutableStateOf("") }
    var body by remember { mutableStateOf("") }
    var quickReplyBody by remember { mutableStateOf("") }
    var quickReplyAttachments by remember { mutableStateOf(emptyList<DraftAttachment>()) }
    var quickReplyFailure by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("") }
    var syncing by remember { mutableStateOf(false) }
    var showUnreadBadges by remember { mutableStateOf(loadAppBoolean(context, SHOW_UNREAD_BADGES_PREF, true)) }
    var showUnifiedInboxNav by remember { mutableStateOf(loadAppBoolean(context, SHOW_UNIFIED_INBOX_PREF, true)) }
    var showStarredNav by remember { mutableStateOf(loadAppBoolean(context, SHOW_STARRED_NAV_PREF, true)) }
    var showSenderImages by remember { mutableStateOf(loadAppBoolean(context, SHOW_SENDER_IMAGES_PREF, false)) }
    var sendShortcutMode by remember { mutableStateOf(loadSendShortcutMode(context)) }
    var kanbanColumnWidth by remember {
        mutableStateOf(
            loadAppInt(context, KANBAN_COLUMN_WIDTH_PREF, KANBAN_COLUMN_DEFAULT_WIDTH)
                .coerceIn(KANBAN_COLUMN_MIN_WIDTH, KANBAN_COLUMN_MAX_WIDTH),
        )
    }
    var hiddenNavigationAccountIds by remember { mutableStateOf(loadAppStringSet(context, HIDDEN_NAV_ACCOUNTS_PREF)) }
    var messages by remember { mutableStateOf(emptyList<MessageBody>()) }
    var messageCursor by remember { mutableStateOf("") }
    var loadingMoreMessages by remember { mutableStateOf(false) }
    var attachments by remember { mutableStateOf(emptyList<DraftAttachment>()) }
    var cc by remember { mutableStateOf("") }
    var bcc by remember { mutableStateOf("") }
    var recipientSuggestionField by remember { mutableStateOf("") }
    var recipientSuggestions by remember { mutableStateOf(emptyList<ContactSuggestion>()) }
    var screen by remember { mutableStateOf(Screen.Mail) }
    var errorBanner by remember { mutableStateOf<String?>(null) }
    var addSection by remember { mutableStateOf(0) }
    var notificationPermissionGranted by remember { mutableStateOf(AndroidNotificationService.canNotify(context)) }
    var mailboxMenuOpen by remember { mutableStateOf(false) }
    var showAccountSettings by remember { mutableStateOf(false) }
    var showAddFeedDialog by remember { mutableStateOf(false) }
    var addFeedUrl by remember { mutableStateOf("") }
    var showAboutDialog by remember { mutableStateOf(false) }
    var pendingOpmlExport by remember { mutableStateOf("") }
    var accountMediaUploadTarget by remember { mutableStateOf<AccountMediaUploadTarget?>(null) }
    var storageUsage by remember { mutableStateOf<StorageUsage?>(null) }
    var storageBusy by remember { mutableStateOf(false) }
    var storageClearConfirming by remember { mutableStateOf(false) }
    var imagePreview by remember { mutableStateOf<AndroidImagePreview?>(null) }
    var pendingAttachmentSave by remember { mutableStateOf<MessageAttachment?>(null) }

    val notificationPermissionLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        notificationPermissionGranted = granted
        status = if (granted) "Notifications enabled" else "Notifications are disabled"
    }
    val opmlImportPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
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
                            val threadsJson = client.listThreads(
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
    val opmlExportPicker = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/xml")) { uri ->
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
    val attachmentSavePicker = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
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
    val attachmentPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
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
    val quickReplyAttachmentPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
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
    val accountMediaPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
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
                            val uploadJson = if (target.wallpaper) {
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
                    status = "OAuth authorization code received; exchange it to add the account."
                }
            }.onFailure {
                status = "OAuth callback failed: ${it.message}"
            }
        }
    }

    fun applyAccounts(json: String, preferEmail: String? = null) {
        accountJson = json
        val parsed = parseAccountListResponse(json)
        coreAccounts = parsed
        selectedCoreAccountId = preferEmail?.let { wanted -> parsed.firstOrNull { it.email == wanted }?.id }
            ?: selectedCoreAccountId.takeIf { sel -> sel == UNIFIED_ACCOUNT_ID || parsed.any { it.id == sel } }
            ?: UNIFIED_ACCOUNT_ID
        kanbanBoards = ensureKanbanDefaults(context, kanbanBoards, parsed)
        if (activeKanbanBoardId.isBlank() || kanbanBoards.none { it.id == activeKanbanBoardId }) {
            activeKanbanBoardId = kanbanBoards.firstOrNull()?.id.orEmpty()
            saveActiveKanbanBoardId(context, activeKanbanBoardId)
        }
    }

    fun listAccounts() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) { MobileMailCommandClient(JniMeronCore()).listAccounts() }
            }.onSuccess {
                applyAccounts(it)
                status = "Loaded accounts"
            }.onFailure {
                status = "Account list failed: ${it.message}"
            }
        }
    }

    fun loadStorageUsage(showStatus: Boolean = false) {
        if (!MeronCoreNative.isLoaded()) return
        scope.launch {
            storageBusy = true
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).storageUsage()
                }
            }.onSuccess {
                storageUsage = parseStorageUsageResponse(it)
                if (showStatus) status = "Loaded storage usage"
            }.onFailure {
                if (showStatus) status = "Storage usage failed: ${it.message}"
            }
            storageBusy = false
        }
    }

    fun clearStorageCache() {
        if (!storageClearConfirming) {
            storageClearConfirming = true
            status = "Tap clear cache again to confirm."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        storageClearConfirming = false
        scope.launch {
            storageBusy = true
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).clearStorageCache()
                }
            }.onSuccess {
                storageUsage = parseStorageUsageResponse(it)
                status = "Cleared cached attachments"
            }.onFailure {
                status = "Clear cache failed: ${it.message}"
            }
            storageBusy = false
        }
    }

    fun addPasswordAccount() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val params = AddPasswordAccountParams(
            email = email.trim(),
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            imapHost = host.trim(),
            imapPort = imapPort.trim().toIntOrNull() ?: 993,
            smtpHost = smtpHost.trim(),
            smtpPort = smtpPort.trim().toIntOrNull() ?: 465,
            username = username.trim().ifBlank { email.trim() },
            password = password,
            tls = true,
        )
        status = "Adding password account..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.addPasswordAccount(params)
                    client.listAccounts()
                }
            }.onSuccess {
                applyAccounts(it, preferEmail = params.email)
                screen = Screen.Mail
                errorBanner = null
                status = "Added ${params.email}"
            }.onFailure {
                errorBanner = it.message ?: "Add account failed"
                status = "Add account failed: ${it.message}"
            }
        }
    }

    fun autodiscoverPasswordAccount() {
        val emailValue = email.trim()
        if (!emailValue.contains('@') || emailValue.endsWith('@')) {
            status = "Enter an email address first."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        status = "Finding mail settings..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    parseAutodiscoverResponse(client.autodiscoverAccount(AutodiscoverAccountParams(emailValue)))
                }
            }.onSuccess { discovered ->
                if (discovered.imapHost.isNotBlank()) host = discovered.imapHost
                if (discovered.imapPort > 0) imapPort = discovered.imapPort.toString()
                if (discovered.smtpHost.isNotBlank()) smtpHost = discovered.smtpHost
                if (discovered.smtpPort > 0) smtpPort = discovered.smtpPort.toString()
                if (discovered.username.isNotBlank()) username = discovered.username
                status = when {
                    discovered.appPasswordProvider.isNotBlank() -> "${discovered.providerName.ifBlank { discovered.appPasswordProvider }} settings found. Use an app password."
                    discovered.source == "guess" -> "Settings guessed. Verify the servers before adding."
                    else -> "Settings found${discovered.providerName.takeIf { it.isNotBlank() }?.let { " for $it" }.orEmpty()}."
                }
            }.onFailure {
                status = "Settings lookup failed: ${it.message}"
            }
        }
    }

    fun addRssAccount() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        status = "Adding RSS account..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.addRssAccount(AddRssAccountParams(feedUrl = rssFeedUrl.trim(), displayName = rssDisplayName.trim()))
                    client.listAccounts()
                }
            }.onSuccess {
                applyAccounts(it)
                screen = Screen.Mail
                status = "Added RSS feed"
            }.onFailure {
                status = "Add RSS failed: ${it.message}"
            }
        }
    }

    fun exportOpmlForSelectedAccount() {
        val accountId = selectedCoreAccountId
        if (accountId == UNIFIED_ACCOUNT_ID || accountId.isBlank()) {
            status = "Select an RSS account first."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).exportOpml(ExportOpmlParams(accountId = accountId))
                }
            }.onSuccess {
                val opml = parseOpmlExportResponse(it)
                if (opml.isBlank()) {
                    status = "No OPML content to export."
                } else {
                    pendingOpmlExport = opml
                    opmlExportPicker.launch("meron-feeds.opml")
                }
            }.onFailure {
                status = "OPML export failed: ${it.message}"
            }
        }
    }

    fun saveAccountSettings(
        account: AccountSummary,
        displayName: String,
        senderName: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        loadRemoteImages: Boolean,
        conversationHtml: Boolean,
        includedInUnified: Boolean,
        muted: Boolean,
        paused: Boolean,
        rssSyncIntervalMinutes: Int,
        aliasesText: String,
    ) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val aliases = aliasesText
            .lineSequence()
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .map { line ->
                val parts = line.split(",", limit = 2).map { it.trim() }
                AccountAliasParams(email = parts[0], name = parts.getOrElse(1) { "" })
            }
            .filter { it.email.isNotBlank() }
            .toList()
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.setAccountName(AccountNameParams(account.id, displayName.trim()))
                    client.setAccountAvatar(AccountAvatarParams(account.id, avatarUrl.trim()))
                    client.setAccountChatWallpaper(AccountChatWallpaperParams(account.id, presetId = wallpaperPresetId.trim()))
                    if (!accountSummaryIsRss(account)) {
                        client.setAccountSenderName(AccountNameParams(account.id, senderName.trim()))
                        client.setAccountAliases(AccountAliasesParams(account.id, aliases))
                    }
                    client.setAccountImages(AccountFlagParams(account.id, loadRemoteImages))
                    client.setAccountConversationHtml(AccountFlagParams(account.id, conversationHtml))
                    client.setAccountUnified(AccountFlagParams(account.id, includedInUnified))
                    client.setAccountMuted(AccountFlagParams(account.id, muted))
                    client.setAccountPaused(AccountFlagParams(account.id, paused))
                    if (accountSummaryIsRss(account)) {
                        client.setAccountRssSyncInterval(AccountRssSyncIntervalParams(account.id, rssSyncIntervalMinutes.coerceIn(5, 1440)))
                    }
                    client.listAccounts()
                }
            }.onSuccess {
                applyAccounts(it)
                showAccountSettings = false
                status = "Saved account settings"
            }.onFailure {
                status = "Account settings failed: ${it.message}"
            }
        }
    }

    fun setAccountNavigationVisible(account: AccountSummary, visible: Boolean) {
        hiddenNavigationAccountIds = if (visible) {
            hiddenNavigationAccountIds - account.id
        } else {
            hiddenNavigationAccountIds + account.id
        }
        saveAppStringSet(context, HIDDEN_NAV_ACCOUNTS_PREF, hiddenNavigationAccountIds)
        if (!visible && selectedCoreAccountId == account.id) {
            selectedCoreAccountId = UNIFIED_ACCOUNT_ID
            selectedCoreFolder = INBOX_FOLDER
            selectedCoreThread = null
            messages = emptyList()
            coreThreads = emptyList()
            mailboxCursor = ""
            mailboxAccountCursors = emptyMap()
        }
    }

    fun removeAccount(account: AccountSummary) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.removeAccount(AccountIdParams(account.id))
                    client.listAccounts()
                }
            }.onSuccess {
                hiddenNavigationAccountIds = hiddenNavigationAccountIds - account.id
                saveAppStringSet(context, HIDDEN_NAV_ACCOUNTS_PREF, hiddenNavigationAccountIds)
                selectedCoreAccountId = UNIFIED_ACCOUNT_ID
                selectedCoreFolder = INBOX_FOLDER
                selectedCoreThread = null
                messages = emptyList()
                coreThreads = emptyList()
                applyAccounts(it)
                showAccountSettings = false
                status = "Removed account"
            }.onFailure {
                status = "Remove account failed: ${it.message}"
            }
        }
    }

    fun moveAccount(account: AccountSummary, delta: Int) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val oldIndex = coreAccounts.indexOfFirst { it.id == account.id }
        val newIndex = (oldIndex + delta).coerceIn(0, coreAccounts.lastIndex)
        if (oldIndex < 0 || oldIndex == newIndex) return
        val next = coreAccounts.toMutableList()
        val moved = next.removeAt(oldIndex)
        next.add(newIndex, moved)
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.reorderAccounts(AccountReorderParams(next.map { it.id }))
                    client.listAccounts()
                }
            }.onSuccess {
                applyAccounts(it, preferEmail = account.email.ifBlank { account.id })
                status = "Moved account"
            }.onFailure {
                status = "Move account failed: ${it.message}"
            }
        }
    }

    fun addOAuthAccount() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val refreshToken = oauthRefreshToken.trim()
        if (refreshToken.isBlank()) {
            status = "OAuth refresh token is required."
            return
        }
        val params = AddOAuthAccountParams(
            email = oauthEmail.trim(),
            provider = oauthProvider,
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            accessToken = oauthAccessToken.trim(),
            refreshToken = refreshToken,
            tokenExpiresAt = oauthExpiresAt.trim().toLongOrNull() ?: 0,
        )
        status = "Adding ${oauthProvider.replaceFirstChar { it.uppercase() }} account..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.addOAuthAccount(params)
                    client.listAccounts()
                }
            }.onSuccess {
                applyAccounts(it, preferEmail = params.email)
                screen = Screen.Mail
                errorBanner = null
                status = "Added ${params.email}"
            }.onFailure {
                errorBanner = it.message ?: "Add OAuth failed"
                status = "Add OAuth failed: ${it.message}"
            }
        }
    }

    fun exchangeOAuthCode() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val code = oauthAuthorizationCode.trim()
        if (code.isBlank()) {
            status = "OAuth authorization code is required."
            return
        }
        val clientId = oauthClientId.trim()
        if (clientId.isBlank()) {
            status = "OAuth client ID is required."
            return
        }
        val params = ExchangeOAuthCodeParams(
            email = oauthEmail.trim(),
            provider = oauthProvider,
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            code = code,
            clientId = clientId,
            clientSecret = oauthClientSecret.trim(),
            redirectUri = oauthRedirectUri.trim(),
            codeVerifier = oauthVerifier,
        )
        status = "Exchanging OAuth code..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.exchangeOAuthCode(params)
                    client.listAccounts()
                }
            }.onSuccess {
                applyAccounts(it, preferEmail = params.email)
                screen = Screen.Mail
                errorBanner = null
                status = "Connected ${params.email}"
            }.onFailure {
                errorBanner = it.message ?: "OAuth exchange failed"
                status = "OAuth exchange failed: ${it.message}"
            }
        }
    }

    fun launchOAuthFlow() {
        val clientId = oauthClientId.trim()
        if (clientId.isBlank()) {
            status = "OAuth client ID is required."
            return
        }
        oauthState = UUID.randomUUID().toString()
        oauthVerifier = UUID.randomUUID().toString() + UUID.randomUUID().toString()
        val url = buildOAuthAuthorizationUrl(
            OAuthAuthorizationRequest(
                provider = oauthProvider,
                clientId = clientId,
                redirectUri = oauthRedirectUri.trim(),
                state = oauthState,
                codeChallenge = oauthVerifier.pkceChallenge(),
                loginHint = oauthEmail.trim(),
            ),
        )
        runCatching {
            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
        }.onSuccess {
            status = "Opened ${oauthProvider.replaceFirstChar { it.uppercase() }} sign-in"
        }.onFailure {
            status = "OAuth browser launch failed: ${it.message}"
        }
    }

    suspend fun loadAccountInbox(
        client: MobileMailCommandClient,
        account: AccountSummary,
        requestedFolder: String,
        query: String = mailSearch,
        filter: FilterMode = mailFilter,
        syncFirst: Boolean = true,
        beforeCursor: String? = null,
    ): MailboxLoadResult {
        // When syncFirst is false we read whatever the local (encrypted) store
        // already has — used on startup so the inbox shows instantly without a
        // server round-trip. Pull-to-sync / "Sync now" still fetch from server.
        if (syncFirst) {
            if (accountSummaryIsRss(account)) {
                client.syncRss(SyncRssParams(accountId = account.id))
            } else {
                client.sync(SyncMailParams(accountId = account.id, folderId = requestedFolder, limit = 50, folders = true))
            }
        }
        val foldersJson = client.listFolders(FolderListParams(accountId = account.id))
        val folders = parseFolderListResponse(foldersJson)
        // Server folder names are case-sensitive ("INBOX"), but the default
        // request uses "inbox"; match case-insensitively and fall back to a real
        // inbox before the first folder.
        val folder = folders.firstOrNull { it.name.equals(requestedFolder, ignoreCase = true) }?.name
            ?: folders.firstOrNull { it.name.equals(INBOX_FOLDER, ignoreCase = true) }?.name
            ?: folders.firstOrNull()?.name
            ?: requestedFolder
        val threadsJson = client.listThreads(
            ThreadListParams(
                accountId = account.id,
                folderId = folder,
                query = query.trim(),
                filter = filter.protocolValue(),
                beforeCursor = beforeCursor,
            ),
        )
        val page = parseThreadListPage(threadsJson)
        return MailboxLoadResult(
            folders = folders.filter { it.name.equals(folder, ignoreCase = true) },
            folder = folder,
            threads = page.threads,
            nextCursor = page.nextCursor,
        )
    }

    suspend fun loadAccountFolders(client: MobileMailCommandClient, account: AccountSummary): List<FolderSummary> {
        val foldersJson = client.listFolders(FolderListParams(accountId = account.id))
        return parseFolderListResponse(foldersJson)
    }

    fun persistKanbanBoards(next: List<KanbanBoardSpec>) {
        kanbanBoards = next
        saveKanbanBoards(context, next)
        if (activeKanbanBoardId.isBlank() || next.none { it.id == activeKanbanBoardId }) {
            activeKanbanBoardId = next.firstOrNull()?.id.orEmpty()
            saveActiveKanbanBoardId(context, activeKanbanBoardId)
        }
    }

    fun persistKanbanFilter(next: FilterMode) {
        kanbanFilter = next
        saveKanbanFilter(context, next)
    }

    fun persistKanbanSearch(next: String) {
        kanbanSearch = next
        saveKanbanSearch(context, next)
    }

    fun updateKanbanColumn(key: String, update: (KanbanColumnState) -> KanbanColumnState) {
        kanbanColumns = kanbanColumns + (key to update(kanbanColumns[key] ?: KanbanColumnState()))
    }

    fun updateThreadEverywhere(thread: ThreadSummary, update: (ThreadSummary) -> ThreadSummary) {
        val next = update(thread)
        coreThreads = coreThreads.map { if (it.id == thread.id) next else it }
        selectedCoreThread = selectedCoreThread?.let { if (it.id == thread.id) next else it }
        kanbanColumns = kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.map { if (it.id == thread.id) next else it })
        }
    }

    fun removeThreadEverywhere(threadId: String) {
        coreThreads = coreThreads.filterNot { it.id == threadId }
        kanbanColumns = kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.filterNot { it.id == threadId })
        }
        if (selectedCoreThread?.id == threadId) {
            selectedCoreThread = null
            messages = emptyList()
        }
    }

    suspend fun fetchKanbanColumn(
        client: MobileMailCommandClient,
        column: KanbanColumnSpec,
        refresh: Boolean,
        beforeCursor: String? = null,
        accountCursors: Map<String, String> = emptyMap(),
    ): MailboxLoadResult {
        return if (column.accountId == UNIFIED_ACCOUNT_ID) {
            val unifiedAccounts = coreAccounts.filter { it.includedInUnified }
            val accounts = if (accountCursors.isEmpty()) {
                unifiedAccounts
            } else {
                unifiedAccounts.filter { accountCursors[it.id].orEmpty().isNotBlank() }
            }
            val results = accounts.map { account ->
                account.id to loadAccountInbox(
                    client,
                    account,
                    INBOX_FOLDER,
                    query = kanbanSearch,
                    filter = kanbanFilter,
                    syncFirst = refresh,
                    beforeCursor = accountCursors[account.id],
                )
            }
            MailboxLoadResult(
                folders = results.flatMap { it.second.folders },
                folder = INBOX_FOLDER,
                threads = results.flatMap { it.second.threads }.sortedByDescending { it.dateEpochSeconds },
                accountCursors = results
                    .mapNotNull { (id, result) -> result.nextCursor.takeIf { it.isNotBlank() }?.let { id to it } }
                    .toMap(),
            )
        } else {
            val account = coreAccounts.firstOrNull { it.id == column.accountId }
                ?: return MailboxLoadResult(emptyList(), column.folderId, emptyList())
            if (refresh) {
                if (accountSummaryIsRss(account)) client.syncRss(SyncRssParams(accountId = account.id))
                else client.sync(SyncMailParams(accountId = account.id, folderId = column.folderId, limit = 50, folders = true))
            }
            val folders = loadAccountFolders(client, account)
            val folder = folders.firstOrNull { it.name.equals(column.folderId, ignoreCase = true) }?.name ?: column.folderId
            val threadsJson = client.listThreads(
                ThreadListParams(
                    accountId = account.id,
                    folderId = folder,
                    query = kanbanSearch.trim(),
                    filter = kanbanFilter.protocolValue(),
                    beforeCursor = beforeCursor,
                ),
            )
            val page = parseThreadListPage(threadsJson)
            MailboxLoadResult(
                folders = folders,
                folder = folder,
                threads = page.threads,
                nextCursor = page.nextCursor,
            )
        }
    }

    fun loadKanbanColumn(column: KanbanColumnSpec, refresh: Boolean = false) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val key = kanbanColumnKey(column)
        updateKanbanColumn(key) { it.copy(loading = true, error = null) }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    fetchKanbanColumn(client, column, refresh)
                }
            }.onSuccess { result ->
                if (result.folders.isNotEmpty()) {
                    foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
                }
                updateKanbanColumn(key) {
                    it.copy(
                        threads = result.threads,
                        loading = false,
                        loadingMore = false,
                        error = null,
                        nextCursor = if (kanbanSearch.isBlank()) result.nextCursor else "",
                        accountCursors = if (kanbanSearch.isBlank()) result.accountCursors else emptyMap(),
                    )
                }
            }.onFailure {
                updateKanbanColumn(key) { state -> state.copy(loading = false, error = it.message ?: "Load failed") }
                status = "Kanban load failed: ${it.message}"
            }
        }
    }

    fun loadMoreKanbanColumn(column: KanbanColumnSpec) {
        if (!MeronCoreNative.isLoaded() || kanbanSearch.isNotBlank()) return
        val key = kanbanColumnKey(column)
        val state = kanbanColumns[key] ?: return
        val hasCursor = if (column.accountId == UNIFIED_ACCOUNT_ID) state.accountCursors.isNotEmpty() else state.nextCursor.isNotBlank()
        if (state.loadingMore || !hasCursor) return
        updateKanbanColumn(key) { it.copy(loadingMore = true, error = null) }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    fetchKanbanColumn(
                        client = client,
                        column = column,
                        refresh = false,
                        beforeCursor = state.nextCursor,
                        accountCursors = state.accountCursors,
                    )
                }
            }.onSuccess { result ->
                if (result.folders.isNotEmpty()) {
                    foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
                }
                updateKanbanColumn(key) { current ->
                    val existingIds = current.threads.map { it.id }.toSet()
                    val appended = result.threads.filterNot { it.id in existingIds }
                    current.copy(
                        threads = (current.threads + appended).sortedByDescending { it.dateEpochSeconds },
                        loadingMore = false,
                        error = null,
                        nextCursor = result.nextCursor,
                        accountCursors = result.accountCursors,
                    )
                }
            }.onFailure {
                updateKanbanColumn(key) { state -> state.copy(loadingMore = false, error = it.message ?: "Load failed") }
                status = "Kanban load more failed: ${it.message}"
            }
        }
    }

    fun loadKanbanBoard(refresh: Boolean = false) {
        val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
        board.columns.forEach { column -> loadKanbanColumn(column, refresh) }
    }

    fun syncCoreThreads(accountOverride: String? = null, folderOverride: String? = null, syncFirst: Boolean = true) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val accountId = accountOverride ?: selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
        val requestedFolder = folderOverride ?: selectedCoreFolder.ifBlank { INBOX_FOLDER }
        val query = mailSearch
        val filter = mailFilter
        val selectedAccounts = if (accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts.filter { it.includedInUnified }
        } else {
            coreAccounts.filter { it.id == accountId }
        }
        if (selectedAccounts.isEmpty()) {
            status = if (accountId == UNIFIED_ACCOUNT_ID) "No accounts are included in Unified inbox." else "No account selected."
            return
        }
        syncing = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    if (accountId == UNIFIED_ACCOUNT_ID) {
                        val results = selectedAccounts.map { account ->
                            account.id to loadAccountInbox(
                                client,
                                account,
                                INBOX_FOLDER,
                                query = query,
                                filter = filter,
                                syncFirst = syncFirst,
                            )
                        }
                        MailboxLoadResult(
                            folders = results.flatMap { it.second.folders },
                            folder = INBOX_FOLDER,
                            threads = results.flatMap { it.second.threads }.sortedByDescending { it.dateEpochSeconds },
                            accountCursors = results
                                .mapNotNull { (id, result) -> result.nextCursor.takeIf { it.isNotBlank() }?.let { id to it } }
                                .toMap(),
                        )
                    } else {
                        loadAccountInbox(
                            client,
                            selectedAccounts.first(),
                            requestedFolder,
                            query = query,
                            filter = filter,
                            syncFirst = syncFirst,
                        )
                    }
                }
            }.onSuccess { result ->
                coreFolders = result.folders
                if (result.folders.isNotEmpty()) {
                    foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
                }
                val folder = result.folder
                selectedCoreFolder = folder
                val parsedThreads = result.threads
                coreThreads = parsedThreads
                mailboxCursor = result.nextCursor
                mailboxAccountCursors = result.accountCursors
                if (selectedCoreThread?.id !in parsedThreads.map { it.id }) {
                    selectedCoreThread = null
                    messages = emptyList()
                }
                syncing = false
                errorBanner = null
                status = if (accountId == UNIFIED_ACCOUNT_ID) {
                    "${parsedThreads.size} ${filter.label().lowercase()} message(s) in Unified inbox"
                } else {
                    "${parsedThreads.size} ${filter.label().lowercase()} message(s) in ${folder.replaceFirstChar { it.uppercase() }}"
                }
            }.onFailure {
                syncing = false
                errorBanner = it.message ?: "Sync failed"
                status = "Sync failed: ${it.message}"
            }
        }
    }

    fun addFeedToSelectedRssAccount() {
        val account = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }
        val feedUrl = addFeedUrl.trim()
        if (account == null || !accountSummaryIsRss(account)) {
            status = "Select an RSS account first."
            return
        }
        if (feedUrl.isBlank()) {
            status = "Feed URL is required."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        status = "Adding feed..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).addRssFeed(
                        AddRssFeedParams(accountId = account.id, feedUrl = feedUrl),
                    )
                }
            }.onSuccess {
                addFeedUrl = ""
                showAddFeedDialog = false
                status = "Feed added"
                syncCoreThreads(accountOverride = account.id, syncFirst = true)
            }.onFailure {
                status = "Add feed failed: ${it.message}"
            }
        }
    }

    fun loadMoreCoreThreads() {
        if (!MeronCoreNative.isLoaded() || loadingMoreThreads) return
        val accountId = selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
        val requestedFolder = selectedCoreFolder.ifBlank { INBOX_FOLDER }
        val query = mailSearch
        val filter = mailFilter
        val selectedAccounts = if (accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts.filter { it.includedInUnified && mailboxAccountCursors[it.id].orEmpty().isNotBlank() }
        } else {
            coreAccounts.filter { it.id == accountId && mailboxCursor.isNotBlank() }
        }
        if (selectedAccounts.isEmpty()) return
        loadingMoreThreads = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    if (accountId == UNIFIED_ACCOUNT_ID) {
                        val results = selectedAccounts.map { account ->
                            account.id to loadAccountInbox(
                                client,
                                account,
                                INBOX_FOLDER,
                                query = query,
                                filter = filter,
                                syncFirst = false,
                                beforeCursor = mailboxAccountCursors[account.id],
                            )
                        }
                        MailboxLoadResult(
                            folders = results.flatMap { it.second.folders },
                            folder = INBOX_FOLDER,
                            threads = results.flatMap { it.second.threads }.sortedByDescending { it.dateEpochSeconds },
                            accountCursors = results
                                .mapNotNull { (id, result) -> result.nextCursor.takeIf { it.isNotBlank() }?.let { id to it } }
                                .toMap(),
                        )
                    } else {
                        loadAccountInbox(
                            client,
                            selectedAccounts.first(),
                            requestedFolder,
                            query = query,
                            filter = filter,
                            syncFirst = false,
                            beforeCursor = mailboxCursor,
                        )
                    }
                }
            }.onSuccess { result ->
                if (result.folders.isNotEmpty()) {
                    coreFolders = result.folders
                    foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
                }
                val existingIds = coreThreads.map { it.id }.toSet()
                val appended = result.threads.filterNot { it.id in existingIds }
                coreThreads = (coreThreads + appended).sortedByDescending { it.dateEpochSeconds }
                mailboxCursor = result.nextCursor
                mailboxAccountCursors = result.accountCursors
                loadingMoreThreads = false
                errorBanner = null
                status = if (appended.isEmpty()) "No older messages." else "Loaded ${appended.size} older message(s)."
            }.onFailure {
                loadingMoreThreads = false
                errorBanner = it.message ?: "Load more failed"
                status = "Load more failed: ${it.message}"
            }
        }
    }

    fun loadStarredItems() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        syncing = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) { MobileMailCommandClient(JniMeronCore()).listStarredItems() }
            }.onSuccess {
                starredItems = parseStarredItemsResponse(it)
                syncing = false
                status = "${starredItems.size} starred item(s)"
            }.onFailure {
                syncing = false
                status = "Starred load failed: ${it.message}"
            }
        }
    }

    fun openDraftCompose(message: MessageBody, thread: ThreadSummary) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    forwardableAttachments(message).mapNotNull { attachment ->
                        val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                        data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                    }
                }
            }.onSuccess { copiedAttachments ->
                to = message.to
                cc = message.cc
                bcc = message.bcc
                subject = message.subject
                body = message.body
                attachments = copiedAttachments
                composeFromAccountId = thread.accountId
                composeFromEmail = ""
                composeDraftId = message.messageId.trim().trim('<', '>').ifBlank { newDraftMessageId(thread.accountId) }
                composeDraftSaved = true
                previousTopScreen = Screen.Mail
                screen = Screen.Compose
                status = "Draft ready"
            }.onFailure {
                status = "Draft open failed: ${it.message}"
            }
        }
    }

    fun readCoreThread(thread: ThreadSummary) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        selectedCoreThread = thread
        messages = emptyList()
        messageCursor = ""
        loadingMoreMessages = false
        previousTopScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
        screen = Screen.Thread
        if (thread.unread) {
            coreThreads = coreThreads.map { if (it.id == thread.id) it.copy(unread = false) else it }
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    if (threadIdIsRss(thread.id)) {
                        client.readRssThread(RssThreadParams(threadId = thread.id))
                    } else {
                        client.readThread(ThreadReadParams(threadId = thread.id))
                    }
                }
            }.onSuccess {
                val page = parseThreadReadPage(it)
                messages = page.messages
                messageCursor = page.nextCursor
                updateThreadEverywhere(thread) { current -> current.copy(unread = false) }
                if (!threadIdIsRss(thread.id) && folderIsDrafts(thread.folder)) {
                    page.messages.lastOrNull()?.let { message ->
                        openDraftCompose(message, thread)
                    }
                }
            }.onFailure {
                status = "Could not open message: ${it.message}"
            }
        }
    }

    fun loadMoreThreadMessages() {
        val thread = selectedCoreThread ?: return
        if (!MeronCoreNative.isLoaded() || messageCursor.isBlank() || loadingMoreMessages) return
        val cursor = messageCursor
        loadingMoreMessages = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    if (threadIdIsRss(thread.id)) {
                        client.readRssThread(RssThreadParams(threadId = thread.id, beforeCursor = cursor))
                    } else {
                        client.readThread(ThreadReadParams(threadId = thread.id, beforeCursor = cursor))
                    }
                }
            }.onSuccess {
                val page = parseThreadReadPage(it)
                val existingIds = messages.map { message -> message.id }.toSet()
                val older = page.messages.filterNot { message -> message.id in existingIds }
                messages = (older + messages).sortedBy { message -> message.dateEpochSeconds }
                messageCursor = page.nextCursor
                loadingMoreMessages = false
                status = if (older.isEmpty()) "No older messages in this thread." else "Loaded ${older.size} older message(s)."
            }.onFailure {
                loadingMoreMessages = false
                status = "Could not load older messages: ${it.message}"
            }
        }
    }

    fun readStarredItem(item: StarredItemSummary) {
        readCoreThread(
            ThreadSummary(
                id = item.threadId,
                accountId = item.accountId,
                folder = item.folder,
                subject = item.subject,
                sender = item.sender,
                preview = item.preview,
                unread = item.unread,
                starred = true,
                dateEpochSeconds = item.dateEpochSeconds,
            ),
        )
    }

    fun runStarredItemAction(
        item: StarredItemSummary,
        label: String,
        action: suspend MobileMailCommandClient.() -> String,
        update: (List<StarredItemSummary>) -> List<StarredItemSummary>,
    ) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) { MobileMailCommandClient(JniMeronCore()).action() }
            }.onSuccess {
                starredItems = update(starredItems)
                status = "$label complete"
            }.onFailure {
                status = "$label failed: ${it.message}"
            }
        }
    }

    fun toggleStarredItemRead(item: StarredItemSummary) {
        val isRssItem = threadIdIsRss(item.threadId)
        runStarredItemAction(
            item = item,
            label = if (item.unread) "Mark read" else "Mark unread",
            action = {
                if (isRssItem) markRssRead(RssMarkReadParams(threadId = item.threadId, seen = item.unread, itemKeys = listOf(item.id)))
                else markRead(MarkReadParams(threadId = item.threadId, seen = item.unread, messageIds = listOf(item.id)))
            },
            update = { rows -> rows.map { if (it.id == item.id) it.copy(unread = !item.unread) else it } },
        )
    }

    fun unstarStarredItem(item: StarredItemSummary) {
        val isRssItem = threadIdIsRss(item.threadId)
        runStarredItemAction(
            item = item,
            label = "Unstar",
            action = {
                if (isRssItem) markRssStarred(RssMarkStarredParams(threadId = item.threadId, starred = false, itemKeys = listOf(item.id)))
                else markStarred(MarkStarredParams(threadId = item.threadId, starred = false, messageIds = listOf(item.id)))
            },
            update = { rows -> rows.filterNot { it.id == item.id } },
        )
    }

    fun deleteStarredMailItem(item: StarredItemSummary) {
        if (threadIdIsRss(item.threadId)) {
            status = "RSS items cannot be deleted."
            return
        }
        runStarredItemAction(
            item = item,
            label = threadDeleteActionLabel(item.folder),
            action = { delete(ThreadActionParams(threadId = item.threadId, folderId = item.folder, messageIds = listOf(item.id))) },
            update = { rows -> rows.filterNot { it.id == item.id } },
        )
    }

    fun runCoreThreadAction(
        thread: ThreadSummary,
        label: String,
        action: suspend MobileMailCommandClient.() -> String,
        update: (List<ThreadSummary>) -> List<ThreadSummary>,
        undoMessage: String? = null,
        onUndo: (() -> Unit)? = null,
    ) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) { MobileMailCommandClient(JniMeronCore()).action() }
            }.onSuccess {
                coreThreads = update(coreThreads)
                kanbanColumns = kanbanColumns.mapValues { (_, state) -> state.copy(threads = update(state.threads)) }
                if (undoMessage != null && onUndo != null) {
                    val result = snackbarHost.showSnackbar(
                        message = undoMessage,
                        actionLabel = "Undo",
                        duration = SnackbarDuration.Long,
                    )
                    if (result == SnackbarResult.ActionPerformed) onUndo()
                } else {
                    status = "$label complete"
                }
            }.onFailure {
                status = "$label failed: ${it.message}"
            }
        }
    }

    // Moves a thread back to the folder it was in before an archive/delete and
    // restores the pre-action list snapshots, backing the "Undo" snackbar action.
    fun restoreThread(
        thread: ThreadSummary,
        threadsSnapshot: List<ThreadSummary>,
        kanbanSnapshot: Map<String, KanbanColumnState>,
    ) {
        if (!MeronCoreNative.isLoaded()) return
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).move(
                        MoveThreadParams(threadId = thread.id, targetFolderId = thread.folder),
                    )
                }
            }.onSuccess {
                coreThreads = threadsSnapshot
                kanbanColumns = kanbanSnapshot
                status = "Restored"
            }.onFailure {
                status = "Undo failed: ${it.message}"
            }
        }
    }

    fun toggleStar(thread: ThreadSummary) {
        val isRssThread = threadIdIsRss(thread.id)
        runCoreThreadAction(
            thread = thread,
            label = if (thread.starred) "Unstar" else "Star",
            action = {
                if (isRssThread) markRssStarred(RssMarkStarredParams(threadId = thread.id, starred = !thread.starred))
                else markStarred(MarkStarredParams(threadId = thread.id, starred = !thread.starred))
            },
            update = { threads -> threads.map { if (it.id == thread.id) it.copy(starred = !thread.starred) else it } },
        )
    }

    fun toggleRead(thread: ThreadSummary) {
        val isRssThread = threadIdIsRss(thread.id)
        runCoreThreadAction(
            thread = thread,
            label = if (thread.unread) "Mark read" else "Mark unread",
            action = {
                if (isRssThread) markRssRead(RssMarkReadParams(threadId = thread.id, seen = thread.unread))
                else markRead(MarkReadParams(threadId = thread.id, seen = thread.unread))
            },
            update = { threads -> threads.map { if (it.id == thread.id) it.copy(unread = !thread.unread) else it } },
        )
    }

    fun updateMessageEverywhere(messageId: String, update: (MessageBody) -> MessageBody) {
        messages = messages.map { if (it.id == messageId) update(it) else it }
    }

    fun toggleMessageRead(message: MessageBody) {
        val thread = selectedCoreThread ?: return
        val seen = message.unread
        status = if (seen) "Marking read..." else "Marking unread..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).markRead(
                        MarkReadParams(threadId = thread.id, seen = seen, messageIds = listOf(message.id)),
                    )
                }
            }.onSuccess {
                updateMessageEverywhere(message.id) { it.copy(unread = !seen) }
                val updatedUnread = messages.any { it.unread }
                updateThreadEverywhere(thread) { it.copy(unread = updatedUnread) }
                selectedCoreThread = selectedCoreThread?.copy(unread = updatedUnread)
                status = if (seen) "Marked read" else "Marked unread"
            }.onFailure {
                status = "Message update failed: ${it.message}"
            }
        }
    }

    fun toggleMessageStarred(message: MessageBody) {
        val thread = selectedCoreThread ?: return
        val starred = !message.starred
        status = if (starred) "Starring..." else "Unstarring..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).markStarred(
                        MarkStarredParams(threadId = thread.id, starred = starred, messageIds = listOf(message.id)),
                    )
                }
            }.onSuccess {
                updateMessageEverywhere(message.id) { it.copy(starred = starred) }
                val updatedStarred = messages.any { it.starred }
                updateThreadEverywhere(thread) { it.copy(starred = updatedStarred) }
                selectedCoreThread = selectedCoreThread?.copy(starred = updatedStarred)
                status = if (starred) "Starred" else "Unstarred"
            }.onFailure {
                status = "Star failed: ${it.message}"
            }
        }
    }

    fun deleteMessage(message: MessageBody) {
        val thread = selectedCoreThread ?: return
        status = "Deleting message..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).delete(
                        ThreadActionParams(
                            threadId = thread.id,
                            folderId = thread.folder,
                            messageIds = listOf(message.id),
                        ),
                    )
                }
            }.onSuccess {
                messages = messages.filterNot { it.id == message.id }
                status = "Delete complete"
            }.onFailure {
                status = "Delete failed: ${it.message}"
            }
        }
    }

    fun markVisibleMailboxAllRead() {
        val unread = coreThreads.filter { it.unread }
        if (unread.isEmpty()) {
            status = "No unread messages."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val accountsById = coreAccounts.associateBy { it.id }
        val mailTargets = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
            unread
                .map { it.accountId }
                .distinct()
                .filter { accountId -> accountsById[accountId]?.let { !accountSummaryIsRss(it) } ?: true }
                .map { accountId -> accountId to INBOX_FOLDER }
        } else {
            val account = accountsById[selectedCoreAccountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(selectedCoreAccountId to selectedCoreFolder) else emptyList()
        }
        val rssTargets = unread.filter { threadIdIsRss(it.id) }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    mailTargets.forEach { (accountId, folderId) ->
                        client.markAllRead(MarkAllReadParams(accountId = accountId, folderId = folderId))
                    }
                    rssTargets.forEach { thread ->
                        client.markRssRead(RssMarkReadParams(threadId = thread.id, seen = true))
                    }
                }
            }.onSuccess {
                coreThreads = coreThreads.map { if (it.unread) it.copy(unread = false) else it }
                kanbanColumns = kanbanColumns.mapValues { (_, state) ->
                    state.copy(threads = state.threads.map { if (it.unread) it.copy(unread = false) else it })
                }
                status = "Marked ${unread.size} unread item(s) read"
                syncCoreThreads(syncFirst = false)
            }.onFailure {
                status = "Mark all read failed: ${it.message}"
            }
        }
    }

    fun markKanbanColumnAllRead(column: KanbanColumnSpec) {
        val key = kanbanColumnKey(column)
        val unread = kanbanColumns[key]?.threads.orEmpty().filter { it.unread }
        if (unread.isEmpty()) {
            status = "No unread cards."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val accountsById = coreAccounts.associateBy { it.id }
        val mailTargets = if (column.accountId == UNIFIED_ACCOUNT_ID) {
            unread
                .map { it.accountId }
                .distinct()
                .filter { accountId -> accountsById[accountId]?.let { !accountSummaryIsRss(it) } ?: true }
                .map { accountId -> accountId to INBOX_FOLDER }
        } else {
            val account = accountsById[column.accountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(column.accountId to column.folderId) else emptyList()
        }
        val rssTargets = unread.filter { threadIdIsRss(it.id) }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    mailTargets.forEach { (accountId, folderId) ->
                        client.markAllRead(MarkAllReadParams(accountId = accountId, folderId = folderId))
                    }
                    rssTargets.forEach { thread ->
                        client.markRssRead(RssMarkReadParams(threadId = thread.id, seen = true))
                    }
                }
            }.onSuccess {
                updateKanbanColumn(key) { state ->
                    state.copy(threads = state.threads.map { if (it.unread) it.copy(unread = false) else it })
                }
                coreThreads = coreThreads.map { thread ->
                    if (unread.any { it.id == thread.id }) thread.copy(unread = false) else thread
                }
                status = "Marked ${unread.size} Kanban card(s) read"
            }.onFailure {
                status = "Kanban mark all read failed: ${it.message}"
            }
        }
    }

    fun archiveOrRemove(thread: ThreadSummary) {
        if (threadIdIsRss(thread.id)) {
            runCoreThreadAction(
                thread = thread,
                label = "Remove feed",
                action = { removeRssFeed(RemoveRssFeedParams(threadId = thread.id)) },
                update = { threads -> threads.filterNot { it.id == thread.id } },
            )
        } else {
            val threadsSnapshot = coreThreads
            val kanbanSnapshot = kanbanColumns
            runCoreThreadAction(
                thread = thread,
                label = "Archive",
                action = { archive(ThreadActionParams(threadId = thread.id)) },
                update = { threads -> threads.filterNot { it.id == thread.id } },
                undoMessage = "Archived",
                onUndo = { restoreThread(thread, threadsSnapshot, kanbanSnapshot) },
            )
        }
    }

    fun deleteThread(thread: ThreadSummary) {
        val threadsSnapshot = coreThreads
        val kanbanSnapshot = kanbanColumns
        runCoreThreadAction(
            thread = thread,
            label = threadDeleteActionLabel(thread.folder),
            action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
            undoMessage = "Deleted",
            onUndo = { restoreThread(thread, threadsSnapshot, kanbanSnapshot) },
        )
    }

    fun moveThreadToFolder(thread: ThreadSummary, targetFolderId: String, onMoved: () -> Unit = {}) {
        if (threadIdIsRss(thread.id)) {
            status = "RSS feeds move between RSS accounts from Kanban."
            return
        }
        if (targetFolderId.equals(thread.folder, ignoreCase = true)) {
            status = "Already in ${targetFolderId.replaceFirstChar { it.uppercase() }}."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        status = "Moving..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).move(
                        MoveThreadParams(threadId = thread.id, targetFolderId = targetFolderId),
                    )
                }
            }.onSuccess {
                removeThreadEverywhere(thread.id)
                if (selectedCoreThread?.id == thread.id) {
                    selectedCoreThread = null
                    messages = emptyList()
                }
                status = "Move complete"
                onMoved()
            }.onFailure {
                status = "Move failed: ${it.message}"
            }
        }
    }

    fun copyThreadToFolder(thread: ThreadSummary, target: FolderSummary) {
        if (threadIdIsRss(thread.id)) {
            status = "RSS feeds can't be copied to mail folders."
            return
        }
        val targetAccountId = target.accountId.ifBlank { thread.accountId }
        val targetAccount = coreAccounts.firstOrNull { it.id == targetAccountId }
        if (targetAccount == null || accountSummaryIsRss(targetAccount)) {
            status = "Choose a mail account folder."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        status = "Copying..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).copy(
                        CopyThreadParams(
                            threadId = thread.id,
                            targetAccountId = targetAccountId,
                            targetFolderId = target.name,
                        ),
                    )
                }
            }.onSuccess {
                status = "Copy complete"
            }.onFailure {
                status = "Copy failed: ${it.message}"
            }
        }
    }

    fun createFolderAndMoveThread(thread: ThreadSummary, name: String, onMoved: () -> Unit = {}) {
        val trimmed = name.trim()
        if (threadIdIsRss(thread.id)) {
            status = "RSS feeds move between RSS accounts from Kanban."
            return
        }
        if (trimmed.isBlank()) {
            status = "Folder name is required."
            return
        }
        val account = coreAccounts.firstOrNull { it.id == thread.accountId }
        if (account == null) {
            status = "Account not found."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        status = "Creating folder..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.createFolder(FolderCreateParams(accountId = thread.accountId, name = trimmed))
                    val folders = loadAccountFolders(client, account)
                    val created = folders.firstOrNull { it.name.equals(trimmed, ignoreCase = true) }?.name ?: trimmed
                    if (created.equals(thread.folder, ignoreCase = true)) {
                        throw IllegalStateException("Already in $created.")
                    }
                    client.move(MoveThreadParams(threadId = thread.id, targetFolderId = created))
                    folders to created
                }
            }.onSuccess { (folders, _) ->
                foldersByAccount = foldersByAccount + (account.id to folders)
                removeThreadEverywhere(thread.id)
                if (selectedCoreThread?.id == thread.id) {
                    selectedCoreThread = null
                    messages = emptyList()
                }
                status = "Folder created and move complete"
                onMoved()
            }.onFailure {
                status = "Create folder failed: ${it.message}"
            }
        }
    }

    fun moveThreadToColumn(thread: ThreadSummary, target: KanbanColumnSpec) {
        if (target.accountId == UNIFIED_ACCOUNT_ID) {
            status = "Move to an account folder column."
            return
        }
        val targetAccount = coreAccounts.firstOrNull { it.id == target.accountId }
        if (targetAccount == null) {
            status = "Target account not found."
            return
        }
        if (threadIdIsRss(thread.id)) {
            if (!accountSummaryIsRss(targetAccount)) {
                status = "RSS feeds can only move to RSS accounts."
                return
            }
            scope.launch {
                runCatching {
                    withContext(Dispatchers.IO) {
                        MobileMailCommandClient(JniMeronCore()).moveRssFeed(
                            MoveRssFeedParams(threadId = thread.id, targetAccountId = target.accountId),
                        )
                    }
                }.onSuccess {
                    removeThreadEverywhere(thread.id)
                    loadKanbanColumn(target, refresh = false)
                    status = "Move complete"
                }.onFailure {
                    status = "Move failed: ${it.message}"
                }
            }
            return
        }
        if (accountSummaryIsRss(targetAccount)) {
            status = "Mail threads can't move into RSS feeds."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).move(
                        MoveThreadParams(threadId = thread.id, targetFolderId = target.folderId),
                    )
                }
            }.onSuccess {
                removeThreadEverywhere(thread.id)
                loadKanbanColumn(target, refresh = false)
                status = "Move complete"
            }.onFailure {
                status = "Move failed: ${it.message}"
            }
        }
    }

    fun defaultSendAccountId(): String {
        return selectedCoreAccountId.takeIf { selected ->
            selected != UNIFIED_ACCOUNT_ID && coreAccounts.any { it.id == selected && !accountSummaryIsRss(it) }
        } ?: coreAccounts.firstOrNull { !accountSummaryIsRss(it) }?.id.orEmpty()
    }

    fun composeIdentityCandidates(): List<SendIdentity> {
        return coreAccounts
            .filter { !accountSummaryIsRss(it) && !it.needsReconnect }
            .flatMap { accountSendIdentities(it) }
    }

    fun selectedComposeIdentity(): SendIdentity? {
        val candidates = composeIdentityCandidates()
        return candidates.firstOrNull { it.accountId == composeFromAccountId && it.email == composeFromEmail }
            ?: candidates.firstOrNull { it.accountId == defaultSendAccountId() }
            ?: candidates.firstOrNull()
    }

    fun clearComposeDraftState() {
        attachments = emptyList()
        to = ""
        cc = ""
        bcc = ""
        subject = ""
        body = ""
        composeFromAccountId = ""
        composeFromEmail = ""
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = ""
        composeReferences = ""
        recipientSuggestionField = ""
        recipientSuggestions = emptyList()
    }

    fun loadRecipientSuggestions(field: String, value: String) {
        val accountId = defaultSendAccountId()
        if (accountId.isBlank() || !MeronCoreNative.isLoaded()) {
            recipientSuggestions = emptyList()
            recipientSuggestionField = field
            return
        }
        recipientSuggestionField = field
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.suggestContacts(
                        ContactSuggestParams(
                            accountId = accountId,
                            query = recipientTail(value),
                            limit = 6,
                        ),
                    )
                }
            }.onSuccess {
                if (recipientSuggestionField == field) {
                    recipientSuggestions = parseContactSuggestResponse(it)
                }
            }.onFailure {
                if (recipientSuggestionField == field) {
                    recipientSuggestions = emptyList()
                }
            }
        }
    }

    fun acceptRecipientSuggestion(field: String, contact: ContactSuggestion) {
        when (field) {
            "to" -> to = replaceRecipientTail(to, contact)
            "cc" -> cc = replaceRecipientTail(cc, contact)
            "bcc" -> bcc = replaceRecipientTail(bcc, contact)
        }
        recipientSuggestions = emptyList()
    }

    fun sendMail() {
        val identity = selectedComposeIdentity()
        val accountId = identity?.accountId ?: defaultSendAccountId()
        val savedDraftId = composeDraftId.takeIf { composeDraftSaved }
        if (accountId.isBlank()) {
            status = "Select or add an account before sending."
            return
        }
        val draft = ComposeDraft(to.trim(), cc.trim(), bcc.trim(), subject.trim(), body.trim(), attachments)
        if (!draft.canSend) {
            status = "Complete To, Subject, and Body or Attachments before sending."
            return
        }
        status = "Sending..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    val params = draft.toSendMailParams(accountId = accountId, from = identity?.email.orEmpty()).copy(
                        inReplyTo = composeInReplyTo,
                        references = composeReferences,
                    )
                    client.send(params)
                    if (!savedDraftId.isNullOrBlank()) {
                        runCatching { client.discardDraft(DiscardDraftParams(accountId = accountId, draftId = savedDraftId)) }
                    }
                }
            }.onSuccess {
                clearComposeDraftState()
                screen = previousTopScreen
                errorBanner = null
                status = "Message sent"
                syncCoreThreads()
            }.onFailure {
                errorBanner = it.message ?: "Send failed"
                status = "Send failed: ${it.message}"
            }
        }
    }

    fun saveComposeDraft() {
        val identity = selectedComposeIdentity()
        val accountId = identity?.accountId ?: defaultSendAccountId()
        if (accountId.isBlank()) {
            status = "Select or add an account before saving."
            return
        }
        val draft = ComposeDraft(to.trim(), cc.trim(), bcc.trim(), subject.trim(), body.trim(), attachments)
        if (listOf(draft.to, draft.cc, draft.bcc, draft.subject, draft.body).all { it.isBlank() } && draft.attachments.isEmpty()) {
            status = "Nothing to save."
            return
        }
        val draftId = composeDraftId.ifBlank { newDraftMessageId(accountId) }
        composeDraftId = draftId
        status = "Saving draft..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val params = draft.toSaveDraftParams(
                        accountId = accountId,
                        draftId = draftId,
                        from = identity?.email.orEmpty(),
                    ).copy(
                        inReplyTo = composeInReplyTo,
                        references = composeReferences,
                    )
                    MobileMailCommandClient(JniMeronCore()).saveDraft(
                        params,
                    )
                }
            }.onSuccess {
                composeDraftSaved = true
                status = "Draft saved"
                syncCoreThreads(syncFirst = false)
            }.onFailure {
                status = "Draft save failed: ${it.message}"
            }
        }
    }

    fun openQuickReplyInFullEditor() {
        val thread = selectedCoreThread
        val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
        val parent = messages.lastOrNull()
        if (accountId.isBlank() || thread == null || parent == null) {
            status = "Open a mail thread before replying."
            return
        }
        if (threadIdIsRss(thread.id)) {
            status = "RSS items do not support replies."
            return
        }
        val replyFrom = coreAccounts.firstOrNull { it.id == accountId }
            ?.let { detectReplyFromIdentity(parent, it) }
            .orEmpty()
        val params = parent.toReplyMailParams(
            accountId = accountId,
            body = quickReplyBody.trim(),
            from = replyFrom,
            ownAddresses = ownAddressList(coreAccounts),
            attachments = quickReplyAttachments,
        )
        to = params.to
        cc = params.cc
        bcc = params.bcc
        subject = params.subject
        body = params.body
        attachments = quickReplyAttachments
        composeFromAccountId = accountId
        composeFromEmail = replyFrom
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = params.inReplyTo
        composeReferences = params.references
        quickReplyBody = ""
        quickReplyAttachments = emptyList()
        quickReplyFailure = ""
        previousTopScreen = Screen.Thread
        screen = Screen.Compose
        status = "Reply opened in full editor"
    }

    fun discardComposeDraft() {
        val identity = selectedComposeIdentity()
        val accountId = identity?.accountId ?: defaultSendAccountId()
        val draftId = composeDraftId.takeIf { composeDraftSaved }
        if (!draftId.isNullOrBlank() && accountId.isBlank()) {
            status = "Select or add an account before discarding."
            return
        }
        status = "Discarding draft..."
        scope.launch {
            runCatching {
                if (!draftId.isNullOrBlank()) {
                    withContext(Dispatchers.IO) {
                        MobileMailCommandClient(JniMeronCore()).discardDraft(
                            DiscardDraftParams(accountId = accountId, draftId = draftId),
                        )
                    }
                }
            }.onSuccess {
                clearComposeDraftState()
                screen = previousTopScreen
                status = "Draft discarded"
                syncCoreThreads(syncFirst = false)
            }.onFailure {
                status = "Draft discard failed: ${it.message}"
            }
        }
    }

    fun sendQuickReply() {
        val thread = selectedCoreThread
        val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
        val parent = messages.lastOrNull()
        val replyBody = quickReplyBody.trim()
        if (accountId.isBlank() || thread == null || parent == null) {
            status = "Open a mail thread before replying."
            return
        }
        if (threadIdIsRss(thread.id)) {
            status = "RSS items do not support replies."
            return
        }
        if (replyBody.isBlank() && quickReplyAttachments.isEmpty()) {
            status = "Write a reply or attach a file before sending."
            return
        }
        quickReplyFailure = ""
        status = "Sending reply..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val replyFrom = coreAccounts.firstOrNull { it.id == accountId }
                        ?.let { detectReplyFromIdentity(parent, it) }
                        .orEmpty()
                    MobileMailCommandClient(JniMeronCore()).send(
                        parent.toReplyMailParams(
                            accountId = accountId,
                            body = replyBody,
                            from = replyFrom,
                            ownAddresses = ownAddressList(coreAccounts),
                            attachments = quickReplyAttachments,
                        ),
                    )
                }
            }.onSuccess {
                quickReplyBody = ""
                quickReplyAttachments = emptyList()
                quickReplyFailure = ""
                status = "Reply sent"
            }.onFailure {
                val message = it.message ?: "Send failed"
                quickReplyFailure = message
                status = "Reply failed: $message"
            }
        }
    }

    fun openMessageCompose(message: MessageBody, forward: Boolean) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    forwardableAttachments(message).mapNotNull { attachment ->
                        val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                        data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                    }
                }
            }.onSuccess { copiedAttachments ->
                val draft = if (forward) {
                    messageForwardDraft(message, copiedAttachments)
                } else {
                    messageEditAsNewDraft(message, copiedAttachments)
                }
                to = draft.to
                cc = draft.cc
                bcc = draft.bcc
                subject = draft.subject
                body = draft.body
                attachments = draft.attachments
                composeFromAccountId = selectedCoreThread?.accountId ?: selectedCoreAccountId.takeIf { it != UNIFIED_ACCOUNT_ID }.orEmpty()
                composeFromEmail = ""
                composeDraftId = ""
                composeDraftSaved = false
                previousTopScreen = Screen.Thread
                screen = Screen.Compose
                status = if (forward) "Forward draft ready" else "Copied message into compose"
            }.onFailure {
                status = if (forward) "Forward failed: ${it.message}" else "Edit as new failed: ${it.message}"
            }
        }
    }

    suspend fun readAttachmentBytes(attachment: MessageAttachment): ByteArray {
        val response = MobileMailCommandClient(JniMeronCore()).readAttachment(AttachmentReadParams(attachment.key))
        val data = parseAttachmentDataResponse(response)
        if (data.isBlank()) error("Attachment data is empty")
        return Base64.decode(data, Base64.DEFAULT)
    }

    fun saveMessageAttachment(attachment: MessageAttachment) {
        if (attachment.key.isBlank()) {
            status = if (attachment.url.isNotBlank()) "Remote attachments can be opened but are not cached for saving." else "Attachment is not cached."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        pendingAttachmentSave = attachment
        attachmentSavePicker.launch(safeAttachmentFilename(attachment.filename))
    }

    fun openMessageAttachment(attachment: MessageAttachment) {
        if (attachment.url.isNotBlank()) {
            runCatching {
                context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(attachment.url)))
            }.onFailure {
                status = "No app can open this attachment."
            }
            return
        }
        if (attachment.key.isBlank()) {
            status = "Attachment is not cached."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val bytes = readAttachmentBytes(attachment)
                    val dir = File(context.cacheDir, "attachments")
                    dir.mkdirs()
                    val file = File(dir, safeAttachmentFilename(attachment.filename))
                    file.writeBytes(bytes)
                    if (attachment.mimeType.startsWith("image/")) {
                        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                            ?: error("Attachment image could not be decoded")
                        AndroidImagePreview(
                            title = attachment.filename.ifBlank { "Image" },
                            bitmap = bitmap,
                            file = file,
                            mimeType = attachment.mimeType.ifBlank { "image/*" },
                        )
                    } else {
                        file
                    }
                }
            }.onSuccess { result ->
                if (result is AndroidImagePreview) {
                    imagePreview = result
                } else {
                    val file = result as File
                    val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
                    val viewIntent = Intent(Intent.ACTION_VIEW).apply {
                        setDataAndType(uri, attachment.mimeType.ifBlank { "application/octet-stream" })
                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    }
                    val chooser = Intent.createChooser(viewIntent, attachment.filename.ifBlank { "Open attachment" })
                    runCatching { context.startActivity(chooser) }
                        .onFailure { status = "No app can open this attachment." }
                }
            }.onFailure {
                status = "Attachment open failed: ${it.message}"
            }
        }
    }

    fun shareImagePreview(preview: AndroidImagePreview) {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", preview.file)
        val shareIntent = Intent(Intent.ACTION_SEND).apply {
            type = preview.mimeType.ifBlank { "image/*" }
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        runCatching {
            context.startActivity(Intent.createChooser(shareIntent, preview.title.ifBlank { "Share image" }))
        }.onFailure {
            status = "No app can share this image."
        }
    }

    fun copyImagePreview(preview: AndroidImagePreview) {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", preview.file)
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newUri(context.contentResolver, preview.title.ifBlank { "Image" }, uri))
        status = "Image copied."
    }

    fun openCompose() {
        to = ""; cc = ""; bcc = ""; subject = ""; body = ""; attachments = emptyList()
        composeDraftId = ""; composeDraftSaved = false
        previousTopScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
        screen = Screen.Compose
    }

    // Re-open account setup pre-filled so the user can fix credentials. OAuth
    // accounts re-run the browser sign-in; password accounts re-enter the
    // password (the IMAP/SMTP host fields keep their last values).
    fun reconnectAccount(account: AccountSummary) {
        val isOAuth = account.authType == "oauth" || account.provider == "gmail" || account.provider == "outlook"
        when {
            accountSummaryIsRss(account) -> {
                addSection = 2
            }
            isOAuth -> {
                oauthEmail = account.email
                if (account.provider == "gmail" || account.provider == "outlook") oauthProvider = account.provider
                oauthAuthorizationCode = ""
                addSection = 1
            }
            else -> {
                email = account.email
                username = account.email
                password = ""
                if (account.imapHost.isNotBlank()) host = account.imapHost
                if (account.imapPort > 0) imapPort = account.imapPort.toString()
                if (account.smtpHost.isNotBlank()) smtpHost = account.smtpHost
                if (account.smtpPort > 0) smtpPort = account.smtpPort.toString()
                addSection = 0
            }
        }
        errorBanner = null
        previousTopScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
        screen = Screen.AddAccount
    }

    fun createKanbanBoard() {
        val board = defaultKanbanBoard(coreAccounts).copy(name = "Kanban board ${kanbanBoards.size + 1}")
        persistKanbanBoards(kanbanBoards + board)
        activeKanbanBoardId = board.id
        saveActiveKanbanBoardId(context, board.id)
        loadKanbanBoard(refresh = false)
    }

    fun renameActiveKanbanBoard(name: String) {
        val trimmed = name.trim()
        if (trimmed.isBlank()) return
        persistKanbanBoards(kanbanBoards.map { if (it.id == activeKanbanBoardId) it.copy(name = trimmed) else it })
    }

    fun updateActiveKanbanBoardAppearance(avatarUrl: String, wallpaperPresetId: String, wallpaperUrl: String) {
        persistKanbanBoards(kanbanBoards.map {
            if (it.id == activeKanbanBoardId) {
                it.copy(
                    avatarUrl = avatarUrl.trim(),
                    wallpaperPresetId = wallpaperPresetId.trim(),
                    wallpaperUrl = wallpaperUrl.trim(),
                )
            } else {
                it
            }
        })
    }

    fun deleteActiveKanbanBoard() {
        val next = kanbanBoards.filterNot { it.id == activeKanbanBoardId }
        persistKanbanBoards(if (next.isEmpty()) listOf(defaultKanbanBoard(coreAccounts)) else next)
        kanbanColumns = emptyMap()
        loadKanbanBoard(refresh = false)
    }

    fun addKanbanColumn(column: KanbanColumnSpec) {
        val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
        if (board.columns.any { kanbanColumnKey(it) == kanbanColumnKey(column) }) return
        persistKanbanBoards(kanbanBoards.map {
            if (it.id == board.id) it.copy(columns = it.columns + column) else it
        })
        loadKanbanColumn(column, refresh = true)
    }

    fun removeKanbanColumn(column: KanbanColumnSpec) {
        val key = kanbanColumnKey(column)
        persistKanbanBoards(kanbanBoards.map {
            if (it.id == activeKanbanBoardId) it.copy(columns = it.columns.filterNot { existing -> kanbanColumnKey(existing) == key }) else it
        })
        kanbanColumns = kanbanColumns - key
    }

    fun moveKanbanColumn(column: KanbanColumnSpec, delta: Int) {
        persistKanbanBoards(kanbanBoards.map { board ->
            if (board.id != activeKanbanBoardId) return@map board
            val columns = board.columns.toMutableList()
            val index = columns.indexOfFirst { kanbanColumnKey(it) == kanbanColumnKey(column) }
            val target = (index + delta).coerceIn(0, columns.lastIndex)
            if (index < 0 || index == target) board else {
                val item = columns.removeAt(index)
                columns.add(target, item)
                board.copy(columns = columns)
            }
        })
    }

    fun createFolderForKanban(account: AccountSummary, name: String) {
        val trimmed = name.trim()
        if (trimmed.isBlank()) {
            status = "Folder name is required."
            return
        }
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.createFolder(FolderCreateParams(accountId = account.id, name = trimmed))
                    loadAccountFolders(client, account)
                }
            }.onSuccess { folders ->
                foldersByAccount = foldersByAccount + (account.id to folders)
                val created = folders.firstOrNull { it.name.equals(trimmed, ignoreCase = true) }?.name ?: trimmed
                addKanbanColumn(KanbanColumnSpec(account.id, created))
                showKanbanCreateFolderDialog = null
                kanbanFolderNameInput = ""
                status = "Folder created"
            }.onFailure {
                status = "Create folder failed: ${it.message}"
            }
        }
    }

    // Load persisted accounts once on startup so they survive app restarts.
    LaunchedEffect(Unit) {
        if (MeronCoreNative.isLoaded() && coreAccounts.isEmpty()) {
            listAccounts()
            loadStorageUsage()
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
    val selectedThreadPreferHtml = selectedThreadAccount?.let { account ->
        conversationHtmlOverrides[account.id] ?: account.conversationHtml
    } ?: true
    val activeKanbanBoard = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: kanbanBoards.firstOrNull()
    val appBarTitle = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) "Unified inbox" else "Inbox"
    val appBarSubtitle = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
        "All accounts"
    } else {
        selectedAccount?.email?.ifBlank { selectedAccount.displayName }.orEmpty()
    }

    // Hardware back from a sub-screen returns to the inbox instead of exiting.
    BackHandler(enabled = screen != Screen.Mail) {
        screen = if (screen == Screen.Thread || screen == Screen.Compose || screen == Screen.AddAccount || screen == Screen.Settings) previousTopScreen else Screen.Mail
    }

    when (screen) {
        Screen.Thread -> ThreadScreen(
            thread = selectedCoreThread,
            messages = messages,
            accountEmail = selectedThreadAccount?.email.orEmpty(),
            preferHtml = selectedThreadPreferHtml,
            onPreferHtmlChange = { preferHtml ->
                if (selectedThreadAccountId.isNotBlank()) {
                    conversationHtmlOverrides = conversationHtmlOverrides + (selectedThreadAccountId to preferHtml)
                }
            },
            onBack = { screen = previousTopScreen },
            onArchive = { selectedCoreThread?.let { archiveOrRemove(it); screen = previousTopScreen } },
            onDelete = { selectedCoreThread?.let { deleteThread(it); screen = previousTopScreen } },
            onToggleStar = { selectedCoreThread?.let { t -> toggleStar(t); selectedCoreThread = t.copy(starred = !t.starred) } },
            moveFolders = selectedCoreThread
                ?.let { thread -> foldersByAccount[thread.accountId].orEmpty() }
                .orEmpty(),
            copyFolders = coreAccounts
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

        Screen.Compose -> ComposeScreen(
            sendIdentities = composeIdentityCandidates(),
            selectedFromKey = selectedComposeIdentity()?.let { identityKey(it) }.orEmpty(),
            onFromChange = { key ->
                val split = key.indexOf('|')
                if (split > 0) {
                    composeFromAccountId = key.substring(0, split)
                    composeFromEmail = key.substring(split + 1)
                }
            },
            to = to, onToChange = { to = it; loadRecipientSuggestions("to", it) },
            cc = cc, onCcChange = { cc = it; loadRecipientSuggestions("cc", it) },
            bcc = bcc, onBccChange = { bcc = it; loadRecipientSuggestions("bcc", it) },
            subject = subject, onSubjectChange = { subject = it },
            body = body, onBodyChange = { body = it },
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

        Screen.AddAccount -> AddAccountScreen(
            onBack = { screen = previousTopScreen },
            initialSection = addSection,
            displayName = displayName, onDisplayNameChange = { displayName = it },
            senderName = senderName, onSenderNameChange = { senderName = it },
            email = email, onEmailChange = { email = it },
            username = username, onUsernameChange = { username = it },
            password = password, onPasswordChange = { password = it },
            host = host, onHostChange = { host = it },
            imapPort = imapPort, onImapPortChange = { imapPort = it },
            smtpHost = smtpHost, onSmtpHostChange = { smtpHost = it },
            smtpPort = smtpPort, onSmtpPortChange = { smtpPort = it },
            onAutodiscover = ::autodiscoverPasswordAccount,
            onAddPassword = ::addPasswordAccount,
            oauthProvider = oauthProvider, onOauthProviderChange = { oauthProvider = it },
            oauthEmail = oauthEmail, onOauthEmailChange = { oauthEmail = it },
            oauthClientId = oauthClientId, onOauthClientIdChange = { oauthClientId = it },
            oauthClientSecret = oauthClientSecret, onOauthClientSecretChange = { oauthClientSecret = it },
            oauthRedirectUri = oauthRedirectUri, onOauthRedirectUriChange = { oauthRedirectUri = it },
            oauthAuthorizationCode = oauthAuthorizationCode,
            oauthAccessToken = oauthAccessToken, onOauthAccessTokenChange = { oauthAccessToken = it },
            oauthRefreshToken = oauthRefreshToken, onOauthRefreshTokenChange = { oauthRefreshToken = it },
            oauthExpiresAt = oauthExpiresAt, onOauthExpiresAtChange = { oauthExpiresAt = it },
            onLaunchOAuth = ::launchOAuthFlow,
            onExchangeOAuth = ::exchangeOAuthCode,
            onAddOAuth = ::addOAuthAccount,
            rssFeedUrl = rssFeedUrl, onRssFeedUrlChange = { rssFeedUrl = it },
            rssDisplayName = rssDisplayName, onRssDisplayNameChange = { rssDisplayName = it },
            onAddRss = ::addRssAccount,
            diagnostics = coreStatus(coreInitJson),
        )

        Screen.Settings -> {
            LaunchedEffect(Unit) { loadStorageUsage() }
            SettingsScreen(
                onBack = { screen = previousTopScreen },
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

        Screen.Starred -> ModalNavigationDrawer(
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
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            Column {
                                Text("Starred", fontWeight = FontWeight.SemiBold)
                                Text("All starred messages and feed items", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = "Open navigation")
                            }
                        },
                        actions = {
                            IconButton(onClick = ::loadStarredItems) {
                                Icon(Icons.Filled.Refresh, contentDescription = "Refresh starred")
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
                            title = "No starred items",
                            text = "Star messages or feed items to collect them here.",
                            actionLabel = "Refresh",
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

        Screen.Kanban -> ModalNavigationDrawer(
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
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                KanbanBoardTile(activeKanbanBoard, 36.dp)
                                Column {
                                    Text(activeKanbanBoard?.name ?: "Kanban board", fontWeight = FontWeight.SemiBold)
                                    Text(
                                        "${activeKanbanBoard?.columns?.size ?: 0} columns${if (activeKanbanBoard?.hasBoardStyle() == true) " · styled" else ""}",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = "Open navigation")
                            }
                        },
                        actions = {
                            IconButton(onClick = {
                                showKanbanBoardDialog = true
                                kanbanBoardNameInput = activeKanbanBoard?.name.orEmpty()
                                kanbanBoardAvatarInput = activeKanbanBoard?.avatarUrl.orEmpty()
                                kanbanBoardWallpaperPresetInput = activeKanbanBoard?.wallpaperPresetId.orEmpty()
                                kanbanBoardWallpaperUrlInput = activeKanbanBoard?.wallpaperUrl.orEmpty()
                            }) {
                                Icon(Icons.Filled.MoreVert, contentDescription = "Board menu")
                            }
                            IconButton(onClick = {
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
                            }) {
                                Icon(Icons.Filled.Add, contentDescription = "Add column")
                            }
                            IconButton(onClick = { loadKanbanBoard(refresh = true) }) {
                                Icon(Icons.Filled.Refresh, contentDescription = "Refresh board")
                            }
                        },
                    )
                },
                floatingActionButton = {
                    if (coreAccounts.any { !accountSummaryIsRss(it) }) {
                        ExtendedFloatingActionButton(
                            onClick = ::openCompose,
                            icon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                            text = { Text("Compose") },
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
                    onFilter = ::persistKanbanFilter,
                    onSearch = ::persistKanbanSearch,
                    onOpen = ::readCoreThread,
                    onLongPress = { kanbanActionThread = it },
                    onToggleStar = ::toggleStar,
                    onArchive = ::archiveOrRemove,
                    onRefreshColumn = { loadKanbanColumn(it, refresh = true) },
                    onLoadMoreColumn = ::loadMoreKanbanColumn,
                    onMarkColumnAllRead = ::markKanbanColumnAllRead,
                    onRemoveColumn = ::removeKanbanColumn,
                    onMoveColumn = ::moveKanbanColumn,
                    onAddColumn = { showKanbanColumnDialog = true },
                    showSenderImages = showSenderImages,
                    kanbanColumnWidth = kanbanColumnWidth.dp,
                )
            }
        }

        Screen.Mail -> ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts,
                    selectedAccountId = selectedCoreAccountId,
                    folders = coreFolders,
                    currentScreen = screen,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
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
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            Column {
                                Text(appBarTitle, fontWeight = FontWeight.SemiBold)
                                if (appBarSubtitle.isNotBlank()) {
                                    Text(
                                        appBarSubtitle,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = "Open navigation")
                            }
                        },
                        actions = {
                            if (coreThreads.any { it.unread }) {
                                IconButton(onClick = ::markVisibleMailboxAllRead) {
                                    Icon(Icons.Filled.MarkEmailUnread, contentDescription = "Mark all read")
                                }
                            }
                            if (selectedAccount != null) {
                                Box {
                                    IconButton(onClick = { mailboxMenuOpen = true }) {
                                        Icon(Icons.Filled.MoreVert, contentDescription = "Mailbox actions")
                                    }
                                    DropdownMenu(expanded = mailboxMenuOpen, onDismissRequest = { mailboxMenuOpen = false }) {
                                        DropdownMenuItem(
                                            text = { Text("Account settings") },
                                            onClick = {
                                                mailboxMenuOpen = false
                                                showAccountSettings = true
                                            },
                                        )
                                        if (selectedAccount.let(::accountSummaryIsRss)) {
                                            DropdownMenuItem(
                                                text = { Text("Add feed") },
                                                onClick = {
                                                    mailboxMenuOpen = false
                                                    addFeedUrl = ""
                                                    showAddFeedDialog = true
                                                },
                                            )
                                            DropdownMenuItem(
                                                text = { Text("Import OPML") },
                                                onClick = {
                                                    mailboxMenuOpen = false
                                                    opmlImportPicker.launch(arrayOf("text/xml", "application/xml", "text/*", "*/*"))
                                                },
                                            )
                                            DropdownMenuItem(
                                                text = { Text("Export OPML") },
                                                onClick = {
                                                    mailboxMenuOpen = false
                                                    exportOpmlForSelectedAccount()
                                                },
                                            )
                                        }
                                    }
                                }
                            }
                            IconButton(onClick = ::syncCoreThreads) {
                                Icon(Icons.Filled.Refresh, contentDescription = "Sync")
                            }
                        },
                    )
                },
                floatingActionButton = {
                    if (coreAccounts.isNotEmpty()) {
                        ExtendedFloatingActionButton(
                            onClick = ::openCompose,
                            icon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                            text = { Text("Compose") },
                        )
                    }
                },
                snackbarHost = { SnackbarHost(snackbarHost) },
            ) { innerPadding ->
                Column(Modifier.fillMaxSize().padding(innerPadding)) {
                    val reconnectAccount2 = selectedAccount?.takeIf { it.needsReconnect }
                    when {
                        reconnectAccount2 != null -> StatusBanner(
                            message = "Can't sign in to ${reconnectAccount2.email.ifBlank { reconnectAccount2.displayName }}. Update the credentials to reconnect.",
                            isError = true,
                            actionLabel = "Reconnect",
                            onAction = { reconnectAccount(reconnectAccount2) },
                            onDismiss = null,
                        )
                        errorBanner != null -> {
                            val authLike = isAuthError(errorBanner!!)
                            StatusBanner(
                                message = errorBanner!!,
                                isError = true,
                                actionLabel = if (authLike && selectedAccount != null) "Reconnect" else "Retry",
                                onAction = {
                                    if (authLike && selectedAccount != null) reconnectAccount(selectedAccount)
                                    else syncCoreThreads()
                                },
                                onDismiss = { errorBanner = null },
                            )
                        }
                    }
                    MailSearchFilterBar(
                        search = mailSearch,
                        filter = mailFilter,
                        onSearchChange = { mailSearch = it },
                        onSearchSubmit = { syncCoreThreads(syncFirst = false) },
                        onFilterChange = {
                            mailFilter = it
                            syncCoreThreads(syncFirst = false)
                        },
                    )
                    PullToRefreshBox(
                        isRefreshing = syncing,
                        onRefresh = { syncCoreThreads() },
                        modifier = Modifier.fillMaxSize(),
                    ) {
                        when {
                            coreAccounts.isEmpty() -> EmptyState(
                                icon = Icons.Filled.PersonAdd,
                                title = "Welcome to Meron",
                                text = "Add a mail or RSS account to start reading your inbox.",
                                actionLabel = "Add account",
                                onAction = { addSection = 0; screen = Screen.AddAccount },
                            )
                            coreThreads.isEmpty() -> EmptyState(
                                icon = Icons.Outlined.Drafts,
                                title = if (mailSearch.isBlank() && mailFilter == FilterMode.All) "Nothing here yet" else "No matching mail",
                                text = if (mailSearch.isBlank() && mailFilter == FilterMode.All) {
                                    "Pull in your latest messages from the server."
                                } else {
                                    "Adjust the search or filter, then try again."
                                },
                                actionLabel = "Sync now",
                                onAction = ::syncCoreThreads,
                            )
                            else -> MailList(
                                threads = coreThreads,
                                canLoadMore = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                                    mailboxAccountCursors.isNotEmpty()
                                } else {
                                    mailboxCursor.isNotBlank()
                                },
                                loadingMore = loadingMoreThreads,
                                onOpen = ::readCoreThread,
                                onToggleStar = ::toggleStar,
                                onArchive = ::archiveOrRemove,
                                onCopyFeedUrl = { thread ->
                                    copyToClipboard(context, "Feed URL", thread.feedUrl)
                                    status = "Copied feed URL"
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

    if (showKanbanBoardDialog && screen == Screen.Kanban) {
        KanbanBoardDialog(
            boards = kanbanBoards,
            activeBoardId = activeKanbanBoardId,
            name = kanbanBoardNameInput,
            avatarUrl = kanbanBoardAvatarInput,
            wallpaperPresetId = kanbanBoardWallpaperPresetInput,
            wallpaperUrl = kanbanBoardWallpaperUrlInput,
            onNameChange = { kanbanBoardNameInput = it },
            onAvatarUrlChange = { kanbanBoardAvatarInput = it },
            onWallpaperPresetChange = { kanbanBoardWallpaperPresetInput = it },
            onWallpaperUrlChange = { kanbanBoardWallpaperUrlInput = it },
            onSelect = {
                activeKanbanBoardId = it
                saveActiveKanbanBoardId(context, it)
                val board = kanbanBoards.firstOrNull { board -> board.id == it }
                kanbanBoardNameInput = board?.name.orEmpty()
                kanbanBoardAvatarInput = board?.avatarUrl.orEmpty()
                kanbanBoardWallpaperPresetInput = board?.wallpaperPresetId.orEmpty()
                kanbanBoardWallpaperUrlInput = board?.wallpaperUrl.orEmpty()
                showKanbanBoardDialog = false
                loadKanbanBoard(refresh = false)
            },
            onRename = { renameActiveKanbanBoard(kanbanBoardNameInput) },
            onSaveAppearance = {
                updateActiveKanbanBoardAppearance(
                    kanbanBoardAvatarInput,
                    kanbanBoardWallpaperPresetInput,
                    kanbanBoardWallpaperUrlInput,
                )
            },
            onCreate = { createKanbanBoard() },
            onDelete = { deleteActiveKanbanBoard(); showKanbanBoardDialog = false },
            onDismiss = { showKanbanBoardDialog = false },
        )
    }

    if (showAccountSettings) {
        selectedAccount?.let { account ->
            val accountIndex = coreAccounts.indexOfFirst { it.id == account.id }
            AccountSettingsDialog(
                account = account,
                canMoveUp = accountIndex > 0,
                canMoveDown = accountIndex >= 0 && accountIndex < coreAccounts.lastIndex,
                showInNavigation = account.id !in hiddenNavigationAccountIds,
                onSave = { displayName, senderName, avatarUrl, wallpaperPresetId, loadRemoteImages, conversationHtml, includedInUnified, showInNavigation, muted, paused, interval, aliases ->
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
                onPickAvatar = {
                    accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = false)
                    accountMediaPicker.launch(arrayOf("image/*"))
                },
                onPickWallpaper = {
                    accountMediaUploadTarget = AccountMediaUploadTarget(account, wallpaper = true)
                    accountMediaPicker.launch(arrayOf("image/*"))
                },
                onMoveUp = { moveAccount(account, -1) },
                onMoveDown = { moveAccount(account, 1) },
                onRemove = { removeAccount(account) },
                onDismiss = { showAccountSettings = false },
            )
        } ?: run {
            showAccountSettings = false
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
            onOpen = { kanbanActionThread = null; readCoreThread(thread) },
            onToggleStar = { kanbanActionThread = null; toggleStar(thread) },
            onToggleRead = { kanbanActionThread = null; toggleRead(thread) },
            onArchive = { kanbanActionThread = null; archiveOrRemove(thread) },
            onDelete = { kanbanActionThread = null; deleteThread(thread) },
            onCopyFeedUrl = {
                kanbanActionThread = null
                copyToClipboard(context, "Feed URL", thread.feedUrl)
                status = "Copied feed URL"
            },
            onMove = { target -> kanbanActionThread = null; moveThreadToColumn(thread, target) },
        )
    }
}
