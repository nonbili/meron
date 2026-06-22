package jp.nonbili.meron

import android.Manifest
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Base64
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
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
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
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
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseThreadReadResponse
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
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
            MeronCoreNative.initJson(filesDir.absolutePath)
        } else {
            ""
        }
        setContent {
            MeronTheme {
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    MeronMobileScreen(
                        coreInitJson = coreInitJson,
                        incomingMailtoDraft = incomingMailtoDraft,
                        incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
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

private enum class Screen { Mail, Kanban, Thread, Compose, AddAccount }
private enum class FilterMode { All, Unread, Starred }

private const val UNIFIED_ACCOUNT_ID = "unified"
private const val INBOX_FOLDER = "inbox"
private const val KANBAN_PREFS = "kanban"
private const val KANBAN_BOARDS_PREF = "kanban_boards_v1"
private const val ACTIVE_KANBAN_BOARD_PREF = "active_kanban_board_id_v1"
private const val KANBAN_FILTER_PREF = "kanban_filter_v1"
private const val KANBAN_SEARCH_PREF = "kanban_search_v1"

private data class MailboxLoadResult(
    val folders: List<FolderSummary>,
    val folder: String,
    val threads: List<ThreadSummary>,
)

private data class KanbanColumnSpec(
    val accountId: String,
    val folderId: String,
)

private data class KanbanBoardSpec(
    val id: String,
    val name: String,
    val columns: List<KanbanColumnSpec>,
)

private data class KanbanColumnState(
    val threads: List<ThreadSummary> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MeronMobileScreen(
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft?,
    incomingOAuthCallbackUrl: String?,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val snackbarHost = remember { SnackbarHostState() }

    var host by remember { mutableStateOf("10.0.2.2") }
    var email by remember { mutableStateOf("user1@mail.localhost") }
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
    var coreThreads by remember { mutableStateOf(emptyList<ThreadSummary>()) }
    var selectedCoreThread by remember { mutableStateOf<ThreadSummary?>(null) }
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
    var kanbanFolderNameInput by remember { mutableStateOf("") }
    var to by remember { mutableStateOf("") }
    var subject by remember { mutableStateOf("") }
    var body by remember { mutableStateOf("") }
    var quickReplyBody by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("") }
    var syncing by remember { mutableStateOf(false) }
    var messages by remember { mutableStateOf(emptyList<MessageBody>()) }
    var attachments by remember { mutableStateOf(emptyList<DraftAttachment>()) }
    var cc by remember { mutableStateOf("") }
    var bcc by remember { mutableStateOf("") }
    var screen by remember { mutableStateOf(Screen.Mail) }
    var errorBanner by remember { mutableStateOf<String?>(null) }
    var addSection by remember { mutableStateOf(0) }
    var notificationPermissionGranted by remember { mutableStateOf(AndroidNotificationService.canNotify(context)) }

    val notificationPermissionLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        notificationPermissionGranted = granted
        status = if (granted) "Notifications enabled" else "Notifications are disabled"
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

    LaunchedEffect(status) {
        if (status.isNotBlank()) {
            snackbarHost.showSnackbar(status)
        }
    }

    LaunchedEffect(incomingMailtoDraft) {
        incomingMailtoDraft?.let { draft ->
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
            username = email.trim(),
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
    ): MailboxLoadResult {
        if (accountSummaryIsRss(account)) {
            client.syncRss(SyncRssParams(accountId = account.id))
        } else {
            client.sync(SyncMailParams(accountId = account.id, folderId = requestedFolder, limit = 50, folders = true))
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
        val threadsJson = client.listThreads(ThreadListParams(accountId = account.id, folderId = folder))
        return MailboxLoadResult(
            folders = folders.filter { it.name.equals(folder, ignoreCase = true) },
            folder = folder,
            threads = parseThreadListResponse(threadsJson),
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
    ): Pair<List<FolderSummary>, List<ThreadSummary>> {
        return if (column.accountId == UNIFIED_ACCOUNT_ID) {
            val results = coreAccounts.map { account ->
                loadAccountInbox(client, account, INBOX_FOLDER)
            }
            results.flatMap { it.folders } to results.flatMap { it.threads }.sortedByDescending { it.dateEpochSeconds }
        } else {
            val account = coreAccounts.firstOrNull { it.id == column.accountId } ?: return emptyList<FolderSummary>() to emptyList()
            if (refresh) {
                if (accountSummaryIsRss(account)) client.syncRss(SyncRssParams(accountId = account.id))
                else client.sync(SyncMailParams(accountId = account.id, folderId = column.folderId, limit = 50, folders = true))
            }
            val folders = loadAccountFolders(client, account)
            val folder = folders.firstOrNull { it.name.equals(column.folderId, ignoreCase = true) }?.name ?: column.folderId
            val threadsJson = client.listThreads(ThreadListParams(accountId = account.id, folderId = folder))
            folders to parseThreadListResponse(threadsJson)
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
            }.onSuccess { (folders, threads) ->
                if (folders.isNotEmpty()) {
                    foldersByAccount = foldersByAccount + folders.groupBy { it.accountId }
                }
                updateKanbanColumn(key) { it.copy(threads = threads, loading = false, error = null) }
            }.onFailure {
                updateKanbanColumn(key) { state -> state.copy(loading = false, error = it.message ?: "Load failed") }
                status = "Kanban load failed: ${it.message}"
            }
        }
    }

    fun loadKanbanBoard(refresh: Boolean = false) {
        val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
        board.columns.forEach { column -> loadKanbanColumn(column, refresh) }
    }

    fun syncCoreThreads(accountOverride: String? = null, folderOverride: String? = null) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val accountId = accountOverride ?: selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
        val requestedFolder = folderOverride ?: selectedCoreFolder.ifBlank { INBOX_FOLDER }
        val selectedAccounts = if (accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts
        } else {
            coreAccounts.filter { it.id == accountId }
        }
        if (selectedAccounts.isEmpty()) {
            status = "No account selected."
            return
        }
        syncing = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    if (accountId == UNIFIED_ACCOUNT_ID) {
                        val results = selectedAccounts.map { account ->
                            loadAccountInbox(client, account, INBOX_FOLDER)
                        }
                        MailboxLoadResult(
                            folders = results.flatMap { it.folders },
                            folder = INBOX_FOLDER,
                            threads = results.flatMap { it.threads }.sortedByDescending { it.dateEpochSeconds },
                        )
                    } else {
                        loadAccountInbox(client, selectedAccounts.first(), requestedFolder)
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
                if (selectedCoreThread?.id !in parsedThreads.map { it.id }) {
                    selectedCoreThread = null
                    messages = emptyList()
                }
                syncing = false
                errorBanner = null
                status = if (accountId == UNIFIED_ACCOUNT_ID) {
                    "${parsedThreads.size} message(s) in Unified inbox"
                } else {
                    "${parsedThreads.size} message(s) in ${folder.replaceFirstChar { it.uppercase() }}"
                }
            }.onFailure {
                syncing = false
                errorBanner = it.message ?: "Sync failed"
                status = "Sync failed: ${it.message}"
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
        previousTopScreen = if (screen == Screen.Kanban) Screen.Kanban else Screen.Mail
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
                messages = parseThreadReadResponse(it)
                updateThreadEverywhere(thread) { current -> current.copy(unread = false) }
            }.onFailure {
                status = "Could not open message: ${it.message}"
            }
        }
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
            label = "Delete",
            action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
        )
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

    fun sendMail() {
        val accountId = defaultSendAccountId()
        if (accountId.isBlank()) {
            status = "Select or add an account before sending."
            return
        }
        val draft = ComposeDraft(to.trim(), cc.trim(), bcc.trim(), subject.trim(), body.trim(), attachments)
        if (!draft.canSend) {
            status = "Complete To, Subject, and Body before sending."
            return
        }
        status = "Sending..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.send(draft.toSendMailParams(accountId = accountId))
                }
            }.onSuccess {
                attachments = emptyList()
                to = ""; cc = ""; bcc = ""; subject = ""; body = ""
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
        if (replyBody.isBlank()) {
            status = "Write a reply before sending."
            return
        }
        status = "Sending reply..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).send(parent.toReplyMailParams(accountId, replyBody))
                }
            }.onSuccess {
                quickReplyBody = ""
                status = "Reply sent"
            }.onFailure {
                status = "Reply failed: ${it.message}"
            }
        }
    }

    fun openCompose() {
        to = ""; cc = ""; bcc = ""; subject = ""; body = ""; attachments = emptyList()
        previousTopScreen = if (screen == Screen.Kanban) Screen.Kanban else Screen.Mail
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
                password = ""
                if (account.imapHost.isNotBlank()) host = account.imapHost
                if (account.imapPort > 0) imapPort = account.imapPort.toString()
                if (account.smtpHost.isNotBlank()) smtpHost = account.smtpHost
                if (account.smtpPort > 0) smtpPort = account.smtpPort.toString()
                addSection = 0
            }
        }
        errorBanner = null
        previousTopScreen = if (screen == Screen.Kanban) Screen.Kanban else Screen.Mail
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
        }
    }

    LaunchedEffect(coreAccounts, activeKanbanBoardId) {
        if (coreAccounts.isNotEmpty() && screen == Screen.Kanban) {
            loadKanbanBoard(refresh = false)
        }
    }

    val selectedAccount = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }
    val selectedThreadAccount = selectedCoreThread?.accountId?.let { accountId -> coreAccounts.firstOrNull { it.id == accountId } }
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
            onBack = { screen = previousTopScreen },
            onArchive = { selectedCoreThread?.let { archiveOrRemove(it); screen = previousTopScreen } },
            onDelete = { selectedCoreThread?.let { deleteThread(it); screen = previousTopScreen } },
            onToggleStar = { selectedCoreThread?.let { t -> toggleStar(t); selectedCoreThread = t.copy(starred = !t.starred) } },
            quickReplyBody = quickReplyBody,
            onQuickReplyChange = { quickReplyBody = it },
            onSendReply = ::sendQuickReply,
        )

        Screen.Compose -> ComposeScreen(
            to = to, onToChange = { to = it },
            cc = cc, onCcChange = { cc = it },
            bcc = bcc, onBccChange = { bcc = it },
            subject = subject, onSubjectChange = { subject = it },
            body = body, onBodyChange = { body = it },
            attachments = attachments,
            onAttach = { attachmentPicker.launch(arrayOf("*/*")) },
            onClearAttachments = { attachments = emptyList() },
            onSend = ::sendMail,
            onBack = { screen = previousTopScreen },
        )

        Screen.AddAccount -> AddAccountScreen(
            onBack = { screen = previousTopScreen },
            initialSection = addSection,
            displayName = displayName, onDisplayNameChange = { displayName = it },
            senderName = senderName, onSenderNameChange = { senderName = it },
            email = email, onEmailChange = { email = it },
            password = password, onPasswordChange = { password = it },
            host = host, onHostChange = { host = it },
            imapPort = imapPort, onImapPortChange = { imapPort = it },
            smtpHost = smtpHost, onSmtpHostChange = { smtpHost = it },
            smtpPort = smtpPort, onSmtpPortChange = { smtpPort = it },
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

        Screen.Kanban -> ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts,
                    selectedAccountId = selectedCoreAccountId,
                    folders = coreFolders,
                    currentScreen = screen,
                    notificationsNeedPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted,
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
                )
            },
        ) {
            Scaffold(
                topBar = {
                    TopAppBar(
                        title = {
                            Column {
                                Text(activeKanbanBoard?.name ?: "Kanban board", fontWeight = FontWeight.SemiBold)
                                Text(
                                    "${activeKanbanBoard?.columns?.size ?: 0} columns",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        },
                        navigationIcon = {
                            IconButton(onClick = { scope.launch { drawerState.open() } }) {
                                Icon(Icons.Filled.Menu, contentDescription = "Open navigation")
                            }
                        },
                        actions = {
                            IconButton(onClick = { showKanbanBoardDialog = true; kanbanBoardNameInput = activeKanbanBoard?.name.orEmpty() }) {
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
                    accounts = coreAccounts,
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
                    onRemoveColumn = ::removeKanbanColumn,
                    onMoveColumn = ::moveKanbanColumn,
                    onAddColumn = { showKanbanColumnDialog = true },
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
                                title = "Nothing here yet",
                                text = "Pull in your latest messages from the server.",
                                actionLabel = "Sync now",
                                onAction = ::syncCoreThreads,
                            )
                            else -> MailList(
                                threads = coreThreads,
                                onOpen = ::readCoreThread,
                                onToggleStar = ::toggleStar,
                                onArchive = ::archiveOrRemove,
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
            onNameChange = { kanbanBoardNameInput = it },
            onSelect = {
                activeKanbanBoardId = it
                saveActiveKanbanBoardId(context, it)
                showKanbanBoardDialog = false
                loadKanbanBoard(refresh = false)
            },
            onRename = { renameActiveKanbanBoard(kanbanBoardNameInput) },
            onCreate = { createKanbanBoard() },
            onDelete = { deleteActiveKanbanBoard(); showKanbanBoardDialog = false },
            onDismiss = { showKanbanBoardDialog = false },
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
            onMove = { target -> kanbanActionThread = null; moveThreadToColumn(thread, target) },
        )
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
    onRemoveColumn: (KanbanColumnSpec) -> Unit,
    onMoveColumn: (KanbanColumnSpec, Int) -> Unit,
    onAddColumn: () -> Unit,
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
    Column(modifier.background(MaterialTheme.colorScheme.background)) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
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
                        onRemove = { onRemoveColumn(column) },
                        onMoveLeft = { onMoveColumn(column, -1) },
                        onMoveRight = { onMoveColumn(column, 1) },
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
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
) {
    val visibleThreads = state.threads.filteredKanbanThreads(filter, search)
    Card(
        modifier = Modifier.width(320.dp).fillMaxSize(),
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
                            onOpen = { onOpen(thread) },
                            onLongPress = { onLongPress(thread) },
                            onToggleStar = { onToggleStar(thread) },
                            onArchive = { onArchive(thread) },
                        )
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
private fun KanbanBoardDialog(
    boards: List<KanbanBoardSpec>,
    activeBoardId: String,
    name: String,
    onNameChange: (String) -> Unit,
    onSelect: (String) -> Unit,
    onRename: () -> Unit,
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
                items(boards, key = { it.id }) { board ->
                    SidebarLikeDialogRow(
                        selected = board.id == activeBoardId,
                        title = board.name,
                        subtitle = "${board.columns.size} columns",
                        onClick = { onSelect(board.id) },
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onRename) { Text("Rename") }
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
    onMove: (KanbanColumnSpec) -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(thread.subject.ifBlank { "(no subject)" }, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        text = {
            LazyColumn(Modifier.heightIn(max = 430.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item { DialogAction("Open", onOpen) }
                item { DialogAction(if (thread.starred) "Unstar" else "Star", onToggleStar) }
                item { DialogAction(if (thread.unread) "Mark read" else "Mark unread", onToggleRead) }
                item { DialogAction(if (threadIdIsRss(thread.id)) "Remove feed" else "Archive", onArchive) }
                if (!threadIdIsRss(thread.id)) {
                    item { DialogAction("Delete", onDelete) }
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
private fun MailList(
    threads: List<ThreadSummary>,
    onOpen: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onArchive: (ThreadSummary) -> Unit,
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
                MailRow(thread = thread, onOpen = { onOpen(thread) }, onToggleStar = { onToggleStar(thread) })
            }
            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f))
        }
    }
}

@Composable
private fun MailRow(thread: ThreadSummary, onOpen: () -> Unit, onToggleStar: () -> Unit) {
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
        Avatar(senderLabel)
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ThreadScreen(
    thread: ThreadSummary?,
    messages: List<MessageBody>,
    accountEmail: String,
    onBack: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    onToggleStar: () -> Unit,
    quickReplyBody: String,
    onQuickReplyChange: (String) -> Unit,
    onSendReply: () -> Unit,
) {
    val isRss = thread?.let { threadIdIsRss(it.id) } ?: false
    val chat = LocalChatColors.current
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
                        IconButton(onClick = onDelete) {
                            Icon(Icons.Filled.Delete, contentDescription = "Delete")
                        }
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .padding(innerPadding),
        ) {
            if (messages.isEmpty()) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else {
                LazyColumn(
                    Modifier.weight(1f),
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    items(messages, key = { it.id }) { message ->
                        val outgoing = isOutgoing(message, accountEmail)
                        MessageBubble(message = message, outgoing = outgoing, chat = chat)
                    }
                }
            }
            if (thread != null && !isRss && messages.isNotEmpty()) {
                ReplyBar(quickReplyBody, onQuickReplyChange, onSendReply)
            }
        }
    }
}

@Composable
private fun MessageBubble(message: MessageBody, outgoing: Boolean, chat: ChatColors) {
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
            // Subject is the conversation title (top bar); the bubble shows the
            // message body, matching the desktop chat reader.
            Text(
                message.body.ifBlank { "(no content)" },
                color = if (message.body.isBlank()) textColor.copy(alpha = 0.6f) else textColor,
            )
            Text(
                formatRelativeTime(message.dateEpochSeconds),
                fontSize = 10.5.sp,
                color = textColor.copy(alpha = 0.6f),
                modifier = Modifier.align(Alignment.End),
            )
        }
    }
}

@Composable
private fun ReplyBar(value: String, onChange: (String) -> Unit, onSend: () -> Unit) {
    Surface(tonalElevation = 3.dp, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Row(
            Modifier.fillMaxWidth().padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(Icons.AutoMirrored.Filled.Reply, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            OutlinedTextField(
                value = value,
                onValueChange = onChange,
                placeholder = { Text("Reply") },
                modifier = Modifier.weight(1f),
                maxLines = 4,
            )
            IconButton(onClick = onSend, enabled = value.isNotBlank()) {
                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send reply", tint = MaterialTheme.colorScheme.primary)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ComposeScreen(
    to: String, onToChange: (String) -> Unit,
    cc: String, onCcChange: (String) -> Unit,
    bcc: String, onBccChange: (String) -> Unit,
    subject: String, onSubjectChange: (String) -> Unit,
    body: String, onBodyChange: (String) -> Unit,
    attachments: List<DraftAttachment>,
    onAttach: () -> Unit,
    onClearAttachments: () -> Unit,
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
            ComposeField(to, onToChange, "To")
            ComposeField(cc, onCcChange, "Cc")
            ComposeField(bcc, onBccChange, "Bcc")
            ComposeField(subject, onSubjectChange, "Subject")
            OutlinedTextField(
                value = body,
                onValueChange = onBodyChange,
                label = { Text("Message") },
                modifier = Modifier.fillMaxWidth().weight(1f),
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
private fun ComposeField(value: String, onChange: (String) -> Unit, label: String) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddAccountScreen(
    onBack: () -> Unit,
    initialSection: Int,
    displayName: String, onDisplayNameChange: (String) -> Unit,
    senderName: String, onSenderNameChange: (String) -> Unit,
    email: String, onEmailChange: (String) -> Unit,
    password: String, onPasswordChange: (String) -> Unit,
    host: String, onHostChange: (String) -> Unit,
    imapPort: String, onImapPortChange: (String) -> Unit,
    smtpHost: String, onSmtpHostChange: (String) -> Unit,
    smtpPort: String, onSmtpPortChange: (String) -> Unit,
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
                        SetupField(password, onPasswordChange, "Password", isPassword = true)
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
    onSelectUnified: () -> Unit,
    onSelectAccount: (AccountSummary) -> Unit,
    onSelectKanban: () -> Unit,
    onAddAccount: () -> Unit,
    onRefreshBackground: () -> Unit,
    onEnableNotifications: () -> Unit,
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
                item {
                    val unread = folders.sumOf { it.unread }
                    SidebarRow(
                        selected = selectedAccountId == UNIFIED_ACCOUNT_ID,
                        chat = chat,
                        onClick = onSelectUnified,
                        leading = { Icon(Icons.Filled.Inbox, contentDescription = null, modifier = Modifier.size(20.dp)) },
                        title = "Unified inbox",
                        subtitle = "All accounts",
                        trailing = unread.takeIf { it > 0 }?.toString(),
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
                        trailing = if (account.needsReconnect) "!" else unread.takeIf { it > 0 }?.toString(),
                    )
                }
            }
            if (accounts.isNotEmpty()) {
                item { DrawerLabel("Views", chat) }
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
        }
    }
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

private fun FilterMode.label(): String = when (this) {
    FilterMode.All -> "All"
    FilterMode.Unread -> "Unread"
    FilterMode.Starred -> "Starred"
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
                    val colArray = obj.optJSONArray("columns") ?: JSONArray()
                    val columns = buildList {
                        for (j in 0 until colArray.length()) {
                            val col = colArray.optJSONObject(j) ?: continue
                            val accountId = col.optString("accountId")
                            val folderId = col.optString("folderId")
                            if (accountId.isNotBlank() && folderId.isNotBlank()) add(KanbanColumnSpec(accountId, folderId))
                        }
                    }.distinctBy(::kanbanColumnKey)
                    add(KanbanBoardSpec(id, name, columns))
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
        array.put(JSONObject().put("id", board.id).put("name", board.name).put("columns", columns))
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
