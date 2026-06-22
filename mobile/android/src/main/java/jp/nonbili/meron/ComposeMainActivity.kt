package jp.nonbili.meron

import android.Manifest
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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
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
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
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

private enum class Screen { Mail, Thread, Compose, AddAccount }

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
    var selectedCoreAccountId by remember { mutableStateOf("") }
    var coreFolders by remember { mutableStateOf(emptyList<FolderSummary>()) }
    var selectedCoreFolder by remember { mutableStateOf("inbox") }
    var coreThreads by remember { mutableStateOf(emptyList<ThreadSummary>()) }
    var selectedCoreThread by remember { mutableStateOf<ThreadSummary?>(null) }
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
            ?: selectedCoreAccountId.takeIf { sel -> parsed.any { it.id == sel } }
            ?: parsed.firstOrNull()?.id.orEmpty()
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

    fun syncCoreThreads() {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        val accountId = selectedCoreAccountId.ifBlank { coreAccounts.firstOrNull()?.id.orEmpty() }
        val isRssAccount = coreAccounts.firstOrNull { it.id == accountId }?.let(::accountSummaryIsRss) ?: false
        val requestedFolder = selectedCoreFolder.ifBlank { "inbox" }
        if (accountId.isBlank()) {
            status = "No account selected."
            return
        }
        syncing = true
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    if (isRssAccount) {
                        client.syncRss(SyncRssParams(accountId = accountId))
                    } else {
                        client.sync(SyncMailParams(accountId = accountId, folderId = requestedFolder, limit = 50, folders = true))
                    }
                    val foldersJson = client.listFolders(FolderListParams(accountId = accountId))
                    val folders = parseFolderListResponse(foldersJson)
                    // Server folder names are case-sensitive ("INBOX"), but the
                    // default request uses "inbox"; match case-insensitively and
                    // fall back to a real inbox before the first folder.
                    val folder = folders.firstOrNull { it.name.equals(requestedFolder, ignoreCase = true) }?.name
                        ?: folders.firstOrNull { it.name.equals("inbox", ignoreCase = true) }?.name
                        ?: folders.firstOrNull()?.name
                        ?: requestedFolder
                    val threadsJson = client.listThreads(ThreadListParams(accountId = accountId, folderId = folder))
                    Triple(foldersJson, folder, threadsJson)
                }
            }.onSuccess { (foldersJson, folder, threadsJson) ->
                coreFolders = parseFolderListResponse(foldersJson)
                selectedCoreFolder = folder
                val parsedThreads = parseThreadListResponse(threadsJson)
                coreThreads = parsedThreads
                if (selectedCoreThread?.id !in parsedThreads.map { it.id }) {
                    selectedCoreThread = null
                    messages = emptyList()
                }
                syncing = false
                errorBanner = null
                status = "${parsedThreads.size} message(s) in ${folder.replaceFirstChar { it.uppercase() }}"
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

    fun sendMail() {
        val accountId = selectedCoreAccountId.ifBlank { coreAccounts.firstOrNull()?.id.orEmpty() }
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
                    client.listThreads(ThreadListParams(accountId = accountId, folderId = selectedCoreFolder.ifBlank { "inbox" }))
                }
            }.onSuccess {
                coreThreads = parseThreadListResponse(it)
                attachments = emptyList()
                to = ""; cc = ""; bcc = ""; subject = ""; body = ""
                screen = Screen.Mail
                errorBanner = null
                status = "Message sent"
            }.onFailure {
                errorBanner = it.message ?: "Send failed"
                status = "Send failed: ${it.message}"
            }
        }
    }

    fun sendQuickReply() {
        val accountId = selectedCoreAccountId.ifBlank { coreAccounts.firstOrNull()?.id.orEmpty() }
        val thread = selectedCoreThread
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
        screen = Screen.AddAccount
    }

    // Load persisted accounts once on startup so they survive app restarts.
    LaunchedEffect(Unit) {
        if (MeronCoreNative.isLoaded() && coreAccounts.isEmpty()) {
            listAccounts()
        }
    }

    val selectedAccount = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }

    // Hardware back from a sub-screen returns to the inbox instead of exiting.
    BackHandler(enabled = screen != Screen.Mail) {
        screen = Screen.Mail
    }

    when (screen) {
        Screen.Thread -> ThreadScreen(
            thread = selectedCoreThread,
            messages = messages,
            accountEmail = selectedAccount?.email.orEmpty(),
            onBack = { screen = Screen.Mail },
            onArchive = { selectedCoreThread?.let { archiveOrRemove(it); screen = Screen.Mail } },
            onDelete = { selectedCoreThread?.let { deleteThread(it); screen = Screen.Mail } },
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
            onBack = { screen = Screen.Mail },
        )

        Screen.AddAccount -> AddAccountScreen(
            onBack = { screen = Screen.Mail },
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

        Screen.Mail -> ModalNavigationDrawer(
            drawerState = drawerState,
            drawerContent = {
                MailDrawer(
                    accounts = coreAccounts,
                    selectedAccountId = selectedCoreAccountId,
                    folders = coreFolders,
                    selectedFolder = selectedCoreFolder,
                    notificationsNeedPermission = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted,
                    onSelectAccount = { account ->
                        if (selectedCoreAccountId != account.id) {
                            selectedCoreAccountId = account.id
                            selectedCoreFolder = "inbox"
                            coreFolders = emptyList()
                            coreThreads = emptyList()
                            selectedCoreThread = null
                            messages = emptyList()
                            syncCoreThreads()
                        }
                        scope.launch { drawerState.close() }
                    },
                    onSelectFolder = { folder ->
                        selectedCoreFolder = folder.name
                        selectedCoreThread = null
                        messages = emptyList()
                        syncCoreThreads()
                        scope.launch { drawerState.close() }
                    },
                    onAddAccount = {
                        addSection = 0
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
                                Text(selectedCoreFolder.replaceFirstChar { it.uppercase() }, fontWeight = FontWeight.SemiBold)
                                selectedAccount?.let {
                                    Text(
                                        it.email.ifBlank { it.displayName },
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
    selectedFolder: String,
    notificationsNeedPermission: Boolean,
    onSelectAccount: (AccountSummary) -> Unit,
    onSelectFolder: (FolderSummary) -> Unit,
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
                item { DrawerLabel("Accounts", chat) }
                items(accounts, key = { it.id }) { account ->
                    val label = account.displayName.ifBlank { account.email.ifBlank { account.id } }
                    SidebarRow(
                        selected = account.id == selectedAccountId,
                        chat = chat,
                        onClick = { onSelectAccount(account) },
                        leading = {
                            Avatar(label, size = 32.dp)
                        },
                        title = label,
                        subtitle = account.email.takeIf { it.isNotBlank() && it != label },
                        trailing = if (account.needsReconnect) "!" else null,
                    )
                }
            }
            if (folders.isNotEmpty()) {
                item { DrawerLabel("Folders", chat) }
                items(folders, key = { it.name }) { folder ->
                    SidebarRow(
                        selected = folder.name == selectedFolder,
                        chat = chat,
                        onClick = { onSelectFolder(folder) },
                        leading = { Icon(folderIcon(folder.name), contentDescription = null, modifier = Modifier.size(20.dp)) },
                        title = folder.name.replaceFirstChar { it.uppercase() },
                        subtitle = null,
                        trailing = folder.unread.takeIf { it > 0 }?.toString(),
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
