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
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
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

class ComposeMainActivity : ComponentActivity() {
    private var incomingMailtoDraft by mutableStateOf<ComposeDraft?>(null)
    private var incomingOAuthCallbackUrl by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        incomingMailtoDraft = intent.toMailtoDraft()
        incomingOAuthCallbackUrl = intent.toOAuthCallbackUrl()
        AndroidNotificationService.ensureChannels(this)
        AndroidBackgroundSyncScheduler.schedule(this)
        val coreInitJson = if (MeronCoreNative.isLoaded()) {
            MeronCoreNative.initJson(filesDir.absolutePath, MeronDbKey.get(this))
        } else {
            ""
        }
        setContent {
            var appearanceMode by remember { mutableStateOf(loadAppearanceMode(this)) }
            MeronTheme(appearanceMode = appearanceMode) {
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    MeronMobileScreen(
                        coreInitJson = coreInitJson,
                        incomingMailtoDraft = incomingMailtoDraft,
                        incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
                        appearanceMode = appearanceMode,
                        onAppearanceModeChange = { mode ->
                            appearanceMode = mode
                            saveAppearanceMode(this, mode)
                        },
                    )
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        incomingMailtoDraft = intent.toMailtoDraft()
        incomingOAuthCallbackUrl = intent.toOAuthCallbackUrl()
    }

    fun currentMailtoDraftForTesting(): ComposeDraft? = incomingMailtoDraft
    fun currentOAuthCallbackUrlForTesting(): String? = incomingOAuthCallbackUrl
}

private enum class Screen { Mail, Starred, Kanban, Thread, Compose, AddAccount }

private data class AccountMediaUploadTarget(
    val account: AccountSummary,
    val wallpaper: Boolean,
)

private data class AndroidImagePreview(
    val title: String,
    val bitmap: Bitmap,
    val file: File,
    val mimeType: String,
)

private enum class FilterMode { All, Unread, Starred }
private enum class SendShortcutMode { Enter, ModEnter }

private const val UNIFIED_ACCOUNT_ID = "unified"
private const val INBOX_FOLDER = "inbox"
private const val KANBAN_PREFS = "kanban"
private const val KANBAN_BOARDS_PREF = "kanban_boards_v1"
private const val ACTIVE_KANBAN_BOARD_PREF = "active_kanban_board_id_v1"
private const val KANBAN_FILTER_PREF = "kanban_filter_v1"
private const val KANBAN_SEARCH_PREF = "kanban_search_v1"
private const val APP_PREFS = "meron_app_prefs"
private const val APPEARANCE_MODE_PREF = "appearance_mode_v1"
private const val SHOW_UNREAD_BADGES_PREF = "show_unread_badges_v1"
private const val SHOW_UNIFIED_INBOX_PREF = "show_unified_inbox_v1"
private const val SHOW_STARRED_NAV_PREF = "show_starred_nav_v1"
private const val SHOW_SENDER_IMAGES_PREF = "show_sender_images_v1"
private const val SEND_SHORTCUT_PREF = "send_shortcut_v1"
private const val HIDDEN_NAV_ACCOUNTS_PREF = "hidden_navigation_accounts_v1"
private const val KANBAN_COLUMN_WIDTH_PREF = "kanban_column_width_v1"
private const val KANBAN_COLUMN_MIN_WIDTH = 240
private const val KANBAN_COLUMN_DEFAULT_WIDTH = 320
private const val KANBAN_COLUMN_MAX_WIDTH = 520
private const val KANBAN_COLUMN_WIDTH_STEP = 20

private data class MailboxLoadResult(
    val folders: List<FolderSummary>,
    val folder: String,
    val threads: List<ThreadSummary>,
    val nextCursor: String = "",
    val accountCursors: Map<String, String> = emptyMap(),
)

private data class KanbanColumnSpec(
    val accountId: String,
    val folderId: String,
)

private data class KanbanBoardSpec(
    val id: String,
    val name: String,
    val columns: List<KanbanColumnSpec>,
    val avatarUrl: String = "",
    val wallpaperPresetId: String = "",
    val wallpaperUrl: String = "",
)

private data class KanbanColumnState(
    val threads: List<ThreadSummary> = emptyList(),
    val loading: Boolean = false,
    val loadingMore: Boolean = false,
    val error: String? = null,
    val nextCursor: String = "",
    val accountCursors: Map<String, String> = emptyMap(),
)

private data class ConversationParticipant(
    val name: String,
    val email: String,
    val count: Int,
    val isSelf: Boolean,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MeronMobileScreen(
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

    var host by remember { mutableStateOf("10.0.2.2") }
    var email by remember { mutableStateOf("user1@mail.localhost") }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("user1password") }
    var displayName by remember { mutableStateOf("Local Test") }
    var senderName by remember { mutableStateOf("Local Test") }
    var imapPort by remember { mutableStateOf("993") }
    var smtpHost by remember { mutableStateOf("10.0.2.2") }
    var smtpPort by remember { mutableStateOf("465") }
    var oauthProvider by remember { mutableStateOf("gmail") }
    var oauthEmail by remember { mutableStateOf("me@gmail.com") }
    var oauthAccessToken by remember { mutableStateOf("") }
    var oauthRefreshToken by remember { mutableStateOf("") }
    var oauthExpiresAt by remember { mutableStateOf("0") }
    var oauthClientId by remember { mutableStateOf("") }
    var oauthClientSecret by remember { mutableStateOf("") }
    var oauthRedirectUri by remember { mutableStateOf(defaultOAuthRedirectUri()) }
    var oauthState by remember { mutableStateOf(UUID.randomUUID().toString()) }
    var oauthVerifier by remember { mutableStateOf(UUID.randomUUID().toString() + UUID.randomUUID().toString()) }
    var oauthAuthorizationCode by remember { mutableStateOf("") }
    var rssFeedUrl by remember { mutableStateOf("https://example.com/feed.xml") }
    var rssDisplayName by remember { mutableStateOf("Example Feed") }
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
                status = "$label complete"
            }.onFailure {
                status = "$label failed: ${it.message}"
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
            runCoreThreadAction(
                thread = thread,
                label = "Archive",
                action = { archive(ThreadActionParams(threadId = thread.id)) },
                update = { threads -> threads.filterNot { it.id == thread.id } },
            )
        }
    }

    fun deleteThread(thread: ThreadSummary) {
        runCoreThreadAction(
            thread = thread,
            label = threadDeleteActionLabel(thread.folder),
            action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
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
        screen = if (screen == Screen.Thread || screen == Screen.Compose || screen == Screen.AddAccount) previousTopScreen else Screen.Mail
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

        Screen.Starred -> ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts.filterNot { it.id in hiddenNavigationAccountIds },
                    selectedAccountId = selectedCoreAccountId,
                    folders = coreFolders,
                    currentScreen = screen,
                    notificationsNeedPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted,
                    appearanceMode = appearanceMode,
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
                    appVersion = appVersion,
                    showSenderImages = showSenderImages,
                    sendShortcutMode = sendShortcutMode,
                    kanbanColumnWidth = kanbanColumnWidth,
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
                    onRefreshBackground = {
                        AndroidBackgroundSyncScheduler.runOnce(context)
                        status = "Queued background refresh"
                        scope.launch { drawerState.close() }
                    },
                    onEnableNotifications = {
                        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                        scope.launch { drawerState.close() }
                    },
                    onAppearanceModeChange = onAppearanceModeChange,
                    onToggleUnreadBadges = {
                        showUnreadBadges = !showUnreadBadges
                        saveAppBoolean(context, SHOW_UNREAD_BADGES_PREF, showUnreadBadges)
                    },
                    onToggleUnifiedInboxNav = {
                        showUnifiedInboxNav = !showUnifiedInboxNav
                        saveAppBoolean(context, SHOW_UNIFIED_INBOX_PREF, showUnifiedInboxNav)
                    },
                    onToggleStarredNav = {
                        showStarredNav = !showStarredNav
                        saveAppBoolean(context, SHOW_STARRED_NAV_PREF, showStarredNav)
                    },
                    onToggleSenderImages = {
                        showSenderImages = !showSenderImages
                        saveAppBoolean(context, SHOW_SENDER_IMAGES_PREF, showSenderImages)
                    },
                    onToggleSendShortcut = {
                        val next = sendShortcutMode.next()
                        sendShortcutMode = next
                        saveSendShortcutMode(context, next)
                    },
                    onCycleKanbanColumnWidth = {
                        val next = nextKanbanColumnWidth(kanbanColumnWidth)
                        kanbanColumnWidth = next
                        saveAppInt(context, KANBAN_COLUMN_WIDTH_PREF, next)
                    },
                    onRefreshStorage = { loadStorageUsage(showStatus = true) },
                    onClearStorageCache = ::clearStorageCache,
                    onShowAbout = {
                        showAboutDialog = true
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
                    notificationsNeedPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted,
                    appearanceMode = appearanceMode,
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
                    appVersion = appVersion,
                    showSenderImages = showSenderImages,
                    sendShortcutMode = sendShortcutMode,
                    kanbanColumnWidth = kanbanColumnWidth,
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
                    onRefreshBackground = {
                        AndroidBackgroundSyncScheduler.runOnce(context)
                        status = "Queued background refresh"
                        scope.launch { drawerState.close() }
                    },
                    onEnableNotifications = {
                        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                        scope.launch { drawerState.close() }
                    },
                    onAppearanceModeChange = onAppearanceModeChange,
                    onToggleUnreadBadges = {
                        showUnreadBadges = !showUnreadBadges
                        saveAppBoolean(context, SHOW_UNREAD_BADGES_PREF, showUnreadBadges)
                    },
                    onToggleUnifiedInboxNav = {
                        showUnifiedInboxNav = !showUnifiedInboxNav
                        saveAppBoolean(context, SHOW_UNIFIED_INBOX_PREF, showUnifiedInboxNav)
                    },
                    onToggleStarredNav = {
                        showStarredNav = !showStarredNav
                        saveAppBoolean(context, SHOW_STARRED_NAV_PREF, showStarredNav)
                    },
                    onToggleSenderImages = {
                        showSenderImages = !showSenderImages
                        saveAppBoolean(context, SHOW_SENDER_IMAGES_PREF, showSenderImages)
                    },
                    onToggleSendShortcut = {
                        val next = sendShortcutMode.next()
                        sendShortcutMode = next
                        saveSendShortcutMode(context, next)
                    },
                    onCycleKanbanColumnWidth = {
                        val next = nextKanbanColumnWidth(kanbanColumnWidth)
                        kanbanColumnWidth = next
                        saveAppInt(context, KANBAN_COLUMN_WIDTH_PREF, next)
                    },
                    onRefreshStorage = { loadStorageUsage(showStatus = true) },
                    onClearStorageCache = ::clearStorageCache,
                    onShowAbout = {
                        showAboutDialog = true
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
                    notificationsNeedPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted,
                    appearanceMode = appearanceMode,
                    storageUsage = storageUsage,
                    storageBusy = storageBusy,
                    storageClearConfirming = storageClearConfirming,
                    showUnreadBadges = showUnreadBadges,
                    showUnifiedInboxNav = showUnifiedInboxNav,
                    showStarredNav = showStarredNav,
                    appVersion = appVersion,
                    showSenderImages = showSenderImages,
                    sendShortcutMode = sendShortcutMode,
                    kanbanColumnWidth = kanbanColumnWidth,
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
                    onRefreshBackground = {
                        AndroidBackgroundSyncScheduler.runOnce(context)
                        status = "Queued background refresh"
                        scope.launch { drawerState.close() }
                    },
                    onEnableNotifications = {
                        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                        scope.launch { drawerState.close() }
                    },
                    onAppearanceModeChange = onAppearanceModeChange,
                    onToggleUnreadBadges = {
                        showUnreadBadges = !showUnreadBadges
                        saveAppBoolean(context, SHOW_UNREAD_BADGES_PREF, showUnreadBadges)
                    },
                    onToggleUnifiedInboxNav = {
                        showUnifiedInboxNav = !showUnifiedInboxNav
                        saveAppBoolean(context, SHOW_UNIFIED_INBOX_PREF, showUnifiedInboxNav)
                    },
                    onToggleStarredNav = {
                        showStarredNav = !showStarredNav
                        saveAppBoolean(context, SHOW_STARRED_NAV_PREF, showStarredNav)
                    },
                    onToggleSenderImages = {
                        showSenderImages = !showSenderImages
                        saveAppBoolean(context, SHOW_SENDER_IMAGES_PREF, showSenderImages)
                    },
                    onToggleSendShortcut = {
                        val next = sendShortcutMode.next()
                        sendShortcutMode = next
                        saveSendShortcutMode(context, next)
                    },
                    onCycleKanbanColumnWidth = {
                        val next = nextKanbanColumnWidth(kanbanColumnWidth)
                        kanbanColumnWidth = next
                        saveAppInt(context, KANBAN_COLUMN_WIDTH_PREF, next)
                    },
                    onRefreshStorage = { loadStorageUsage(showStatus = true) },
                    onClearStorageCache = ::clearStorageCache,
                    onShowAbout = {
                        showAboutDialog = true
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
                    Box(Modifier.fillMaxSize()) {
                        if (syncing) {
                            CircularProgressIndicator(
                                Modifier.padding(top = 4.dp).align(Alignment.TopCenter).size(28.dp),
                            )
                        }
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

@Composable
private fun MailSearchFilterBar(
    search: String,
    filter: FilterMode,
    onSearchChange: (String) -> Unit,
    onSearchSubmit: () -> Unit,
    onFilterChange: (FilterMode) -> Unit,
) {
    Column(
        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface).padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = search,
            onValueChange = onSearchChange,
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("Search mail") },
            leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
            trailingIcon = {
                if (search.isNotBlank()) {
                    IconButton(onClick = {
                        onSearchChange("")
                        onSearchSubmit()
                    }) {
                        Icon(Icons.Filled.Close, contentDescription = "Clear search")
                    }
                }
            },
            singleLine = true,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterMode.values().forEach { mode ->
                FilterChip(
                    selected = filter == mode,
                    onClick = { onFilterChange(mode) },
                    label = { Text(mode.label()) },
                )
            }
            Spacer(Modifier.weight(1f))
            TextButton(onClick = onSearchSubmit) {
                Text("Apply")
            }
        }
    }
}

@Composable
private fun KanbanScreen(
    modifier: Modifier,
    accounts: List<AccountSummary>,
    board: KanbanBoardSpec?,
    columns: Map<String, KanbanColumnState>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    filter: FilterMode,
    search: String,
    onFilter: (FilterMode) -> Unit,
    onSearch: (String) -> Unit,
    onOpen: (ThreadSummary) -> Unit,
    onLongPress: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onArchive: (ThreadSummary) -> Unit,
    onRefreshColumn: (KanbanColumnSpec) -> Unit,
    onLoadMoreColumn: (KanbanColumnSpec) -> Unit,
    onMarkColumnAllRead: (KanbanColumnSpec) -> Unit,
    onRemoveColumn: (KanbanColumnSpec) -> Unit,
    onMoveColumn: (KanbanColumnSpec, Int) -> Unit,
    onAddColumn: () -> Unit,
    showSenderImages: Boolean,
    kanbanColumnWidth: Dp,
) {
    if (accounts.isEmpty()) {
        EmptyState(
            icon = Icons.Filled.PersonAdd,
            title = "Welcome to Meron",
            text = "Add an account to use Kanban.",
            actionLabel = "Add account",
            onAction = {},
        )
        return
    }
    val boardColumns = board?.columns.orEmpty()
    val boardBackground = boardBackgroundBrush(board)
    Column(
        modifier.then(
            if (boardBackground != null) {
                Modifier.background(boardBackground)
            } else {
                Modifier.background(MaterialTheme.colorScheme.background)
            },
        ),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (board?.hasBoardStyle() == true) {
                KanbanBoardTile(board, 38.dp)
            }
            OutlinedTextField(
                value = search,
                onValueChange = onSearch,
                placeholder = { Text("Search board") },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            FilterModeButton(filter, onFilter)
        }
        if (boardColumns.isEmpty()) {
            EmptyState(
                icon = Icons.Filled.ViewKanban,
                title = "No columns",
                text = "Add a column to start using this board.",
                actionLabel = "Add column",
                onAction = onAddColumn,
            )
        } else {
            LazyRow(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 88.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(boardColumns, key = { kanbanColumnKey(it) }) { column ->
                    KanbanColumn(
                        column = column,
                        state = columns[kanbanColumnKey(column)] ?: KanbanColumnState(),
                        accounts = accounts,
                        foldersByAccount = foldersByAccount,
                        filter = filter,
                        search = search,
                        onOpen = onOpen,
                        onLongPress = onLongPress,
                        onToggleStar = onToggleStar,
                        onArchive = onArchive,
                        onRefresh = { onRefreshColumn(column) },
                        onLoadMore = { onLoadMoreColumn(column) },
                        onMarkAllRead = { onMarkColumnAllRead(column) },
                        onRemove = { onRemoveColumn(column) },
                        onMoveLeft = { onMoveColumn(column, -1) },
                        onMoveRight = { onMoveColumn(column, 1) },
                        showSenderImages = showSenderImages,
                        kanbanColumnWidth = kanbanColumnWidth,
                    )
                }
            }
        }
    }
}

@Composable
private fun FilterModeButton(value: FilterMode, onChange: (FilterMode) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { expanded = true }) {
            Icon(Icons.Filled.FilterList, contentDescription = "Filter")
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            FilterMode.values().forEach { mode ->
                DropdownMenuItem(
                    text = { Text(mode.label()) },
                    onClick = {
                        expanded = false
                        onChange(mode)
                    },
                )
            }
        }
    }
}

@Composable
private fun KanbanColumn(
    column: KanbanColumnSpec,
    state: KanbanColumnState,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    filter: FilterMode,
    search: String,
    onOpen: (ThreadSummary) -> Unit,
    onLongPress: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onArchive: (ThreadSummary) -> Unit,
    onRefresh: () -> Unit,
    onLoadMore: () -> Unit,
    onMarkAllRead: () -> Unit,
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
    showSenderImages: Boolean,
    kanbanColumnWidth: Dp,
) {
    val visibleThreads = state.threads.filteredKanbanThreads(filter, search)
    val canLoadMore = search.isBlank() && (state.nextCursor.isNotBlank() || state.accountCursors.isNotEmpty())
    Card(
        modifier = Modifier.width(kanbanColumnWidth).fillMaxSize(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(Modifier.fillMaxSize()) {
            KanbanColumnHeader(
                column = column,
                accounts = accounts,
                foldersByAccount = foldersByAccount,
                count = visibleThreads.size,
                unread = visibleThreads.count { it.unread },
                onRefresh = onRefresh,
                onMarkAllRead = onMarkAllRead,
                onRemove = onRemove,
                onMoveLeft = onMoveLeft,
                onMoveRight = onMoveRight,
            )
            if (state.loading) {
                Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(Modifier.size(24.dp))
                }
            }
            if (state.error != null) {
                Text(
                    state.error,
                    color = MaterialTheme.colorScheme.error,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (!state.loading && visibleThreads.isEmpty()) {
                Box(Modifier.fillMaxSize().padding(18.dp), contentAlignment = Alignment.Center) {
                    Text(
                        if (search.isBlank()) "No ${filter.emptyNoun()} here" else "No matches",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontSize = 13.sp,
                    )
                }
            } else {
                LazyColumn(
                    Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(visibleThreads, key = { it.id }) { thread ->
                        KanbanThreadCard(
                            thread = thread,
                            showSenderImages = showSenderImages,
                            onOpen = { onOpen(thread) },
                            onLongPress = { onLongPress(thread) },
                            onToggleStar = { onToggleStar(thread) },
                            onArchive = { onArchive(thread) },
                        )
                    }
                    if (canLoadMore || state.loadingMore) {
                        item {
                            Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.Center) {
                                if (state.loadingMore) {
                                    CircularProgressIndicator(Modifier.size(22.dp))
                                } else {
                                    OutlinedButton(onClick = onLoadMore) {
                                        Text("Load older")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun KanbanColumnHeader(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    count: Int,
    unread: Int,
    onRefresh: () -> Unit,
    onMarkAllRead: () -> Unit,
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 10.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(folderIcon(column.folderId), contentDescription = null, modifier = Modifier.size(19.dp), tint = MaterialTheme.colorScheme.primary)
        Column(Modifier.weight(1f)) {
            Text(columnTitle(column, accounts, foldersByAccount), fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("$count cards${if (unread > 0) " · $unread unread" else ""}", fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Box {
            IconButton(onClick = { menuOpen = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = "Column actions")
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                DropdownMenuItem(text = { Text("Refresh") }, onClick = { menuOpen = false; onRefresh() })
                DropdownMenuItem(text = { Text("Mark all read") }, onClick = { menuOpen = false; onMarkAllRead() }, enabled = unread > 0)
                DropdownMenuItem(text = { Text("Move left") }, onClick = { menuOpen = false; onMoveLeft() })
                DropdownMenuItem(text = { Text("Move right") }, onClick = { menuOpen = false; onMoveRight() })
                DropdownMenuItem(text = { Text("Remove column") }, onClick = { menuOpen = false; onRemove() })
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
private fun KanbanThreadCard(
    thread: ThreadSummary,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
    onLongPress: () -> Unit,
    onToggleStar: () -> Unit,
    onArchive: () -> Unit,
) {
    val chat = LocalChatColors.current
    val dismissState = rememberSwipeToDismissBoxState(
        confirmValueChange = { value ->
            if (value == SwipeToDismissBoxValue.EndToStart) {
                onArchive()
                true
            } else {
                false
            }
        },
    )
    SwipeToDismissBox(
        state = dismissState,
        enableDismissFromStartToEnd = false,
        backgroundContent = {
            Box(
                Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)).background(MaterialTheme.colorScheme.primaryContainer).padding(horizontal = 16.dp),
                contentAlignment = Alignment.CenterEnd,
            ) {
                Icon(if (threadIdIsRss(thread.id)) Icons.Filled.Delete else Icons.Filled.Archive, contentDescription = null)
            }
        },
    ) {
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .combinedClickable(onClick = onOpen, onLongClick = onLongPress),
            shape = RoundedCornerShape(8.dp),
            colors = CardDefaults.cardColors(
                containerColor = if (thread.unread) MaterialTheme.colorScheme.primary.copy(alpha = 0.08f) else MaterialTheme.colorScheme.surface,
            ),
        ) {
            Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    SenderAvatar(
                        label = thread.sender.ifBlank { thread.accountId },
                        enabled = showSenderImages,
                        size = 26.dp,
                    )
                    Text(
                        thread.sender.ifBlank { thread.accountId }.substringBefore('@'),
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 12.sp,
                        modifier = Modifier.weight(1f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(formatRelativeTime(thread.dateEpochSeconds), fontSize = 10.5.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Text(
                    thread.subject.ifBlank { "(no subject)" },
                    fontSize = 12.sp,
                    fontWeight = if (thread.unread) FontWeight.SemiBold else FontWeight.Normal,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                if (thread.preview.isNotBlank()) {
                    Text(thread.preview, fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (thread.unread) {
                        Box(Modifier.size(7.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
                    }
                    Spacer(Modifier.weight(1f))
                    IconButton(onClick = onToggleStar, modifier = Modifier.size(28.dp)) {
                        Icon(
                            if (thread.starred) Icons.Filled.Star else Icons.Filled.StarBorder,
                            contentDescription = if (thread.starred) "Unstar" else "Star",
                            tint = if (thread.starred) chat.star else MaterialTheme.colorScheme.outline,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AccountSettingsDialog(
    account: AccountSummary,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    showInNavigation: Boolean,
    onSave: (
        displayName: String,
        senderName: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        loadRemoteImages: Boolean,
        conversationHtml: Boolean,
        includedInUnified: Boolean,
        showInNavigation: Boolean,
        muted: Boolean,
        paused: Boolean,
        rssSyncIntervalMinutes: Int,
        aliasesText: String,
    ) -> Unit,
    onPickAvatar: () -> Unit,
    onPickWallpaper: () -> Unit,
    onMoveUp: () -> Unit,
    onMoveDown: () -> Unit,
    onRemove: () -> Unit,
    onDismiss: () -> Unit,
) {
    val isRss = accountSummaryIsRss(account)
    var displayName by remember(account.id) { mutableStateOf(account.displayName) }
    var senderName by remember(account.id) { mutableStateOf(account.senderName) }
    var avatarUrl by remember(account.id) { mutableStateOf(account.avatarUrl) }
    var wallpaperPresetId by remember(account.id) { mutableStateOf(account.chatWallpaperPresetId) }
    var loadRemoteImages by remember(account.id) { mutableStateOf(account.loadRemoteImages || isRss) }
    var conversationHtml by remember(account.id) { mutableStateOf(account.conversationHtml) }
    var includedInUnified by remember(account.id) { mutableStateOf(account.includedInUnified) }
    var visibleInNavigation by remember(account.id, showInNavigation) { mutableStateOf(showInNavigation) }
    var muted by remember(account.id) { mutableStateOf(account.muted) }
    var paused by remember(account.id) { mutableStateOf(account.paused) }
    var intervalText by remember(account.id) { mutableStateOf(account.rssSyncIntervalMinutes.toString()) }
    var aliasesText by remember(account.id) {
        mutableStateOf(account.aliases.joinToString("\n") { alias ->
            if (alias.name.isBlank()) alias.email else "${alias.email}, ${alias.name}"
        })
    }
    val interval = intervalText.toIntOrNull()?.coerceIn(5, 1440) ?: account.rssSyncIntervalMinutes.coerceIn(5, 1440)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Account settings") },
        text = {
            LazyColumn(
                modifier = Modifier.heightIn(max = 520.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(account.email.ifBlank { account.id }, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        OutlinedTextField(
                            value = displayName,
                            onValueChange = { displayName = it },
                            label = { Text(if (isRss) "Feed group name" else "Account name") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        if (!isRss) {
                            OutlinedTextField(
                                value = senderName,
                                onValueChange = { senderName = it },
                                label = { Text("Sender name") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                        }
                        OutlinedTextField(
                            value = avatarUrl,
                            onValueChange = { avatarUrl = it },
                            label = { Text("Avatar URL") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        TextButton(onClick = onPickAvatar) {
                            Text("Choose avatar image")
                        }
                        OutlinedTextField(
                            value = wallpaperPresetId,
                            onValueChange = { wallpaperPresetId = it },
                            label = { Text("Chat wallpaper preset") },
                            supportingText = { Text("Blank for default; try grid, dots, forest, ocean, sunset") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        TextButton(onClick = onPickWallpaper) {
                            Text("Choose wallpaper image")
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            TextButton(onClick = onMoveUp, enabled = canMoveUp) {
                                Text("Move up")
                            }
                            TextButton(onClick = onMoveDown, enabled = canMoveDown) {
                                Text("Move down")
                            }
                        }
                    }
                }
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        AccountSwitchRow("Show in unified inbox", includedInUnified) { includedInUnified = it }
                        AccountSwitchRow("Show in navigation", visibleInNavigation) { visibleInNavigation = it }
                        AccountSwitchRow("Mute notifications", muted) { muted = it }
                        AccountSwitchRow("Pause automatic sync", paused) { paused = it }
                        AccountSwitchRow("Load remote images", loadRemoteImages) { loadRemoteImages = it }
                        AccountSwitchRow("Render HTML messages", conversationHtml) { conversationHtml = it }
                    }
                }
                if (isRss) {
                    item {
                        OutlinedTextField(
                            value = intervalText,
                            onValueChange = { intervalText = it.filter(Char::isDigit).take(4) },
                            label = { Text("RSS sync interval minutes") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                    }
                } else {
                    item {
                        OutlinedTextField(
                            value = aliasesText,
                            onValueChange = { aliasesText = it },
                            label = { Text("Send-as aliases") },
                            supportingText = { Text("One per line: email, optional name") },
                            modifier = Modifier.fillMaxWidth(),
                            minLines = 2,
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    onSave(
                        displayName,
                        senderName,
                        avatarUrl,
                        wallpaperPresetId,
                        loadRemoteImages,
                        conversationHtml,
                        includedInUnified,
                        visibleInNavigation,
                        muted,
                        paused,
                        interval,
                        aliasesText,
                    )
                },
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = onRemove) {
                    Text("Remove", color = MaterialTheme.colorScheme.error)
                }
                TextButton(onClick = onDismiss) {
                    Text("Cancel")
                }
            }
        },
    )
}

@Composable
private fun AccountSwitchRow(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth().heightIn(min = 44.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(label, modifier = Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
private fun KanbanBoardDialog(
    boards: List<KanbanBoardSpec>,
    activeBoardId: String,
    name: String,
    avatarUrl: String,
    wallpaperPresetId: String,
    wallpaperUrl: String,
    onNameChange: (String) -> Unit,
    onAvatarUrlChange: (String) -> Unit,
    onWallpaperPresetChange: (String) -> Unit,
    onWallpaperUrlChange: (String) -> Unit,
    onSelect: (String) -> Unit,
    onRename: () -> Unit,
    onSaveAppearance: () -> Unit,
    onCreate: () -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Kanban boards") },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                item {
                    OutlinedTextField(
                        value = name,
                        onValueChange = onNameChange,
                        label = { Text("Board name") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                item {
                    OutlinedTextField(
                        value = avatarUrl,
                        onValueChange = onAvatarUrlChange,
                        label = { Text("Board image URL") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                item {
                    OutlinedTextField(
                        value = wallpaperPresetId,
                        onValueChange = onWallpaperPresetChange,
                        label = { Text("Wallpaper preset") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                item {
                    OutlinedTextField(
                        value = wallpaperUrl,
                        onValueChange = onWallpaperUrlChange,
                        label = { Text("Wallpaper image URL") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                items(boards, key = { it.id }) { board ->
                    SidebarLikeDialogRow(
                        selected = board.id == activeBoardId,
                        title = board.name,
                        subtitle = "${board.columns.size} columns${if (board.avatarUrl.isNotBlank() || board.wallpaperPresetId.isNotBlank() || board.wallpaperUrl.isNotBlank()) " · customized" else ""}",
                        onClick = { onSelect(board.id) },
                    )
                }
            }
        },
        confirmButton = {
            Row {
                TextButton(onClick = onRename) { Text("Rename") }
                TextButton(onClick = onSaveAppearance) { Text("Save style") }
            }
        },
        dismissButton = {
            Row {
                TextButton(onClick = onCreate) { Text("New") }
                TextButton(onClick = onDelete) { Text("Delete") }
                TextButton(onClick = onDismiss) { Text("Done") }
            }
        },
    )
}

@Composable
private fun KanbanColumnDialog(
    accounts: List<AccountSummary>,
    board: KanbanBoardSpec?,
    foldersByAccount: Map<String, List<FolderSummary>>,
    onAddColumn: (KanbanColumnSpec) -> Unit,
    onCreateFolder: (AccountSummary) -> Unit,
    onDismiss: () -> Unit,
) {
    val selected = board?.columns.orEmpty().map(::kanbanColumnKey).toSet()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Add column") },
        text = {
            LazyColumn(Modifier.heightIn(max = 460.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                item {
                    val unified = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                    SidebarLikeDialogRow(
                        selected = selected.contains(kanbanColumnKey(unified)),
                        title = "Unified inbox",
                        subtitle = "All accounts",
                        onClick = { onAddColumn(unified) },
                    )
                }
                accounts.forEach { account ->
                    item {
                        Text(
                            account.displayName.ifBlank { account.email.ifBlank { account.id } },
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                    val folders = foldersByAccount[account.id].orEmpty()
                    if (!accountSummaryIsRss(account)) {
                        item {
                            TextButton(onClick = { onCreateFolder(account) }) {
                                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(16.dp))
                                Spacer(Modifier.width(6.dp))
                                Text("Create folder")
                            }
                        }
                    }
                    val visibleFolders = if (folders.isEmpty()) {
                        listOf(FolderSummary(account.id, INBOX_FOLDER, 0))
                    } else {
                        folders
                    }
                    items(visibleFolders, key = { "${account.id}\n${it.name}" }) { folder ->
                        val column = KanbanColumnSpec(account.id, folder.name)
                        SidebarLikeDialogRow(
                            selected = selected.contains(kanbanColumnKey(column)),
                            title = folder.name.replaceFirstChar { it.uppercase() },
                            subtitle = if (accountSummaryIsRss(account)) "Feed" else account.email,
                            onClick = { onAddColumn(column) },
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Done") }
        },
    )
}

@Composable
private fun KanbanThreadActionDialog(
    thread: ThreadSummary,
    board: KanbanBoardSpec?,
    accounts: List<AccountSummary>,
    onDismiss: () -> Unit,
    onOpen: () -> Unit,
    onToggleStar: () -> Unit,
    onToggleRead: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    onCopyFeedUrl: () -> Unit,
    onMove: (KanbanColumnSpec) -> Unit,
) {
    val deleteLabel = threadDeleteActionLabel(thread.folder)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(thread.subject.ifBlank { "(no subject)" }, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        text = {
            LazyColumn(Modifier.heightIn(max = 430.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item { DialogAction("Open", onOpen) }
                item { DialogAction(if (thread.starred) "Unstar" else "Star", onToggleStar) }
                item { DialogAction(if (thread.unread) "Mark read" else "Mark unread", onToggleRead) }
                if (threadIdIsRss(thread.id) && thread.feedUrl.isNotBlank()) {
                    item { DialogAction("Copy feed URL", onCopyFeedUrl) }
                }
                item { DialogAction(if (threadIdIsRss(thread.id)) "Remove feed" else "Archive", onArchive) }
                if (!threadIdIsRss(thread.id)) {
                    item { DialogAction(deleteLabel, onDelete) }
                }
                val moveTargets = board?.columns.orEmpty()
                    .filter { it.accountId != UNIFIED_ACCOUNT_ID }
                    .filter { target ->
                        val targetAccount = accounts.firstOrNull { it.id == target.accountId }
                        targetAccount != null && (threadIdIsRss(thread.id) == accountSummaryIsRss(targetAccount))
                    }
                    .filterNot { it.accountId == thread.accountId && it.folderId.equals(thread.folder, ignoreCase = true) }
                if (moveTargets.isNotEmpty()) {
                    item {
                        Text("Move to", fontSize = 12.sp, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 10.dp))
                    }
                    items(moveTargets, key = { kanbanColumnKey(it) }) { target ->
                        val account = accounts.firstOrNull { it.id == target.accountId }
                        DialogAction("${account?.displayName?.ifBlank { account.email } ?: target.accountId} / ${target.folderId}") {
                            onMove(target)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Close") }
        },
    )
}

@Composable
private fun DialogAction(label: String, onClick: () -> Unit) {
    TextButton(onClick = onClick, modifier = Modifier.fillMaxWidth()) {
        Text(label, modifier = Modifier.weight(1f))
    }
}

@Composable
private fun SidebarLikeDialogRow(selected: Boolean, title: String, subtitle: String?, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp))
            .background(if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.12f) else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle != null) {
                Text(subtitle, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun StarredItemList(
    items: List<StarredItemSummary>,
    showSenderImages: Boolean,
    onOpen: (StarredItemSummary) -> Unit,
    onToggleRead: (StarredItemSummary) -> Unit,
    onUnstar: (StarredItemSummary) -> Unit,
    onDelete: (StarredItemSummary) -> Unit,
) {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        items(items, key = { it.id }) { item ->
            StarredItemRow(
                item = item,
                showSenderImages = showSenderImages,
                onOpen = { onOpen(item) },
                onToggleRead = { onToggleRead(item) },
                onUnstar = { onUnstar(item) },
                onDelete = { onDelete(item) },
            )
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f))
        }
    }
}

@Composable
private fun StarredItemRow(
    item: StarredItemSummary,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
    onToggleRead: () -> Unit,
    onUnstar: () -> Unit,
    onDelete: () -> Unit,
) {
    val chat = LocalChatColors.current
    var menuOpen by remember { mutableStateOf(false) }
    val isRssItem = threadIdIsRss(item.threadId)
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(horizontal = 16.dp, vertical = 13.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SenderAvatar(
            label = item.sender.ifBlank { item.accountId },
            enabled = showSenderImages,
            size = 36.dp,
        )
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Filled.Star, contentDescription = null, tint = chat.star, modifier = Modifier.size(14.dp))
                Text(
                    item.sender.ifBlank { item.accountId }.substringBefore('@'),
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(formatRelativeTime(item.dateEpochSeconds), fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Text(
                item.subject.ifBlank { "(no subject)" },
                fontWeight = if (item.unread) FontWeight.SemiBold else FontWeight.Normal,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                if (item.unread) {
                    Box(Modifier.size(7.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
                }
                Text(
                    listOf(item.accountId, item.folder).filter { it.isNotBlank() }.joinToString(" / "),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Box {
            IconButton(onClick = { menuOpen = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = "Starred item actions")
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                DropdownMenuItem(
                    text = { Text(if (item.unread) "Mark read" else "Mark unread") },
                    onClick = {
                        menuOpen = false
                        onToggleRead()
                    },
                )
                DropdownMenuItem(
                    text = { Text("Unstar") },
                    onClick = {
                        menuOpen = false
                        onUnstar()
                    },
                )
                if (!isRssItem) {
                    DropdownMenuItem(
                        text = { Text("Delete message", color = MaterialTheme.colorScheme.error) },
                        onClick = {
                            menuOpen = false
                            onDelete()
                        },
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MailList(
    threads: List<ThreadSummary>,
    canLoadMore: Boolean,
    loadingMore: Boolean,
    onOpen: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onArchive: (ThreadSummary) -> Unit,
    onCopyFeedUrl: (ThreadSummary) -> Unit,
    onLoadMore: () -> Unit,
    showSenderImages: Boolean,
) {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 88.dp)) {
        items(threads, key = { it.id }) { thread ->
            val dismissState = rememberSwipeToDismissBoxState(
                confirmValueChange = { value ->
                    if (value == SwipeToDismissBoxValue.EndToStart) {
                        onArchive(thread)
                        true
                    } else {
                        false
                    }
                },
            )
            SwipeToDismissBox(
                state = dismissState,
                enableDismissFromStartToEnd = false,
                backgroundContent = {
                    val isRss = threadIdIsRss(thread.id)
                    Box(
                        Modifier
                            .fillMaxSize()
                            .background(MaterialTheme.colorScheme.primaryContainer)
                            .padding(horizontal = 24.dp),
                        contentAlignment = Alignment.CenterEnd,
                    ) {
                        Icon(
                            if (isRss) Icons.Filled.Delete else Icons.Filled.Archive,
                            contentDescription = if (isRss) "Remove" else "Archive",
                            tint = MaterialTheme.colorScheme.onPrimaryContainer,
                        )
                    }
                },
            ) {
                MailRow(
                    thread = thread,
                    showSenderImages = showSenderImages,
                    onOpen = { onOpen(thread) },
                    onToggleStar = { onToggleStar(thread) },
                    onCopyFeedUrl = if (threadIdIsRss(thread.id) && thread.feedUrl.isNotBlank()) {
                        { onCopyFeedUrl(thread) }
                    } else {
                        null
                    },
                )
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f))
        }
        if (canLoadMore || loadingMore) {
            item {
                Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                    if (loadingMore) {
                        CircularProgressIndicator(Modifier.size(24.dp))
                    } else {
                        OutlinedButton(onClick = onLoadMore) {
                            Text("Load older")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun MailRow(
    thread: ThreadSummary,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
    onToggleStar: () -> Unit,
    onCopyFeedUrl: (() -> Unit)?,
) {
    val unread = thread.unread
    val chat = LocalChatColors.current
    val senderLabel = thread.sender.ifBlank { thread.accountId }
    val rowBackground = when {
        unread -> MaterialTheme.colorScheme.primary.copy(alpha = 0.07f)
        else -> MaterialTheme.colorScheme.surface
    }
    Row(
        Modifier
            .fillMaxWidth()
            .background(rowBackground)
            .clickable(onClick = onOpen)
            .padding(horizontal = 12.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        SenderAvatar(label = senderLabel, enabled = showSenderImages, size = 42.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    senderLabel.substringBefore('@'),
                    fontSize = 13.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    formatRelativeTime(thread.dateEpochSeconds),
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = if (unread) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                if (thread.starred) {
                    Icon(Icons.Filled.Star, contentDescription = null, tint = chat.star, modifier = Modifier.size(11.dp))
                }
                Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(color = MaterialTheme.colorScheme.onSurface, fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Normal)) {
                            append(thread.subject.ifBlank { "(no subject)" })
                        }
                        if (thread.preview.isNotBlank()) {
                            withStyle(SpanStyle(color = MaterialTheme.colorScheme.onSurfaceVariant)) {
                                append(" — ${thread.preview}")
                            }
                        }
                    },
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                if (unread) {
                    Box(
                        Modifier.size(8.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary),
                    )
                }
            }
        }
        IconButton(onClick = onToggleStar, modifier = Modifier.size(24.dp)) {
            Icon(
                if (thread.starred) Icons.Filled.Star else Icons.Filled.StarBorder,
                contentDescription = if (thread.starred) "Unstar" else "Star",
                tint = if (thread.starred) chat.star else MaterialTheme.colorScheme.outline,
                modifier = Modifier.size(18.dp),
            )
        }
        if (onCopyFeedUrl != null) {
            IconButton(onClick = onCopyFeedUrl, modifier = Modifier.size(24.dp)) {
                Icon(
                    Icons.Filled.MoreVert,
                    contentDescription = "Copy feed URL",
                    tint = MaterialTheme.colorScheme.outline,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
    }
}

@Composable
private fun SenderAvatar(label: String, enabled: Boolean, size: Dp) {
    val urls = remember(label, enabled) {
        if (enabled) senderImageUrls(label) else emptyList()
    }
    var bitmap by remember(label, enabled) { mutableStateOf<Bitmap?>(null) }

    LaunchedEffect(urls) {
        bitmap = null
        if (urls.isNotEmpty()) {
            bitmap = withContext(Dispatchers.IO) { loadFirstBitmap(urls) }
        }
    }

    if (enabled && bitmap != null) {
        Image(
            bitmap = bitmap!!.asImageBitmap(),
            contentDescription = null,
            modifier = Modifier.size(size).clip(CircleShape),
        )
    } else {
        Avatar(label, size)
    }
}

@Composable
private fun Avatar(name: String, size: Dp = 42.dp) {
    Box(
        Modifier
            .size(size)
            .clip(CircleShape)
            .background(avatarBrush(name)),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            avatarInitials(name),
            color = Color.White,
            fontWeight = FontWeight.SemiBold,
            fontSize = (size.value * 0.34f).sp,
        )
    }
}

private fun senderImageUrls(label: String): List<String> {
    val email = extractEmail(label) ?: return emptyList()
    val domain = email.substringAfter('@', "").takeIf { it.isNotBlank() } ?: return emptyList()
    val hash = md5Hex(email.lowercase(Locale.ROOT).trim())
    return listOf(
        "https://www.gravatar.com/avatar/$hash?s=96&d=404",
        "https://www.google.com/s2/favicons?domain=$domain&sz=96",
    )
}

private fun extractEmail(value: String): String? {
    val match = Regex("[A-Z0-9._%+-]+@[A-Z0-9.-]+\\.[A-Z]{2,}", RegexOption.IGNORE_CASE).find(value)
    return match?.value?.trim()
}

private fun md5Hex(value: String): String {
    return MessageDigest.getInstance("MD5")
        .digest(value.toByteArray())
        .joinToString("") { "%02x".format(it) }
}

private fun loadFirstBitmap(urls: List<String>): Bitmap? {
    for (url in urls) {
        val bitmap = runCatching {
            URL(url).openStream().use { stream ->
                BitmapFactory.decodeStream(stream)
            }
        }.getOrNull()
        if (bitmap != null) return bitmap
    }
    return null
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ThreadScreen(
    thread: ThreadSummary?,
    messages: List<MessageBody>,
    accountEmail: String,
    preferHtml: Boolean,
    onPreferHtmlChange: (Boolean) -> Unit,
    onBack: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    onToggleStar: () -> Unit,
    moveFolders: List<FolderSummary>,
    copyFolders: List<FolderSummary>,
    onMoveToFolder: (FolderSummary) -> Unit,
    onCreateFolderAndMove: (String) -> Unit,
    onCopyToFolder: (FolderSummary) -> Unit,
    quickReplyBody: String,
    canLoadOlder: Boolean,
    loadingOlder: Boolean,
    onLoadOlder: () -> Unit,
    onQuickReplyChange: (String) -> Unit,
    quickReplyAttachments: List<DraftAttachment>,
    quickReplyFailure: String,
    sendShortcutMode: SendShortcutMode,
    onQuickReplyAttach: () -> Unit,
    onRemoveQuickReplyAttachment: (DraftAttachment) -> Unit,
    onOpenFullReply: () -> Unit,
    onSendReply: () -> Unit,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onToggleMessageRead: (MessageBody) -> Unit,
    onToggleMessageStarred: (MessageBody) -> Unit,
    onDeleteMessage: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onComposeTo: (String) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
) {
    val isRss = thread?.let { threadIdIsRss(it.id) } ?: false
    val deleteLabel = thread?.let { threadDeleteActionLabel(it.folder) } ?: "Move to Trash"
    val chat = LocalChatColors.current
    val context = LocalContext.current
    var searchOpen by remember(thread?.id) { mutableStateOf(false) }
    var threadSearch by remember(thread?.id) { mutableStateOf("") }
    var activeSearchIndex by remember(thread?.id) { mutableStateOf(0) }
    var detailsOpen by remember(thread?.id) { mutableStateOf(false) }
    var moveDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    var copyDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    val normalizedSearch = threadSearch.trim().lowercase()
    val currentThreadAccountId = thread?.accountId.orEmpty()
    val currentThreadFolder = thread?.folder.orEmpty()
    val targetMoveFolders = remember(currentThreadFolder, moveFolders) {
        moveFolders.filterNot { folder -> folder.name.equals(currentThreadFolder, ignoreCase = true) }
    }
    val targetCopyFolders = remember(currentThreadAccountId, currentThreadFolder, copyFolders) {
        copyFolders.filterNot { folder ->
            folder.accountId == currentThreadAccountId && folder.name.equals(currentThreadFolder, ignoreCase = true)
        }
    }
    val searchMatches = remember(messages, normalizedSearch) {
        if (normalizedSearch.isBlank()) emptyList() else messages.filter { threadMessageSearchText(it).contains(normalizedSearch) }.map { it.id }
    }
    val activeSearchId = searchMatches.getOrNull(activeSearchIndex).orEmpty()
    val listState = rememberLazyListState()
    LaunchedEffect(normalizedSearch) {
        activeSearchIndex = 0
    }
    LaunchedEffect(searchMatches.size, activeSearchIndex) {
        if (activeSearchIndex >= searchMatches.size) activeSearchIndex = 0
    }
    LaunchedEffect(activeSearchId, canLoadOlder, loadingOlder) {
        if (activeSearchId.isBlank()) return@LaunchedEffect
        val messageIndex = messages.indexOfFirst { it.id == activeSearchId }
        if (messageIndex >= 0) {
            val offset = if (canLoadOlder || loadingOlder) 1 else 0
            listState.animateScrollToItem(messageIndex + offset)
        }
    }

    fun goToSearchMatch(delta: Int) {
        if (searchMatches.isEmpty()) return
        val next = activeSearchIndex + delta
        activeSearchIndex = when {
            next < 0 -> searchMatches.lastIndex
            next > searchMatches.lastIndex -> 0
            else -> next
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        thread?.subject?.ifBlank { "(no subject)" } ?: "Conversation",
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { detailsOpen = true }) {
                        Icon(Icons.Filled.Info, contentDescription = "Conversation details")
                    }
                    IconButton(onClick = { searchOpen = !searchOpen }) {
                        Icon(Icons.Filled.Search, contentDescription = "Search conversation")
                    }
                    IconButton(onClick = onToggleStar) {
                        Icon(
                            if (thread?.starred == true) Icons.Filled.Star else Icons.Filled.StarBorder,
                            contentDescription = "Star",
                            tint = if (thread?.starred == true) chat.star else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    IconButton(onClick = onArchive) {
                        Icon(if (isRss) Icons.Filled.Delete else Icons.Filled.Archive, contentDescription = "Archive")
                    }
                    if (!isRss) {
                        IconButton(onClick = { moveDialogOpen = true }) {
                            Icon(Icons.Outlined.FolderOpen, contentDescription = "Move to folder")
                        }
                        IconButton(onClick = { copyDialogOpen = true }) {
                            Icon(Icons.Filled.ContentCopy, contentDescription = "Copy to folder")
                        }
                        IconButton(onClick = onDelete) {
                            Icon(Icons.Filled.Delete, contentDescription = deleteLabel)
                        }
                    }
                },
            )
        },
    ) { innerPadding ->
        if (detailsOpen) {
            ConversationDetailsDialog(
                messages = messages,
                isRss = isRss,
                ownEmail = accountEmail,
                onDismiss = { detailsOpen = false },
                onComposeTo = { email ->
                    detailsOpen = false
                    onComposeTo(email)
                },
                onCopy = { label, value ->
                    copyToClipboard(context, label, value)
                },
                onOpenAttachment = onOpenAttachment,
                onSaveAttachment = onSaveAttachment,
            )
        }
        if (moveDialogOpen && thread != null) {
            MoveThreadDialog(
                thread = thread,
                folders = targetMoveFolders,
                onMove = { folder ->
                    moveDialogOpen = false
                    onMoveToFolder(folder)
                },
                onCreateAndMove = { name ->
                    moveDialogOpen = false
                    onCreateFolderAndMove(name)
                },
                onDismiss = { moveDialogOpen = false },
            )
        }
        if (copyDialogOpen && thread != null) {
            CopyThreadDialog(
                thread = thread,
                folders = targetCopyFolders,
                onCopy = { folder ->
                    copyDialogOpen = false
                    onCopyToFolder(folder)
                },
                onDismiss = { copyDialogOpen = false },
            )
        }
        Column(
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .padding(innerPadding),
        ) {
            if (searchOpen) {
                ConversationSearchBar(
                    query = threadSearch,
                    onQueryChange = { threadSearch = it },
                    matchLabel = if (normalizedSearch.isBlank()) "" else "${if (searchMatches.isEmpty()) 0 else activeSearchIndex + 1}/${searchMatches.size}",
                    canNavigate = searchMatches.isNotEmpty(),
                    onPrevious = { goToSearchMatch(-1) },
                    onNext = { goToSearchMatch(1) },
                    onClose = {
                        threadSearch = ""
                        searchOpen = false
                    },
                )
            }
            if (!isRss) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("View", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    FilterChip(
                        selected = preferHtml,
                        onClick = { onPreferHtmlChange(true) },
                        label = { Text("HTML") },
                    )
                    FilterChip(
                        selected = !preferHtml,
                        onClick = { onPreferHtmlChange(false) },
                        label = { Text("Plain text") },
                    )
                }
            }
            if (messages.isEmpty()) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else {
                LazyColumn(
                    Modifier.weight(1f),
                    state = listState,
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    if (canLoadOlder || loadingOlder) {
                        item {
                            Box(Modifier.fillMaxWidth().padding(bottom = 4.dp), contentAlignment = Alignment.Center) {
                                if (loadingOlder) {
                                    CircularProgressIndicator(Modifier.size(24.dp))
                                } else {
                                    OutlinedButton(onClick = onLoadOlder) {
                                        Text("Load older")
                                    }
                                }
                            }
                        }
                    }
                    items(messages, key = { it.id }) { message ->
                        val outgoing = isOutgoing(message, accountEmail)
                        MessageBubble(
                            message = message,
                            outgoing = outgoing,
                            chat = chat,
                            preferHtml = preferHtml,
                            searchQuery = normalizedSearch,
                            activeSearchMatch = message.id == activeSearchId,
                            actionsEnabled = !isRss,
                            onForward = onForward,
                            onEditAsNew = onEditAsNew,
                            onToggleRead = onToggleMessageRead,
                            onToggleStarred = onToggleMessageStarred,
                            onDelete = onDeleteMessage,
                            onOpenAttachment = onOpenAttachment,
                            onSaveAttachment = onSaveAttachment,
                            onCopyMessageText = onCopyMessageText,
                        )
                    }
                }
            }
            if (thread != null && !isRss && messages.isNotEmpty()) {
                ReplyBar(
                    value = quickReplyBody,
                    onChange = onQuickReplyChange,
                    attachments = quickReplyAttachments,
                    failureMessage = quickReplyFailure,
                    sendShortcutMode = sendShortcutMode,
                    onAttach = onQuickReplyAttach,
                    onRemoveAttachment = onRemoveQuickReplyAttachment,
                    onOpenFullEditor = onOpenFullReply,
                    onSend = onSendReply,
                )
            }
        }
    }
}

@Composable
private fun ConversationDetailsDialog(
    messages: List<MessageBody>,
    isRss: Boolean,
    ownEmail: String,
    onDismiss: () -> Unit,
    onComposeTo: (String) -> Unit,
    onCopy: (String, String) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
) {
    val participants = remember(messages, isRss, ownEmail) { conversationParticipants(messages, ownEmail, isRss) }
    val attachments = remember(messages) { messages.flatMap { it.attachments }.asReversed() }
    val mediaCount = attachments.count { it.mimeType.startsWith("image/") || it.mimeType.startsWith("video/") }
    val fileCount = attachments.size - mediaCount
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Conversation details") },
        text = {
            LazyColumn(
                modifier = Modifier.heightIn(max = 460.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                item {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        AssistiveStat("People", participants.size.toString())
                        AssistiveStat("Media", mediaCount.toString())
                        AssistiveStat("Files", fileCount.toString())
                    }
                }
                item {
                    Text("People", fontWeight = FontWeight.SemiBold)
                }
                if (participants.isEmpty()) {
                    item {
                        Text("No conversation participants.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    items(participants, key = { it.email }) { person ->
                        Row(
                            Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Avatar(person.name.ifBlank { person.email }, 34.dp)
                            Column(Modifier.weight(1f)) {
                                Text(
                                    person.name.ifBlank { person.email } + if (person.isSelf) " (you)" else "",
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    "${person.email} · ${person.count}",
                                    fontSize = 12.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            TextButton(onClick = { onCopy("Email address", person.email) }) { Text("Copy") }
                            if (!person.isSelf) {
                                TextButton(onClick = { onComposeTo(person.email) }) { Text("Mail") }
                            }
                        }
                    }
                }
                item {
                    HorizontalDivider()
                    Text("Shared files", fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp))
                }
                if (attachments.isEmpty()) {
                    item {
                        Text("No shared files in loaded messages.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    items(attachments.withIndex().toList(), key = { it.index }) { indexed ->
                        val attachment = indexed.value
                        Row(
                            Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(Icons.Filled.AttachFile, contentDescription = null)
                            Column(Modifier.weight(1f)) {
                                Text(attachment.filename.ifBlank { "Attachment" }, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(
                                    listOf(attachment.mimeType, formatBytes(attachment.sizeBytes)).filter { it.isNotBlank() }.joinToString(" · "),
                                    fontSize = 12.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            TextButton(onClick = { onOpenAttachment(attachment) }) { Text("Open") }
                            TextButton(onClick = { onSaveAttachment(attachment) }) { Text("Save") }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Done") }
        },
    )
}

@Composable
private fun MoveThreadDialog(
    thread: ThreadSummary,
    folders: List<FolderSummary>,
    onMove: (FolderSummary) -> Unit,
    onCreateAndMove: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var newFolderName by remember(thread.id) { mutableStateOf("") }
    val trimmedNewFolderName = newFolderName.trim()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Move conversation") },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item {
                    Text(
                        thread.subject.ifBlank { "(no subject)" },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                item {
                    OutlinedTextField(
                        value = newFolderName,
                        onValueChange = { newFolderName = it },
                        label = { Text("New folder") },
                        singleLine = true,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 8.dp),
                    )
                }
                item {
                    Button(
                        onClick = { onCreateAndMove(trimmedNewFolderName) },
                        enabled = trimmedNewFolderName.isNotBlank(),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("Create & Move")
                    }
                }
                if (folders.isEmpty()) {
                    item {
                        Text(
                            "No other folders available.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                } else {
                    items(folders, key = { "${it.accountId}\n${it.name}" }) { folder ->
                        DialogAction(folder.name.replaceFirstChar { it.uppercase() }) {
                            onMove(folder)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}

@Composable
private fun CopyThreadDialog(
    thread: ThreadSummary,
    folders: List<FolderSummary>,
    onCopy: (FolderSummary) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Copy conversation") },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item {
                    Text(
                        thread.subject.ifBlank { "(no subject)" },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (folders.isEmpty()) {
                    item {
                        Text(
                            "No copy targets available.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                } else {
                    items(folders, key = { "${it.accountId}\n${it.name}" }) { folder ->
                        val label = if (folder.accountId.isBlank() || folder.accountId == thread.accountId) {
                            folder.name.replaceFirstChar { it.uppercase() }
                        } else {
                            "${folder.accountId} / ${folder.name.replaceFirstChar { it.uppercase() }}"
                        }
                        DialogAction(label) {
                            onCopy(folder)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}

@Composable
private fun AssistiveStat(label: String, value: String) {
    Surface(
        shape = RoundedCornerShape(8.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
    ) {
        Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(value, fontWeight = FontWeight.Bold)
            Text(label, fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun ImagePreviewDialog(
    preview: AndroidImagePreview,
    onShare: () -> Unit,
    onCopy: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(preview.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        text = {
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 220.dp, max = 520.dp)
                    .background(Color.Black, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Image(
                    bitmap = preview.bitmap.asImageBitmap(),
                    contentDescription = preview.title,
                    modifier = Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)),
                    contentScale = ContentScale.Fit,
                )
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = onShare) { Text("Share") }
                TextButton(onClick = onCopy) { Text("Copy") }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } },
    )
}

@Composable
private fun ConversationSearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    matchLabel: String,
    canNavigate: Boolean,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    onClose: () -> Unit,
) {
    Surface(tonalElevation = 2.dp, color = MaterialTheme.colorScheme.surfaceContainer) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedTextField(
                value = query,
                onValueChange = onQueryChange,
                placeholder = { Text("Search conversation") },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                trailingIcon = {
                    if (query.isNotBlank()) {
                        IconButton(onClick = { onQueryChange("") }) {
                            Icon(Icons.Filled.Close, contentDescription = "Clear search")
                        }
                    }
                },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            Text(matchLabel, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(42.dp))
            TextButton(onClick = onPrevious, enabled = canNavigate) { Text("Prev") }
            TextButton(onClick = onNext, enabled = canNavigate) { Text("Next") }
            IconButton(onClick = onClose) {
                Icon(Icons.Filled.Close, contentDescription = "Close search")
            }
        }
    }
}

@Composable
private fun MessageBubble(
    message: MessageBody,
    outgoing: Boolean,
    chat: ChatColors,
    preferHtml: Boolean,
    searchQuery: String,
    activeSearchMatch: Boolean,
    actionsEnabled: Boolean,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onToggleRead: (MessageBody) -> Unit,
    onToggleStarred: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    val bubbleShape = if (outgoing) {
        RoundedCornerShape(topStart = 16.dp, topEnd = 4.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
    } else {
        RoundedCornerShape(topStart = 4.dp, topEnd = 16.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
    }
    val bubbleColor = if (outgoing) chat.bubbleOut else chat.bubbleIn
    val textColor = if (outgoing) chat.bubbleOutText else chat.bubbleInText
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Column(
            Modifier
                .widthIn(max = 320.dp)
                .clip(bubbleShape)
                .then(
                    if (activeSearchMatch) {
                        Modifier.border(2.dp, Color(0xFFFFC107), bubbleShape)
                    } else {
                        Modifier
                    },
                )
                .background(bubbleColor)
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (!outgoing) {
                Text(
                    message.from.ifBlank { message.fromAddr },
                    fontSize = 12.5.sp,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (preferHtml && message.bodyHtml.isNotBlank() && searchQuery.isBlank()) {
                HtmlMessageBody(html = message.bodyHtml)
            } else {
                // Subject is the conversation title (top bar); the bubble shows the
                // message body, matching the desktop chat reader.
                Text(
                    highlightedMessageText(message.body.ifBlank { "(no content)" }, searchQuery, activeSearchMatch),
                    color = if (message.body.isBlank()) textColor.copy(alpha = 0.6f) else textColor,
                )
            }
            if (message.attachments.isNotEmpty()) {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    message.attachments.forEach { attachment ->
                        AttachmentRow(
                            attachment = attachment,
                            textColor = textColor,
                            onOpen = { onOpenAttachment(attachment) },
                            onSave = { onSaveAttachment(attachment) },
                        )
                    }
                }
            }
            Text(
                formatRelativeTime(message.dateEpochSeconds),
                fontSize = 10.5.sp,
                color = textColor.copy(alpha = 0.6f),
                modifier = Modifier.align(Alignment.End),
            )
            Box(modifier = Modifier.align(Alignment.End)) {
                TextButton(onClick = { menuOpen = true }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = null, modifier = Modifier.size(16.dp))
                    Text("More")
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    DropdownMenuItem(
                        text = { Text("Copy message text") },
                        onClick = {
                            menuOpen = false
                            onCopyMessageText("Message text", messagePlainText(message))
                        },
                    )
                    DropdownMenuItem(
                        text = { Text("Copy subject") },
                        onClick = {
                            menuOpen = false
                            onCopyMessageText("Subject", message.subject.ifBlank { "(no subject)" })
                        },
                    )
                    if (message.messageId.isNotBlank()) {
                        DropdownMenuItem(
                            text = { Text("Copy message ID") },
                            onClick = {
                                menuOpen = false
                                onCopyMessageText("Message ID", message.messageId)
                            },
                        )
                    }
                    if (actionsEnabled) {
                        DropdownMenuItem(
                            text = { Text(if (message.unread) "Mark read" else "Mark unread") },
                            onClick = {
                                menuOpen = false
                                onToggleRead(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(if (message.starred) "Unstar" else "Star") },
                            onClick = {
                                menuOpen = false
                                onToggleStarred(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Forward") },
                            onClick = {
                                menuOpen = false
                                onForward(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Edit as new") },
                            onClick = {
                                menuOpen = false
                                onEditAsNew(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Delete message", color = MaterialTheme.colorScheme.error) },
                            onClick = {
                                menuOpen = false
                                onDelete(message)
                            },
                        )
                    }
                }
            }
        }
    }
}

private fun threadMessageSearchText(message: MessageBody): String {
    return listOf(
        message.subject,
        message.from,
        message.fromAddr,
        message.to,
        message.cc,
        message.body,
        message.bodyHtml.replace(Regex("<[^>]+>"), " "),
        message.attachments.joinToString(" ") { it.filename },
    ).joinToString(" ").lowercase()
}

private fun threadDeleteActionLabel(folder: String): String {
    return when {
        folderIsDrafts(folder) -> "Discard draft"
        folderIsTrash(folder) -> "Delete forever"
        else -> "Move to Trash"
    }
}

private fun messagePlainText(message: MessageBody): String {
    return message.body.ifBlank {
        message.bodyHtml
            .replace(Regex("<br\\s*/?>", RegexOption.IGNORE_CASE), "\n")
            .replace(Regex("</p>", RegexOption.IGNORE_CASE), "\n")
            .replace(Regex("<[^>]+>"), " ")
            .replace(Regex("[ \\t]+"), " ")
            .trim()
    }.ifBlank { "(no content)" }
}

private fun conversationParticipants(messages: List<MessageBody>, ownEmail: String, isRss: Boolean): List<ConversationParticipant> {
    if (isRss) return emptyList()
    data class MutableParticipant(var name: String, val email: String, var count: Int, val isSelf: Boolean)
    val own = ownEmail.trim().lowercase()
    val byEmail = linkedMapOf<String, MutableParticipant>()

    fun add(name: String, email: String) {
        val normalized = email.trim().trim('<', '>', ',', ';').lowercase()
        if (normalized.isBlank() || !normalized.contains("@")) return
        val existing = byEmail[normalized]
        if (existing != null) {
            existing.count += 1
            if ((existing.name.isBlank() || existing.name == existing.email) && name.isNotBlank() && name != email) {
                existing.name = name
            }
        } else {
            byEmail[normalized] = MutableParticipant(
                name = name.takeIf { it.isNotBlank() && it != email } ?: normalized,
                email = normalized,
                count = 1,
                isSelf = normalized == own,
            )
        }
    }

    messages.forEach { message ->
        add(message.from, message.fromAddr.ifBlank { message.from })
        parseAddressList(message.to).forEach { (name, email) -> add(name, email) }
        parseAddressList(message.cc).forEach { (name, email) -> add(name, email) }
    }
    return byEmail.values
        .map { ConversationParticipant(it.name, it.email, it.count, it.isSelf) }
        .sortedWith(compareBy<ConversationParticipant> { it.isSelf }.thenByDescending { it.count })
}

private fun parseAddressList(value: String): List<Pair<String, String>> {
    if (value.isBlank()) return emptyList()
    return value.split(',', ';').mapNotNull { raw ->
        val entry = raw.trim()
        if (entry.isBlank()) return@mapNotNull null
        val bracket = Regex("""^(.*)<([^>]+)>$""").matchEntire(entry)
        if (bracket != null) {
            val name = bracket.groupValues[1].trim().trim('"')
            val email = bracket.groupValues[2].trim()
            return@mapNotNull name to email
        }
        entry to entry
    }
}

private fun copyToClipboard(context: Context, label: String, value: String) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText(label, value))
}

private fun highlightedMessageText(text: String, query: String, active: Boolean): AnnotatedString {
    if (query.isBlank()) return AnnotatedString(text)
    val lower = text.lowercase()
    val needle = query.lowercase()
    return buildAnnotatedString {
        var start = 0
        while (start < text.length) {
            val index = lower.indexOf(needle, start)
            if (index < 0) {
                append(text.substring(start))
                break
            }
            append(text.substring(start, index))
            withStyle(
                SpanStyle(
                    background = if (active) Color(0xFFFFD54F) else Color(0xFFFFECB3),
                    color = Color.Black,
                    fontWeight = FontWeight.SemiBold,
                ),
            ) {
                append(text.substring(index, index + needle.length))
            }
            start = index + needle.length
        }
    }
}

@Composable
private fun HtmlMessageBody(html: String) {
    AndroidView(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 160.dp, max = 420.dp),
        factory = { context ->
            WebView(context).apply {
                webViewClient = WebViewClient()
                settings.javaScriptEnabled = false
                settings.domStorageEnabled = false
                settings.loadWithOverviewMode = true
                settings.useWideViewPort = true
                isVerticalScrollBarEnabled = true
                isHorizontalScrollBarEnabled = false
            }
        },
        update = { webView ->
            webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        },
    )
}

@Composable
private fun AttachmentRow(
    attachment: MessageAttachment,
    textColor: Color,
    onOpen: () -> Unit,
    onSave: () -> Unit,
) {
    Surface(
        onClick = onOpen,
        shape = RoundedCornerShape(8.dp),
        color = textColor.copy(alpha = 0.08f),
        contentColor = textColor,
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(18.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    attachment.filename.ifBlank { "Attachment" },
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    listOf(attachment.mimeType, formatBytes(attachment.sizeBytes)).filter { it.isNotBlank() }.joinToString(" · "),
                    fontSize = 10.5.sp,
                    color = textColor.copy(alpha = 0.7f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            TextButton(onClick = onSave) {
                Text("Save")
            }
        }
    }
}

@Composable
private fun ReplyBar(
    value: String,
    onChange: (String) -> Unit,
    attachments: List<DraftAttachment>,
    failureMessage: String,
    sendShortcutMode: SendShortcutMode,
    onAttach: () -> Unit,
    onRemoveAttachment: (DraftAttachment) -> Unit,
    onOpenFullEditor: () -> Unit,
    onSend: () -> Unit,
) {
    Surface(tonalElevation = 3.dp, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column(
            Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (attachments.isNotEmpty()) {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(attachments, key = { it.id }) { attachment ->
                        FilterChip(
                            selected = false,
                            onClick = { onRemoveAttachment(attachment) },
                            label = {
                                Text(
                                    attachment.displayName,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            },
                            leadingIcon = {
                                Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(16.dp))
                            },
                        )
                    }
                }
            }
            if (failureMessage.isNotBlank()) {
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(Icons.Filled.ErrorOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                    Text(
                        "Reply failed. Your draft is still here.",
                        modifier = Modifier.weight(1f),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                    TextButton(onClick = onSend, enabled = value.isNotBlank() || attachments.isNotEmpty()) {
                        Text("Retry")
                    }
                }
            }
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(Icons.AutoMirrored.Filled.Reply, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                OutlinedTextField(
                    value = value,
                    onValueChange = onChange,
                    placeholder = { Text("Reply") },
                    supportingText = { Text("${sendShortcutMode.label()} sends") },
                    modifier = Modifier
                        .weight(1f)
                        .onPreviewKeyEvent { event ->
                            if (shouldSendFromEditor(event, sendShortcutMode) && (value.isNotBlank() || attachments.isNotEmpty())) {
                                onSend()
                                true
                            } else {
                                false
                            }
                        },
                    maxLines = 4,
                )
                IconButton(onClick = onAttach) {
                    Icon(Icons.Filled.AttachFile, contentDescription = "Attach")
                }
                IconButton(onClick = onOpenFullEditor) {
                    Icon(Icons.Filled.OpenInFull, contentDescription = "Open full editor")
                }
                IconButton(onClick = onSend, enabled = value.isNotBlank() || attachments.isNotEmpty()) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send reply", tint = MaterialTheme.colorScheme.primary)
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ComposeScreen(
    sendIdentities: List<SendIdentity>,
    selectedFromKey: String,
    onFromChange: (String) -> Unit,
    to: String, onToChange: (String) -> Unit,
    cc: String, onCcChange: (String) -> Unit,
    bcc: String, onBccChange: (String) -> Unit,
    subject: String, onSubjectChange: (String) -> Unit,
    body: String, onBodyChange: (String) -> Unit,
    attachments: List<DraftAttachment>,
    recipientSuggestionField: String,
    recipientSuggestions: List<ContactSuggestion>,
    onRecipientFocus: (field: String, value: String) -> Unit,
    onAcceptRecipientSuggestion: (field: String, contact: ContactSuggestion) -> Unit,
    onAttach: () -> Unit,
    onClearAttachments: () -> Unit,
    sendShortcutMode: SendShortcutMode,
    onSaveDraft: () -> Unit,
    onDiscardDraft: () -> Unit,
    onSend: () -> Unit,
    onBack: () -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("New message") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Filled.Close, contentDescription = "Discard")
                    }
                },
                actions = {
                    IconButton(onClick = onAttach) {
                        Icon(Icons.Filled.AttachFile, contentDescription = "Attach")
                    }
                    IconButton(onClick = onSaveDraft) {
                        Icon(Icons.Outlined.Drafts, contentDescription = "Save draft")
                    }
                    IconButton(onClick = onDiscardDraft) {
                        Icon(Icons.Filled.Delete, contentDescription = "Discard draft", tint = MaterialTheme.colorScheme.error)
                    }
                    IconButton(onClick = onSend) {
                        Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send", tint = MaterialTheme.colorScheme.primary)
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            Modifier.fillMaxSize().padding(innerPadding).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (sendIdentities.size > 1) {
                FromIdentitySelector(
                    identities = sendIdentities,
                    selectedKey = selectedFromKey,
                    onSelect = onFromChange,
                )
            }
            ComposeField(
                value = to,
                onChange = onToChange,
                label = "To",
                field = "to",
                suggestions = if (recipientSuggestionField == "to") recipientSuggestions else emptyList(),
                onFocus = onRecipientFocus,
                onAcceptSuggestion = onAcceptRecipientSuggestion,
            )
            ComposeField(
                value = cc,
                onChange = onCcChange,
                label = "Cc",
                field = "cc",
                suggestions = if (recipientSuggestionField == "cc") recipientSuggestions else emptyList(),
                onFocus = onRecipientFocus,
                onAcceptSuggestion = onAcceptRecipientSuggestion,
            )
            ComposeField(
                value = bcc,
                onChange = onBccChange,
                label = "Bcc",
                field = "bcc",
                suggestions = if (recipientSuggestionField == "bcc") recipientSuggestions else emptyList(),
                onFocus = onRecipientFocus,
                onAcceptSuggestion = onAcceptRecipientSuggestion,
            )
            ComposeField(subject, onSubjectChange, "Subject")
            OutlinedTextField(
                value = body,
                onValueChange = onBodyChange,
                label = { Text("Message") },
                supportingText = { Text("${sendShortcutMode.label()} sends") },
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .onPreviewKeyEvent { event ->
                        if (shouldSendFromEditor(event, sendShortcutMode)) {
                            onSend()
                            true
                        } else {
                            false
                        }
                    },
            )
            attachments.forEach { attachment ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(18.dp))
                    Text(
                        "${attachment.displayName} · ${formatBytes(attachment.sizeBytes)}",
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
            if (attachments.isNotEmpty()) {
                TextButton(onClick = onClearAttachments) { Text("Clear attachments") }
            }
        }
    }
}

@Composable
private fun FromIdentitySelector(
    identities: List<SendIdentity>,
    selectedKey: String,
    onSelect: (String) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val selected = identities.firstOrNull { identityKey(it) == selectedKey } ?: identities.firstOrNull()
    Box {
        TextButton(onClick = { expanded = true }, modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.fillMaxWidth()) {
                Text("From", fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(
                    selected?.let { formatSendIdentity(it) } ?: "Select sender",
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            identities.forEach { identity ->
                DropdownMenuItem(
                    text = {
                        Text(
                            formatSendIdentity(identity),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    onClick = {
                        onSelect(identityKey(identity))
                        expanded = false
                    },
                )
            }
        }
    }
}

@Composable
private fun ComposeField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    field: String = "",
    suggestions: List<ContactSuggestion> = emptyList(),
    onFocus: (field: String, value: String) -> Unit = { _, _ -> },
    onAcceptSuggestion: (field: String, contact: ContactSuggestion) -> Unit = { _, _ -> },
) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OutlinedTextField(
            value = value,
            onValueChange = onChange,
            label = { Text(label) },
            singleLine = true,
            modifier = Modifier
                .fillMaxWidth()
                .onFocusChanged {
                    if (it.isFocused && field.isNotBlank()) onFocus(field, value)
                },
        )
        if (suggestions.isNotEmpty()) {
            LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                items(suggestions, key = { it.addr }) { contact ->
                    FilterChip(
                        selected = false,
                        onClick = { onAcceptSuggestion(field, contact) },
                        label = {
                            Text(
                                formatContactSuggestion(contact),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        },
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddAccountScreen(
    onBack: () -> Unit,
    initialSection: Int,
    displayName: String, onDisplayNameChange: (String) -> Unit,
    senderName: String, onSenderNameChange: (String) -> Unit,
    email: String, onEmailChange: (String) -> Unit,
    username: String, onUsernameChange: (String) -> Unit,
    password: String, onPasswordChange: (String) -> Unit,
    host: String, onHostChange: (String) -> Unit,
    imapPort: String, onImapPortChange: (String) -> Unit,
    smtpHost: String, onSmtpHostChange: (String) -> Unit,
    smtpPort: String, onSmtpPortChange: (String) -> Unit,
    onAutodiscover: () -> Unit,
    onAddPassword: () -> Unit,
    oauthProvider: String, onOauthProviderChange: (String) -> Unit,
    oauthEmail: String, onOauthEmailChange: (String) -> Unit,
    oauthClientId: String, onOauthClientIdChange: (String) -> Unit,
    oauthClientSecret: String, onOauthClientSecretChange: (String) -> Unit,
    oauthRedirectUri: String, onOauthRedirectUriChange: (String) -> Unit,
    oauthAuthorizationCode: String,
    oauthAccessToken: String, onOauthAccessTokenChange: (String) -> Unit,
    oauthRefreshToken: String, onOauthRefreshTokenChange: (String) -> Unit,
    oauthExpiresAt: String, onOauthExpiresAtChange: (String) -> Unit,
    onLaunchOAuth: () -> Unit,
    onExchangeOAuth: () -> Unit,
    onAddOAuth: () -> Unit,
    rssFeedUrl: String, onRssFeedUrlChange: (String) -> Unit,
    rssDisplayName: String, onRssDisplayNameChange: (String) -> Unit,
    onAddRss: () -> Unit,
    diagnostics: String,
) {
    var section by remember(initialSection) { mutableStateOf(initialSection) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Add account") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
            )
        },
    ) { innerPadding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(innerPadding),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            item {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("Password", "OAuth", "RSS").forEachIndexed { index, label ->
                        FilterChip(
                            selected = section == index,
                            onClick = { section = index },
                            label = { Text(label) },
                        )
                    }
                }
            }
            when (section) {
                0 -> item {
                    SetupCard(title = "IMAP / SMTP account") {
                        SetupField(displayName, onDisplayNameChange, "Display name")
                        SetupField(senderName, onSenderNameChange, "Sender name")
                        SetupField(email, onEmailChange, "Email")
                        SetupField(username, onUsernameChange, "Username")
                        SetupField(password, onPasswordChange, "Password", isPassword = true)
                        OutlinedButton(onClick = onAutodiscover, modifier = Modifier.fillMaxWidth()) {
                            Text("Find mail settings")
                        }
                        SetupField(host, onHostChange, "IMAP host")
                        SetupField(imapPort, onImapPortChange, "IMAP port")
                        SetupField(smtpHost, onSmtpHostChange, "SMTP host")
                        SetupField(smtpPort, onSmtpPortChange, "SMTP port")
                        Button(onClick = onAddPassword, modifier = Modifier.fillMaxWidth()) { Text("Add account") }
                    }
                }
                1 -> item {
                    SetupCard(title = "Gmail / Outlook") {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            FilterChip(oauthProvider == "gmail", { onOauthProviderChange("gmail") }, { Text("Gmail") })
                            FilterChip(oauthProvider == "outlook", { onOauthProviderChange("outlook") }, { Text("Outlook") })
                        }
                        SetupField(oauthEmail, onOauthEmailChange, "Email")
                        SetupField(oauthClientId, onOauthClientIdChange, "Client ID")
                        SetupField(oauthClientSecret, onOauthClientSecretChange, "Client secret (optional)")
                        SetupField(oauthRedirectUri, onOauthRedirectUriChange, "Redirect URI")
                        Button(onClick = onLaunchOAuth, modifier = Modifier.fillMaxWidth()) { Text("Sign in with browser") }
                        if (oauthAuthorizationCode.isNotBlank()) {
                            Text("Authorization code received.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary)
                            Button(onClick = onExchangeOAuth, modifier = Modifier.fillMaxWidth()) { Text("Finish sign-in") }
                        }
                        HorizontalDivider()
                        Text("Or paste tokens manually", style = MaterialTheme.typography.labelLarge)
                        SetupField(oauthAccessToken, onOauthAccessTokenChange, "Access token")
                        SetupField(oauthRefreshToken, onOauthRefreshTokenChange, "Refresh token")
                        SetupField(oauthExpiresAt, onOauthExpiresAtChange, "Token expires at")
                        Button(onClick = onAddOAuth, modifier = Modifier.fillMaxWidth()) { Text("Add with tokens") }
                    }
                }
                else -> item {
                    SetupCard(title = "RSS feed") {
                        SetupField(rssFeedUrl, onRssFeedUrlChange, "Feed URL")
                        SetupField(rssDisplayName, onRssDisplayNameChange, "Feed name")
                        Button(onClick = onAddRss, modifier = Modifier.fillMaxWidth()) { Text("Add feed") }
                    }
                }
            }
            item {
                Text(
                    diagnostics,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun SetupCard(title: String, content: @Composable ColumnScope.() -> Unit) {
    Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            content()
        }
    }
}

@Composable
private fun SetupField(value: String, onChange: (String) -> Unit, label: String, isPassword: Boolean = false) {
    var revealed by remember { mutableStateOf(false) }
    OutlinedTextField(
        value,
        onChange,
        label = { Text(label) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        visualTransformation = if (isPassword && !revealed) PasswordVisualTransformation() else VisualTransformation.None,
        trailingIcon = if (isPassword) {
            {
                IconButton(onClick = { revealed = !revealed }) {
                    Icon(
                        if (revealed) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                        contentDescription = if (revealed) "Hide password" else "Show password",
                    )
                }
            }
        } else null,
    )
}

@Composable
private fun MailDrawer(
    accounts: List<AccountSummary>,
    selectedAccountId: String,
    folders: List<FolderSummary>,
    currentScreen: Screen,
    notificationsNeedPermission: Boolean,
    appearanceMode: AppAppearanceMode,
    storageUsage: StorageUsage?,
    storageBusy: Boolean,
    storageClearConfirming: Boolean,
    showUnreadBadges: Boolean,
    showUnifiedInboxNav: Boolean,
    showStarredNav: Boolean,
    appVersion: String,
    showSenderImages: Boolean,
    sendShortcutMode: SendShortcutMode,
    kanbanColumnWidth: Int,
    onSelectUnified: () -> Unit,
    onSelectAccount: (AccountSummary) -> Unit,
    onSelectStarred: () -> Unit,
    onSelectKanban: () -> Unit,
    onAddAccount: () -> Unit,
    onRefreshBackground: () -> Unit,
    onEnableNotifications: () -> Unit,
    onAppearanceModeChange: (AppAppearanceMode) -> Unit,
    onToggleUnreadBadges: () -> Unit,
    onToggleUnifiedInboxNav: () -> Unit,
    onToggleStarredNav: () -> Unit,
    onToggleSenderImages: () -> Unit,
    onToggleSendShortcut: () -> Unit,
    onCycleKanbanColumnWidth: () -> Unit,
    onRefreshStorage: () -> Unit,
    onClearStorageCache: () -> Unit,
    onShowAbout: () -> Unit,
) {
    val chat = LocalChatColors.current
    ModalDrawerSheet(
        drawerContainerColor = chat.sidebar,
        drawerContentColor = chat.onSidebar,
    ) {
        LazyColumn(contentPadding = PaddingValues(vertical = 16.dp)) {
            item {
                Text(
                    "Meron",
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    color = chat.onSidebar,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
                )
            }
            if (accounts.isNotEmpty()) {
                item { DrawerLabel("Inboxes", chat) }
                if (showUnifiedInboxNav) item {
                    val includedAccountIds = accounts.filter { it.includedInUnified }.map { it.id }.toSet()
                    val unread = folders.filter { it.accountId in includedAccountIds }.sumOf { it.unread }
                    SidebarRow(
                        selected = selectedAccountId == UNIFIED_ACCOUNT_ID,
                        chat = chat,
                        onClick = onSelectUnified,
                        leading = { Icon(Icons.Filled.Inbox, contentDescription = null, modifier = Modifier.size(20.dp)) },
                        title = "Unified inbox",
                        subtitle = "All accounts",
                        trailing = if (showUnreadBadges) unread.takeIf { it > 0 }?.toString() else null,
                    )
                }
                items(accounts, key = { it.id }) { account ->
                    val label = account.displayName.ifBlank { account.email.ifBlank { account.id } }
                    val unread = folders
                        .filter { it.accountId == account.id && it.name.equals(INBOX_FOLDER, ignoreCase = true) }
                        .sumOf { it.unread }
                    SidebarRow(
                        selected = account.id == selectedAccountId,
                        chat = chat,
                        onClick = { onSelectAccount(account) },
                        leading = {
                            Icon(if (accountSummaryIsRss(account)) Icons.Filled.RssFeed else Icons.Filled.Inbox, contentDescription = null, modifier = Modifier.size(20.dp))
                        },
                        title = label,
                        subtitle = account.email.takeIf { it.isNotBlank() && it != label } ?: "Inbox",
                        trailing = if (account.needsReconnect) "!" else if (showUnreadBadges) unread.takeIf { it > 0 }?.toString() else null,
                    )
                }
            }
            if (accounts.isNotEmpty()) {
                item { DrawerLabel("Views", chat) }
                if (showStarredNav) item {
                    SidebarRow(
                        selected = currentScreen == Screen.Starred,
                        chat = chat,
                        onClick = onSelectStarred,
                        leading = { Icon(Icons.Filled.Star, contentDescription = null, modifier = Modifier.size(20.dp)) },
                        title = "Starred",
                        subtitle = "All starred items",
                        trailing = null,
                    )
                }
                item {
                    SidebarRow(
                        selected = currentScreen == Screen.Kanban,
                        chat = chat,
                        onClick = onSelectKanban,
                        leading = { Icon(Icons.Filled.ViewKanban, contentDescription = null, modifier = Modifier.size(20.dp)) },
                        title = "Kanban",
                        subtitle = "Boards and columns",
                        trailing = null,
                    )
                }
            }
            item {
                HorizontalDivider(
                    Modifier.padding(vertical = 10.dp, horizontal = 20.dp),
                    color = chat.onSidebarMuted.copy(alpha = 0.25f),
                )
            }
            item { DrawerLabel("Navigation", chat) }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onToggleUnreadBadges,
                    leading = { Icon(Icons.Filled.Visibility, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Unread badges",
                    subtitle = "Show counts in the drawer",
                    trailing = onOffLabel(showUnreadBadges),
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onToggleUnifiedInboxNav,
                    leading = { Icon(Icons.Filled.Inbox, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Unified inbox",
                    subtitle = "Show in navigation",
                    trailing = onOffLabel(showUnifiedInboxNav),
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onToggleStarredNav,
                    leading = { Icon(Icons.Filled.Star, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Starred",
                    subtitle = "Show in navigation",
                    trailing = onOffLabel(showStarredNav),
                )
            }
            item {
                SidebarRow(
                    selected = false, chat = chat, onClick = onAddAccount,
                    leading = { Icon(Icons.Filled.PersonAdd, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Add account", subtitle = null, trailing = null,
                )
            }
            item {
                SidebarRow(
                    selected = false, chat = chat, onClick = onRefreshBackground,
                    leading = { Icon(Icons.Filled.Refresh, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Refresh in background", subtitle = null, trailing = null,
                )
            }
            if (notificationsNeedPermission) {
                item {
                    SidebarRow(
                        selected = false, chat = chat, onClick = onEnableNotifications,
                        leading = { Icon(Icons.Filled.MarkEmailUnread, contentDescription = null, modifier = Modifier.size(20.dp)) },
                        title = "Enable notifications", subtitle = null, trailing = null,
                    )
                }
            }
            item { DrawerLabel("Appearance", chat) }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = { onAppearanceModeChange(appearanceMode.next()) },
                    leading = { Icon(Icons.Filled.Visibility, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Theme",
                    subtitle = "System and desktop presets",
                    trailing = appearanceMode.label,
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onToggleSenderImages,
                    leading = { Icon(Icons.Filled.Visibility, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Sender images",
                    subtitle = "Use Gravatar and site icons",
                    trailing = onOffLabel(showSenderImages),
                )
            }
            item { DrawerLabel("Composer", chat) }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onToggleSendShortcut,
                    leading = { Icon(Icons.AutoMirrored.Filled.Send, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Send shortcut",
                    subtitle = "Hardware keyboard behavior",
                    trailing = sendShortcutMode.label(),
                )
            }
            item { DrawerLabel("Kanban", chat) }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onCycleKanbanColumnWidth,
                    leading = { Icon(Icons.Filled.ViewKanban, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Column width",
                    subtitle = "Adjust board density",
                    trailing = "${kanbanColumnWidth}dp",
                )
            }
            item { DrawerLabel("Storage", chat) }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onRefreshStorage,
                    leading = { Icon(Icons.Filled.Info, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "Storage usage",
                    subtitle = storageUsage?.let { "Cache ${formatBytes(it.cacheBytes)} · Database ${formatBytes(it.dbBytes)}" }
                        ?: if (storageBusy) "Loading..." else "Tap to refresh",
                    trailing = null,
                )
            }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onClearStorageCache,
                    leading = { Icon(Icons.Filled.Delete, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = if (storageClearConfirming) "Confirm clear cache" else "Clear cache",
                    subtitle = if (storageBusy) "Working..." else "Remove cached attachments only",
                    trailing = storageUsage?.cacheBytes?.takeIf { it > 0 }?.let(::formatBytes),
                )
            }
            item { DrawerLabel("About", chat) }
            item {
                SidebarRow(
                    selected = false,
                    chat = chat,
                    onClick = onShowAbout,
                    leading = { Icon(Icons.Filled.Info, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    title = "About Meron",
                    subtitle = "Version and support",
                    trailing = appVersion,
                )
            }
        }
    }
}

@Composable
private fun AboutDialog(
    appVersion: String,
    packageName: String,
    coreProtocolVersion: Int,
    sharedProtocolVersion: Int,
    onOpenUrl: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("About Meron") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Meron", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                Text(
                    "Version $appVersion · Package $packageName",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    "Core protocol $coreProtocolVersion · Shared protocol $sharedProtocolVersion",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { onOpenUrl("https://github.com/sponsors/nonbili") }) {
                        Text("GitHub Sponsors")
                    }
                    OutlinedButton(onClick = { onOpenUrl("https://liberapay.com/nonbili") }) {
                        Text("Liberapay")
                    }
                    OutlinedButton(onClick = { onOpenUrl("https://www.paypal.com/paypalme/nonbili") }) {
                        Text("PayPal")
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("Close")
            }
        },
    )
}

@Composable
private fun SidebarRow(
    selected: Boolean,
    chat: ChatColors,
    onClick: () -> Unit,
    leading: @Composable () -> Unit,
    title: String,
    subtitle: String?,
    trailing: String?,
) {
    val tint = if (selected) MaterialTheme.colorScheme.primary else chat.onSidebarMuted
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.18f) else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(contentAlignment = Alignment.Center) {
            androidx.compose.runtime.CompositionLocalProvider(
                androidx.compose.material3.LocalContentColor provides tint,
            ) { leading() }
        }
        Column(Modifier.weight(1f)) {
            Text(
                title,
                color = if (selected) chat.onSidebar else chat.onSidebar.copy(alpha = 0.9f),
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                fontSize = 14.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle != null) {
                Text(
                    subtitle,
                    color = chat.onSidebarMuted,
                    fontSize = 11.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (trailing != null) {
            Box(
                Modifier
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.primary)
                    .padding(horizontal = 7.dp, vertical = 2.dp),
            ) {
                Text(trailing, color = Color.White, fontSize = 11.sp, fontWeight = FontWeight.Bold)
            }
        }
    }
}

@Composable
private fun DrawerLabel(text: String, chat: ChatColors) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        fontWeight = FontWeight.Bold,
        color = chat.onSidebarMuted,
        modifier = Modifier.padding(start = 24.dp, top = 14.dp, bottom = 6.dp),
    )
}

@Composable
private fun StatusBanner(
    message: String,
    isError: Boolean,
    actionLabel: String?,
    onAction: (() -> Unit)?,
    onDismiss: (() -> Unit)?,
) {
    val container = if (isError) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.primaryContainer
    val content = if (isError) MaterialTheme.colorScheme.onErrorContainer else MaterialTheme.colorScheme.onPrimaryContainer
    Surface(color = container, contentColor = content) {
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(Icons.Filled.ErrorOutline, contentDescription = null, modifier = Modifier.size(20.dp))
            Text(message, fontSize = 13.sp, modifier = Modifier.weight(1f), maxLines = 3, overflow = TextOverflow.Ellipsis)
            if (actionLabel != null && onAction != null) {
                TextButton(onClick = onAction) {
                    Text(actionLabel, color = content, fontWeight = FontWeight.SemiBold)
                }
            }
            if (onDismiss != null) {
                IconButton(onClick = onDismiss, modifier = Modifier.size(32.dp)) {
                    Icon(Icons.Filled.Close, contentDescription = "Dismiss", modifier = Modifier.size(18.dp))
                }
            }
        }
    }
}

@Composable
private fun EmptyState(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    text: String,
    actionLabel: String,
    onAction: () -> Unit,
) {
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(56.dp), tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(16.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(8.dp))
        Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(20.dp))
        Button(onClick = onAction) { Text(actionLabel) }
    }
}

// Gradient pairs mirror the desktop avatar palette in
// frontend/src/components/avatar/Avatar.tsx (Tailwind 400 -> 500 shades).
private val avatarGradients = listOf(
    Color(0xFF818CF8) to Color(0xFF6366F1), // indigo
    Color(0xFFA78BFA) to Color(0xFF8B5CF6), // violet
    Color(0xFF2DD4BF) to Color(0xFF14B8A6), // teal
    Color(0xFF34D399) to Color(0xFF10B981), // emerald
    Color(0xFFFB7185) to Color(0xFFF43F5E), // rose
    Color(0xFFFBBF24) to Color(0xFFF59E0B), // amber
    Color(0xFF38BDF8) to Color(0xFF0EA5E9), // sky
    Color(0xFFE879F9) to Color(0xFFD946EF), // fuchsia
)

private fun avatarBrush(name: String): Brush {
    val key = name.ifBlank { "?" }
    var hash = 0
    for (ch in key) hash = ch.code + ((hash shl 5) - hash)
    val (start, end) = avatarGradients[abs(hash) % avatarGradients.size]
    return Brush.linearGradient(listOf(start, end))
}

private fun avatarInitials(value: String): String {
    val parts = value.split(' ', '\t', '\n', '@').filter { it.isNotBlank() }
    if (parts.isEmpty()) return "?"
    return parts.take(2).mapNotNull { it.firstOrNull()?.uppercaseChar() }.joinToString("")
}

private fun isAuthError(message: String): Boolean {
    val m = message.lowercase()
    return listOf("auth", "login", "credential", "password", "unauthor", "permission", "token", "401", "535")
        .any { m.contains(it) }
}

private fun isOutgoing(message: MessageBody, accountEmail: String): Boolean {
    if (accountEmail.isBlank()) return false
    val acct = accountEmail.trim().lowercase()
    val from = message.fromAddr.ifBlank { message.from }.trim().lowercase()
    return from.contains(acct)
}

private fun folderIcon(name: String): androidx.compose.ui.graphics.vector.ImageVector = when (name.lowercase()) {
    "inbox" -> Icons.Filled.Inbox
    "sent" -> Icons.AutoMirrored.Filled.Send
    "drafts" -> Icons.Outlined.Drafts
    "archive" -> Icons.Filled.Archive
    "trash", "deleted" -> Icons.Filled.Delete
    "starred" -> Icons.Filled.Star
    else -> Icons.Outlined.FolderOpen
}

private fun kanbanColumnKey(column: KanbanColumnSpec): String = "${column.accountId}\n${column.folderId}"

private fun identityKey(identity: SendIdentity): String = "${identity.accountId}|${identity.email}"

private fun FilterMode.label(): String = when (this) {
    FilterMode.All -> "All"
    FilterMode.Unread -> "Unread"
    FilterMode.Starred -> "Starred"
}

private fun FilterMode.protocolValue(): String = when (this) {
    FilterMode.All -> "all"
    FilterMode.Unread -> "unread"
    FilterMode.Starred -> "starred"
}

private fun FilterMode.emptyNoun(): String = when (this) {
    FilterMode.All -> "cards"
    FilterMode.Unread -> "unread cards"
    FilterMode.Starred -> "starred cards"
}

private fun List<ThreadSummary>.filteredKanbanThreads(filter: FilterMode, search: String): List<ThreadSummary> {
    val query = search.trim().lowercase()
    return filter { thread ->
        val filterOk = when (filter) {
            FilterMode.All -> true
            FilterMode.Unread -> thread.unread
            FilterMode.Starred -> thread.starred
        }
        val queryOk = query.isBlank() ||
            thread.subject.lowercase().contains(query) ||
            thread.sender.lowercase().contains(query) ||
            thread.preview.lowercase().contains(query) ||
            thread.accountId.lowercase().contains(query)
        filterOk && queryOk
    }.sortedByDescending { it.dateEpochSeconds }
}

private fun columnTitle(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
): String {
    if (column.accountId == UNIFIED_ACCOUNT_ID) return "Unified inbox"
    val account = accounts.firstOrNull { it.id == column.accountId }
    val folder = foldersByAccount[column.accountId]
        ?.firstOrNull { it.name.equals(column.folderId, ignoreCase = true) }
        ?.name
        ?: column.folderId
    val folderLabel = if (folder.equals(INBOX_FOLDER, ignoreCase = true)) {
        if (account?.let(::accountSummaryIsRss) == true) "Feed" else "Inbox"
    } else {
        folder
    }
    val accountLabel = account?.displayName?.ifBlank { account.email } ?: column.accountId
    return "$accountLabel / $folderLabel"
}

private fun KanbanBoardSpec.hasBoardStyle(): Boolean {
    return avatarUrl.isNotBlank() || wallpaperPresetId.isNotBlank() || wallpaperUrl.isNotBlank()
}

@Composable
private fun KanbanBoardTile(board: KanbanBoardSpec?, size: Dp) {
    val styled = board?.hasBoardStyle() == true
    val tileBrush = if (styled) {
        avatarBrush(board.avatarUrl.ifBlank { board.name })
    } else {
        Brush.linearGradient(listOf(MaterialTheme.colorScheme.primaryContainer, MaterialTheme.colorScheme.primaryContainer))
    }
    Box(
        Modifier
            .size(size)
            .clip(RoundedCornerShape(8.dp))
            .background(tileBrush),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            Icons.Filled.ViewKanban,
            contentDescription = null,
            tint = if (styled) Color.White else MaterialTheme.colorScheme.onPrimaryContainer,
            modifier = Modifier.size(size * 0.55f),
        )
    }
}

@Composable
private fun boardBackgroundBrush(board: KanbanBoardSpec?): Brush? {
    if (board == null || !board.hasBoardStyle()) return null
    return Brush.verticalGradient(
        listOf(
            MaterialTheme.colorScheme.background,
            MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.92f),
            MaterialTheme.colorScheme.background,
        ),
    )
}

private fun defaultKanbanBoard(accounts: List<AccountSummary>): KanbanBoardSpec {
    val columns = mutableListOf(KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER))
    accounts.forEach { account -> columns += KanbanColumnSpec(account.id, INBOX_FOLDER) }
    return KanbanBoardSpec(
        id = "kb-${UUID.randomUUID()}",
        name = "Kanban board",
        columns = columns.distinctBy(::kanbanColumnKey),
    )
}

private fun ensureKanbanDefaults(
    context: Context,
    boards: List<KanbanBoardSpec>,
    accounts: List<AccountSummary>,
): List<KanbanBoardSpec> {
    val next = if (boards.isEmpty()) {
        listOf(defaultKanbanBoard(accounts))
    } else {
        boards.mapIndexed { index, board ->
            if (index != 0) board else {
                val existing = board.columns.map(::kanbanColumnKey).toMutableSet()
                val columns = board.columns.toMutableList()
                val unified = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                if (existing.add(kanbanColumnKey(unified))) columns.add(0, unified)
                accounts.forEach { account ->
                    val column = KanbanColumnSpec(account.id, INBOX_FOLDER)
                    if (existing.add(kanbanColumnKey(column))) columns.add(column)
                }
                board.copy(columns = columns)
            }
        }
    }
    if (next != boards) saveKanbanBoards(context, next)
    return next
}

private fun loadKanbanBoards(context: Context, accounts: List<AccountSummary>): List<KanbanBoardSpec> {
    val raw = context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(KANBAN_BOARDS_PREF, null)
    val parsed = runCatching {
        if (raw.isNullOrBlank()) emptyList() else {
            val array = JSONArray(raw)
            buildList {
                for (i in 0 until array.length()) {
                    val obj = array.optJSONObject(i) ?: continue
                    val id = obj.optString("id").ifBlank { "kb-${UUID.randomUUID()}" }
                    val name = obj.optString("name").ifBlank { "Kanban board" }
                    val avatarUrl = obj.optString("avatarUrl")
                    val wallpaper = obj.optJSONObject("wallpaper")
                    val wallpaperPresetId = wallpaper?.optString("presetId").orEmpty()
                    val wallpaperUrl = wallpaper?.optString("url").orEmpty()
                    val colArray = obj.optJSONArray("columns") ?: JSONArray()
                    val columns = buildList {
                        for (j in 0 until colArray.length()) {
                            val col = colArray.optJSONObject(j) ?: continue
                            val accountId = col.optString("accountId")
                            val folderId = col.optString("folderId")
                            if (accountId.isNotBlank() && folderId.isNotBlank()) add(KanbanColumnSpec(accountId, folderId))
                        }
                    }.distinctBy(::kanbanColumnKey)
                    add(KanbanBoardSpec(id, name, columns, avatarUrl, wallpaperPresetId, wallpaperUrl))
                }
            }
        }
    }.getOrDefault(emptyList())
    return ensureKanbanDefaults(context, parsed, accounts)
}

private fun saveKanbanBoards(context: Context, boards: List<KanbanBoardSpec>) {
    val array = JSONArray()
    boards.forEach { board ->
        val columns = JSONArray()
        board.columns.forEach { column ->
            columns.put(JSONObject().put("accountId", column.accountId).put("folderId", column.folderId))
        }
        val obj = JSONObject().put("id", board.id).put("name", board.name).put("columns", columns)
        if (board.avatarUrl.isNotBlank()) obj.put("avatarUrl", board.avatarUrl)
        when {
            board.wallpaperUrl.isNotBlank() -> obj.put("wallpaper", JSONObject().put("kind", "custom").put("url", board.wallpaperUrl))
            board.wallpaperPresetId.isNotBlank() -> obj.put("wallpaper", JSONObject().put("kind", "preset").put("presetId", board.wallpaperPresetId))
        }
        array.put(obj)
    }
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(KANBAN_BOARDS_PREF, array.toString())
        .apply()
}

private fun loadActiveKanbanBoardId(context: Context): String {
    return context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(ACTIVE_KANBAN_BOARD_PREF, "").orEmpty()
}

private fun saveActiveKanbanBoardId(context: Context, boardId: String) {
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).edit().putString(ACTIVE_KANBAN_BOARD_PREF, boardId).apply()
}

private fun loadKanbanFilter(context: Context): FilterMode {
    val raw = context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(KANBAN_FILTER_PREF, "all")
    return when (raw) {
        "unread" -> FilterMode.Unread
        "starred" -> FilterMode.Starred
        else -> FilterMode.All
    }
}

private fun saveKanbanFilter(context: Context, filter: FilterMode) {
    val value = when (filter) {
        FilterMode.All -> "all"
        FilterMode.Unread -> "unread"
        FilterMode.Starred -> "starred"
    }
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).edit().putString(KANBAN_FILTER_PREF, value).apply()
}

private fun loadKanbanSearch(context: Context): String {
    return context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).getString(KANBAN_SEARCH_PREF, "").orEmpty()
}

private fun saveKanbanSearch(context: Context, search: String) {
    context.getSharedPreferences(KANBAN_PREFS, Context.MODE_PRIVATE).edit().putString(KANBAN_SEARCH_PREF, search).apply()
}

private fun appVersionName(context: Context): String {
    return runCatching {
        val packageInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.packageManager.getPackageInfo(
                context.packageName,
                android.content.pm.PackageManager.PackageInfoFlags.of(0),
            )
        } else {
            @Suppress("DEPRECATION")
            context.packageManager.getPackageInfo(context.packageName, 0)
        }
        packageInfo.versionName ?: "0.1.0"
    }.getOrDefault("0.1.0")
}

private fun SendShortcutMode.label(): String = when (this) {
    SendShortcutMode.Enter -> "Enter"
    SendShortcutMode.ModEnter -> "Ctrl+Enter"
}

private fun SendShortcutMode.storageValue(): String = when (this) {
    SendShortcutMode.Enter -> "enter"
    SendShortcutMode.ModEnter -> "mod_enter"
}

private fun SendShortcutMode.next(): SendShortcutMode = when (this) {
    SendShortcutMode.Enter -> SendShortcutMode.ModEnter
    SendShortcutMode.ModEnter -> SendShortcutMode.Enter
}

private fun loadSendShortcutMode(context: Context): SendShortcutMode {
    return when (context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE).getString(SEND_SHORTCUT_PREF, "mod_enter")) {
        "enter" -> SendShortcutMode.Enter
        else -> SendShortcutMode.ModEnter
    }
}

private fun saveSendShortcutMode(context: Context, mode: SendShortcutMode) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(SEND_SHORTCUT_PREF, mode.storageValue())
        .apply()
}

private fun shouldSendFromEditor(event: KeyEvent, mode: SendShortcutMode): Boolean {
    if (event.type != KeyEventType.KeyDown || event.key != Key.Enter) return false
    return when (mode) {
        SendShortcutMode.Enter -> !event.isShiftPressed && !event.isCtrlPressed && !event.isMetaPressed
        SendShortcutMode.ModEnter -> !event.isShiftPressed && (event.isCtrlPressed || event.isMetaPressed)
    }
}

private fun loadAppearanceMode(context: Context): AppAppearanceMode {
    val stored = context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .getString(APPEARANCE_MODE_PREF, AppAppearanceMode.System.storageValue)
        .orEmpty()
    return AppAppearanceMode.entries.firstOrNull { it.storageValue == stored } ?: AppAppearanceMode.System
}

private fun saveAppearanceMode(context: Context, mode: AppAppearanceMode) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putString(APPEARANCE_MODE_PREF, mode.storageValue)
        .apply()
}

private fun loadAppBoolean(context: Context, key: String, defaultValue: Boolean): Boolean {
    return context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE).getBoolean(key, defaultValue)
}

private fun saveAppBoolean(context: Context, key: String, value: Boolean) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putBoolean(key, value)
        .apply()
}

private fun loadAppInt(context: Context, key: String, defaultValue: Int): Int {
    return context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE).getInt(key, defaultValue)
}

private fun saveAppInt(context: Context, key: String, value: Int) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putInt(key, value)
        .apply()
}

private fun nextKanbanColumnWidth(current: Int): Int {
    val next = current + KANBAN_COLUMN_WIDTH_STEP
    return if (next > KANBAN_COLUMN_MAX_WIDTH) KANBAN_COLUMN_MIN_WIDTH else next
}

private fun loadAppStringSet(context: Context, key: String): Set<String> {
    return context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .getStringSet(key, emptySet())
        .orEmpty()
        .filter { it.isNotBlank() }
        .toSet()
}

private fun saveAppStringSet(context: Context, key: String, value: Set<String>) {
    context.getSharedPreferences(APP_PREFS, Context.MODE_PRIVATE)
        .edit()
        .putStringSet(key, value.filter { it.isNotBlank() }.toSet())
        .apply()
}

private fun onOffLabel(enabled: Boolean): String = if (enabled) "On" else "Off"

private fun AppAppearanceMode.next(): AppAppearanceMode {
    val values = AppAppearanceMode.entries
    return values[(ordinal + 1) % values.size]
}

private fun formatRelativeTime(epochSeconds: Long): String {
    if (epochSeconds <= 0) return ""
    val nowMillis = System.currentTimeMillis()
    val thenMillis = epochSeconds * 1000
    val diff = nowMillis - thenMillis
    return when {
        diff < 60_000 -> "now"
        diff < 3_600_000 -> "${diff / 60_000}m"
        diff < 86_400_000 -> "${diff / 3_600_000}h"
        diff < 7 * 86_400_000L -> SimpleDateFormat("EEE", Locale.getDefault()).format(Date(thenMillis))
        else -> SimpleDateFormat("MMM d", Locale.getDefault()).format(Date(thenMillis))
    }
}

private fun formatBytes(bytes: Long): String = when {
    bytes >= 1_000_000 -> "${bytes / 1_000_000} MB"
    bytes >= 1_000 -> "${bytes / 1_000} KB"
    else -> "$bytes B"
}

private fun safeAttachmentFilename(name: String): String {
    val cleaned = name
        .ifBlank { "attachment" }
        .map { ch -> if (ch.isLetterOrDigit() || ch in listOf('.', '-', '_')) ch else '_' }
        .joinToString("")
        .trim('.', '_')
    return cleaned.ifBlank { "attachment" }
}

private fun android.content.Context.displayNameFor(uri: Uri): String {
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

private fun Intent.toMailtoDraft(): ComposeDraft? {
    val uri = data?.toString() ?: return null
    if (action != Intent.ACTION_SENDTO && action != Intent.ACTION_VIEW) return null
    return parseMailtoUrl(uri)
}

private fun Intent.toOAuthCallbackUrl(): String? {
    val uri = data?.toString() ?: return null
    return uri.takeIf { isPotentialOAuthCallbackUrl(it) || isOAuthCallbackUrl(it) }
}

private fun coreStatus(coreInitJson: String): String {
    return if (MeronCoreNative.isLoaded()) {
        "Core protocol ${MeronCoreNative.protocolVersion()} · shared ${SharedMobileContract.protocolVersion}"
    } else {
        "Rust core not packaged yet; using Java fallback."
    }
}

private fun String.pkceChallenge(): String {
    val digest = MessageDigest.getInstance("SHA-256").digest(toByteArray())
    return Base64.encodeToString(digest, Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)
}
