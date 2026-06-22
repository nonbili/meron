package jp.nonbili.meron

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Base64
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.setContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
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
import java.util.UUID

class ComposeMainActivity : ComponentActivity() {
    private var incomingMailtoDraft by mutableStateOf<ComposeDraft?>(null)
    private var incomingOAuthCallbackUrl by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
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
            MaterialTheme {
                Surface(Modifier.fillMaxSize()) {
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

@Composable
private fun MeronMobileScreen(
    coreInitJson: String,
    incomingMailtoDraft: ComposeDraft?,
    incomingOAuthCallbackUrl: String?,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
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
    var coreAccounts by remember { mutableStateOf(emptyList<jp.nonbili.meron.shared.AccountSummary>()) }
    var selectedCoreAccountId by remember { mutableStateOf("") }
    var coreFolders by remember { mutableStateOf(emptyList<FolderSummary>()) }
    var selectedCoreFolder by remember { mutableStateOf("inbox") }
    var coreThreads by remember { mutableStateOf(emptyList<ThreadSummary>()) }
    var selectedCoreThread by remember { mutableStateOf<ThreadSummary?>(null) }
    var to by remember { mutableStateOf("user1@mail.localhost") }
    var subject by remember { mutableStateOf("Hello from Compose Rust core Android") }
    var body by remember { mutableStateOf("This message was sent from the Compose Android shell through shared KMP state and meron-core.") }
    var quickReplyBody by remember { mutableStateOf("") }
    var status by remember { mutableStateOf("Starting...") }
    var messages by remember { mutableStateOf(emptyList<MessageBody>()) }
    var selected by remember { mutableStateOf<MessageBody?>(null) }
    var attachments by remember { mutableStateOf(emptyList<DraftAttachment>()) }
    var cc by remember { mutableStateOf("") }
    var bcc by remember { mutableStateOf("") }
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

    LaunchedEffect(incomingMailtoDraft) {
        incomingMailtoDraft?.let { draft ->
            to = draft.to
            cc = draft.cc
            bcc = draft.bcc
            subject = draft.subject
            body = draft.body
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
                    status = "OAuth authorization code received; use Exchange code and add account."
                }
            }.onFailure {
                status = "OAuth callback failed: ${it.message}"
            }
        }
    }

    fun listAccounts() {
        if (!MeronCoreNative.isLoaded()) {
            accountJson = "Rust core not packaged."
            return
        }
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).listAccounts()
                }
            }.onSuccess {
                accountJson = it
                val parsedAccounts = parseAccountListResponse(it)
                coreAccounts = parsedAccounts
                selectedCoreAccountId = selectedCoreAccountId.takeIf { selected ->
                    parsedAccounts.any { account -> account.id == selected }
                } ?: parsedAccounts.firstOrNull()?.id.orEmpty()
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
        val parsedImapPort = imapPort.trim().toIntOrNull() ?: 993
        val parsedSmtpPort = smtpPort.trim().toIntOrNull() ?: 465
        val params = AddPasswordAccountParams(
            email = email.trim(),
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            imapHost = host.trim(),
            imapPort = parsedImapPort,
            smtpHost = smtpHost.trim(),
            smtpPort = parsedSmtpPort,
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
                accountJson = it
                val parsedAccounts = parseAccountListResponse(it)
                coreAccounts = parsedAccounts
                selectedCoreAccountId = selectedCoreAccountId.takeIf { selected ->
                    parsedAccounts.any { account -> account.id == selected }
                } ?: parsedAccounts.firstOrNull()?.id.orEmpty()
                status = "Added password account"
            }.onFailure {
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
                    client.addRssAccount(
                        AddRssAccountParams(
                            feedUrl = rssFeedUrl.trim(),
                            displayName = rssDisplayName.trim(),
                        ),
                    )
                    client.listAccounts()
                }
            }.onSuccess {
                accountJson = it
                val parsedAccounts = parseAccountListResponse(it)
                coreAccounts = parsedAccounts
                selectedCoreAccountId = selectedCoreAccountId.takeIf { selected ->
                    parsedAccounts.any { account -> account.id == selected }
                } ?: parsedAccounts.firstOrNull()?.id.orEmpty()
                status = "Added RSS account"
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
        status = "Adding ${oauthProvider.replaceFirstChar { it.uppercase() }} OAuth account..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.addOAuthAccount(params)
                    client.listAccounts()
                }
            }.onSuccess {
                accountJson = it
                val parsedAccounts = parseAccountListResponse(it)
                coreAccounts = parsedAccounts
                selectedCoreAccountId = parsedAccounts.firstOrNull { account -> account.email == params.email }?.id
                    ?: selectedCoreAccountId.takeIf { selected ->
                        parsedAccounts.any { account -> account.id == selected }
                    }
                    ?: parsedAccounts.firstOrNull()?.id.orEmpty()
                status = "Added ${oauthProvider.replaceFirstChar { it.uppercase() }} OAuth account"
            }.onFailure {
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
                accountJson = it
                val parsedAccounts = parseAccountListResponse(it)
                coreAccounts = parsedAccounts
                selectedCoreAccountId = parsedAccounts.firstOrNull { account -> account.email == params.email }?.id
                    ?: selectedCoreAccountId.takeIf { selected ->
                        parsedAccounts.any { account -> account.id == selected }
                    }
                    ?: parsedAccounts.firstOrNull()?.id.orEmpty()
                status = "OAuth account added"
            }.onFailure {
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
            status = "Opened ${oauthProvider.replaceFirstChar { it.uppercase() }} OAuth in browser"
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
        status = "Syncing $accountId / $requestedFolder..."
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
                    val folder = requestedFolder.takeIf { selected -> folders.any { it.name == selected } }
                        ?: folders.firstOrNull()?.name
                        ?: requestedFolder
                    val threadsJson = client.listThreads(ThreadListParams(accountId = accountId, folderId = folder))
                    Triple(foldersJson, folder, threadsJson)
                }
            }.onSuccess { (foldersJson, folder, threadsJson) ->
                val parsedFolders = parseFolderListResponse(foldersJson)
                val parsedThreads = parseThreadListResponse(threadsJson)
                coreFolders = parsedFolders
                selectedCoreFolder = folder
                coreThreads = parsedThreads
                if (selectedCoreThread?.id !in parsedThreads.map { thread -> thread.id }) {
                    selectedCoreThread = null
                    messages = emptyList()
                    selected = null
                }
                status = "Loaded ${parsedThreads.size} core thread(s) from $folder"
            }.onFailure {
                status = "Core sync failed: ${it.message}"
            }
        }
    }

    fun readCoreThread(thread: ThreadSummary) {
        if (!MeronCoreNative.isLoaded()) {
            status = "Rust core not packaged."
            return
        }
        selectedCoreThread = thread
        status = "Opening ${thread.subject.ifBlank { thread.id }}..."
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
                val parsedMessages = parseThreadReadResponse(it)
                messages = parsedMessages
                selected = parsedMessages.firstOrNull()
                status = "Loaded ${parsedMessages.size} message(s)"
            }.onFailure {
                status = "Thread read failed: ${it.message}"
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
        status = "$label..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).action()
                }
            }.onSuccess {
                coreThreads = update(coreThreads)
                status = "$label complete"
            }.onFailure {
                status = "$label failed for ${thread.subject.ifBlank { thread.id }}: ${it.message}"
            }
        }
    }

    fun sendMail() {
        val accountId = selectedCoreAccountId.ifBlank { coreAccounts.firstOrNull()?.id.orEmpty() }
        if (accountId.isBlank()) {
            status = "Select or add an account before sending."
            return
        }
        val draft = ComposeDraft(
            to = to.trim(),
            cc = cc.trim(),
            bcc = bcc.trim(),
            subject = subject.trim(),
            body = body.trim(),
            attachments = attachments,
        )
        if (!draft.canSend) {
            status = "Complete To, Subject, and Body before sending."
            return
        }
        status = "Sending through core..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    val client = MobileMailCommandClient(JniMeronCore())
                    client.send(draft.toSendMailParams(accountId = accountId))
                    client.listThreads(ThreadListParams(accountId = accountId, folderId = selectedCoreFolder.ifBlank { "inbox" }))
                }
            }.onSuccess {
                val parsedThreads = parseThreadListResponse(it)
                coreThreads = parsedThreads
                attachments = emptyList()
                status = "Sent through core; loaded ${parsedThreads.size} thread(s)"
            }.onFailure {
                status = "Core send failed: ${it.message}"
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
            status = "RSS threads do not support replies."
            return
        }
        if (replyBody.isBlank()) {
            status = "Write a reply before sending."
            return
        }
        status = "Sending quick reply..."
        scope.launch {
            runCatching {
                withContext(Dispatchers.IO) {
                    MobileMailCommandClient(JniMeronCore()).send(parent.toReplyMailParams(accountId, replyBody))
                }
            }.onSuccess {
                quickReplyBody = ""
                status = "Quick reply sent"
            }.onFailure {
                status = "Quick reply failed: ${it.message}"
            }
        }
    }

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Text("Meron", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
            Text("Compose Android shell backed by shared KMP state and Rust core IMAP/SMTP.")
            Spacer(Modifier.height(8.dp))
            Text(coreStatus(coreInitJson), style = MaterialTheme.typography.bodySmall)
        }
        item {
            Text("Background refresh", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = {
                    AndroidBackgroundSyncScheduler.runOnce(context)
                    status = "Queued background refresh"
                }) {
                    Text("Refresh in background")
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted) {
                    Button(onClick = {
                        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }) {
                        Text("Enable notifications")
                    }
                }
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !notificationPermissionGranted) {
                Text("Notifications need permission before refresh results can be shown.")
            }
        }
        item {
            Text("Accounts", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            OutlinedTextField(displayName, { displayName = it }, label = { Text("Display name") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(senderName, { senderName = it }, label = { Text("Sender name") }, modifier = Modifier.fillMaxWidth())
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(host, { host = it }, label = { Text("IMAP host") }, modifier = Modifier.weight(1f))
                OutlinedTextField(imapPort, { imapPort = it }, label = { Text("IMAP port") }, modifier = Modifier.weight(1f))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(smtpHost, { smtpHost = it }, label = { Text("SMTP host") }, modifier = Modifier.weight(1f))
                OutlinedTextField(smtpPort, { smtpPort = it }, label = { Text("SMTP port") }, modifier = Modifier.weight(1f))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = ::addPasswordAccount) {
                    Text("Add password account")
                }
                Button(onClick = ::listAccounts) {
                    Text("List accounts")
                }
            }
            Spacer(Modifier.height(8.dp))
            Text("OAuth", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { oauthProvider = "gmail" }) {
                    Text(if (oauthProvider == "gmail") "Gmail selected" else "Gmail")
                }
                Button(onClick = { oauthProvider = "outlook" }) {
                    Text(if (oauthProvider == "outlook") "Outlook selected" else "Outlook")
                }
            }
            OutlinedTextField(oauthEmail, { oauthEmail = it }, label = { Text("OAuth email") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(oauthClientId, { oauthClientId = it }, label = { Text("OAuth client ID") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(oauthClientSecret, { oauthClientSecret = it }, label = { Text("OAuth client secret (optional)") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(oauthRedirectUri, { oauthRedirectUri = it }, label = { Text("Redirect URI") }, modifier = Modifier.fillMaxWidth())
            Button(onClick = ::launchOAuthFlow, modifier = Modifier.fillMaxWidth()) {
                Text("Open OAuth in browser")
            }
            if (oauthAuthorizationCode.isNotBlank()) {
                Text("Authorization code: $oauthAuthorizationCode", style = MaterialTheme.typography.bodySmall)
            }
            Button(onClick = ::exchangeOAuthCode, modifier = Modifier.fillMaxWidth()) {
                Text("Exchange code and add account")
            }
            OutlinedTextField(oauthAccessToken, { oauthAccessToken = it }, label = { Text("Access token") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(oauthRefreshToken, { oauthRefreshToken = it }, label = { Text("Refresh token") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(oauthExpiresAt, { oauthExpiresAt = it }, label = { Text("Token expires at") }, modifier = Modifier.fillMaxWidth())
            Button(onClick = ::addOAuthAccount, modifier = Modifier.fillMaxWidth()) {
                Text("Add OAuth account")
            }
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(rssFeedUrl, { rssFeedUrl = it }, label = { Text("RSS feed URL") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(rssDisplayName, { rssDisplayName = it }, label = { Text("RSS name") }, modifier = Modifier.fillMaxWidth())
            Button(onClick = ::addRssAccount) {
                Text("Add RSS account")
            }
            if (coreAccounts.isNotEmpty()) {
                Text("Selected account", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                coreAccounts.forEach { account ->
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = {
                            if (selectedCoreAccountId != account.id) {
                                selectedCoreAccountId = account.id
                                selectedCoreFolder = "inbox"
                                coreFolders = emptyList()
                                coreThreads = emptyList()
                                selectedCoreThread = null
                                messages = emptyList()
                                selected = null
                            }
                        }) {
                            Text(if (selectedCoreAccountId == account.id) "Selected" else "Select")
                        }
                        Text(
                            account.displayName.ifBlank { account.email.ifBlank { account.id } },
                            modifier = Modifier.weight(1f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                if (coreFolders.isNotEmpty()) {
                    Text("Selected folder", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                    coreFolders.forEach { folder ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(onClick = {
                                selectedCoreFolder = folder.name
                                selectedCoreThread = null
                                messages = emptyList()
                                selected = null
                            }) {
                                Text(if (selectedCoreFolder == folder.name) "Selected" else "Select")
                            }
                            Text(
                                if (folder.unread > 0) "${folder.name} (${folder.unread})" else folder.name,
                                modifier = Modifier.weight(1f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
                Button(onClick = ::syncCoreThreads, modifier = Modifier.fillMaxWidth()) {
                    Text("Sync selected account/folder")
                }
            }
            if (accountJson.isNotBlank()) {
                Text(accountJson, style = MaterialTheme.typography.bodySmall)
            }
        }
        item {
            Text("Core Threads", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            if (coreThreads.isEmpty()) {
                Text("Sync a selected account to load cached core threads.")
            }
        }
        items(coreThreads, key = { it.id }) { thread ->
            val isRssThread = threadIdIsRss(thread.id)
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { readCoreThread(thread) },
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
            ) {
                Column(Modifier.padding(12.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(thread.sender.ifBlank { thread.accountId }, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (thread.unread) Text("Unread")
                        if (thread.starred) Text("Starred")
                    }
                    Text(thread.subject.ifBlank { "(no subject)" }, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold)
                    if (thread.preview.isNotBlank()) {
                        Text(thread.preview, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
                    }
                    Text(thread.folder, style = MaterialTheme.typography.bodySmall)
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = {
                            runCoreThreadAction(
                                thread = thread,
                                label = if (thread.unread) "Mark read" else "Mark unread",
                                action = {
                                    if (isRssThread) {
                                        markRssRead(RssMarkReadParams(threadId = thread.id, seen = thread.unread))
                                    } else {
                                        markRead(MarkReadParams(threadId = thread.id, seen = thread.unread))
                                    }
                                },
                                update = { threads ->
                                    threads.map { if (it.id == thread.id) it.copy(unread = !thread.unread) else it }
                                },
                            )
                        }) {
                            Text(if (thread.unread) "Read" else "Unread")
                        }
                        Button(onClick = {
                            runCoreThreadAction(
                                thread = thread,
                                label = if (thread.starred) "Unstar" else "Star",
                                action = {
                                    if (isRssThread) {
                                        markRssStarred(RssMarkStarredParams(threadId = thread.id, starred = !thread.starred))
                                    } else {
                                        markStarred(MarkStarredParams(threadId = thread.id, starred = !thread.starred))
                                    }
                                },
                                update = { threads ->
                                    threads.map { if (it.id == thread.id) it.copy(starred = !thread.starred) else it }
                                },
                            )
                        }) {
                            Text(if (thread.starred) "Unstar" else "Star")
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (isRssThread) {
                            Button(onClick = {
                                runCoreThreadAction(
                                    thread = thread,
                                    label = "Remove feed",
                                    action = { removeRssFeed(RemoveRssFeedParams(threadId = thread.id)) },
                                    update = { threads -> threads.filterNot { it.id == thread.id } },
                                )
                            }) {
                                Text("Remove Feed")
                            }
                        } else {
                            Button(onClick = {
                                runCoreThreadAction(
                                    thread = thread,
                                    label = "Archive",
                                    action = { archive(ThreadActionParams(threadId = thread.id)) },
                                    update = { threads -> threads.filterNot { it.id == thread.id } },
                                )
                            }) {
                                Text("Archive")
                            }
                            Button(onClick = {
                                runCoreThreadAction(
                                    thread = thread,
                                    label = "Delete",
                                    action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
                                    update = { threads -> threads.filterNot { it.id == thread.id } },
                                )
                            }) {
                                Text("Delete")
                            }
                        }
                    }
                }
            }
        }
        item {
            Text(status, style = MaterialTheme.typography.bodyMedium)
        }
        item {
            Text("Messages", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            selectedCoreThread?.let { thread ->
                Text(thread.subject.ifBlank { thread.id }, style = MaterialTheme.typography.bodySmall)
            }
        }
        items(messages, key = { it.id }) { message ->
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { selected = message },
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
            ) {
                Column(Modifier.padding(12.dp)) {
                    Text(message.subject, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold)
                    Text(message.from, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
                }
            }
        }
        item {
            Text("Message", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(12.dp)) {
                    val message = selected
                    if (message == null) {
                        Text("Select a message.")
                    } else {
                        Text("From: ${message.from}")
                        Text("To: ${message.to}")
                        Text("Date: ${message.dateEpochSeconds}")
                        Spacer(Modifier.height(8.dp))
                        Text(message.subject, fontWeight = FontWeight.SemiBold)
                        Spacer(Modifier.height(8.dp))
                        Text(message.body)
                    }
                }
            }
            val replyThread = selectedCoreThread
            if (replyThread != null && !threadIdIsRss(replyThread.id) && messages.isNotEmpty()) {
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    quickReplyBody,
                    { quickReplyBody = it },
                    label = { Text("Quick reply") },
                    minLines = 3,
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(onClick = ::sendQuickReply, modifier = Modifier.fillMaxWidth()) {
                    Text("Send quick reply")
                }
            }
        }
        item {
            Text("Compose", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            OutlinedTextField(to, { to = it }, label = { Text("To") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(cc, { cc = it }, label = { Text("Cc") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(bcc, { bcc = it }, label = { Text("Bcc") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(subject, { subject = it }, label = { Text("Subject") }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(body, { body = it }, label = { Text("Body") }, minLines = 4, modifier = Modifier.fillMaxWidth())
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { attachmentPicker.launch(arrayOf("*/*")) }) {
                    Text("Attach file")
                }
                if (attachments.isNotEmpty()) {
                    Button(onClick = { attachments = emptyList() }) {
                        Text("Clear")
                    }
                }
            }
            attachments.forEach { attachment ->
                Text(
                    "${attachment.displayName} (${attachment.sizeBytes} bytes)",
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Button(onClick = ::sendMail, modifier = Modifier.fillMaxWidth()) {
                Text("Send through core")
            }
        }
    }
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
        "Rust core loaded: protocol ${MeronCoreNative.protocolVersion()}\nShared protocol: ${SharedMobileContract.protocolVersion}\nInit: $coreInitJson\nPing: ${MeronCoreNative.pingJson()}"
    } else {
        "Rust core not packaged yet; using Java fallback."
    }
}

private fun String.pkceChallenge(): String {
    val digest = MessageDigest.getInstance("SHA-256").digest(toByteArray())
    return Base64.encodeToString(digest, Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)
}
