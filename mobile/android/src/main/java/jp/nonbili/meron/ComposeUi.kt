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
internal fun ComposeScreen(
    sendIdentities: List<SendIdentity>,
    selectedFromKey: String,
    onFromChange: (String) -> Unit,
    to: String,
    onToChange: (String) -> Unit,
    cc: String,
    onCcChange: (String) -> Unit,
    bcc: String,
    onBccChange: (String) -> Unit,
    subject: String,
    onSubjectChange: (String) -> Unit,
    body: String,
    onBodyChange: (String) -> Unit,
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
    var confirmDiscard by remember { mutableStateOf(false) }
    val hasContent =
        to.isNotBlank() || cc.isNotBlank() || bcc.isNotBlank() ||
            subject.isNotBlank() || body.isNotBlank() || attachments.isNotEmpty()

    if (confirmDiscard) {
        AlertDialog(
            onDismissRequest = { confirmDiscard = false },
            title = { Text("Discard draft?") },
            text = { Text("This message will be deleted and won't be saved.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmDiscard = false
                    onDiscardDraft()
                }) {
                    Text("Discard", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDiscard = false }) { Text("Keep editing") }
            },
        )
    }

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
                    IconButton(onClick = { if (hasContent) confirmDiscard = true else onDiscardDraft() }) {
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
                modifier =
                    Modifier
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
internal fun FromIdentitySelector(
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
internal fun ComposeField(
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
            modifier =
                Modifier
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
internal fun AddAccountScreen(
    onBack: () -> Unit,
    initialSection: Int,
    displayName: String,
    onDisplayNameChange: (String) -> Unit,
    senderName: String,
    onSenderNameChange: (String) -> Unit,
    email: String,
    onEmailChange: (String) -> Unit,
    username: String,
    onUsernameChange: (String) -> Unit,
    password: String,
    onPasswordChange: (String) -> Unit,
    host: String,
    onHostChange: (String) -> Unit,
    imapPort: String,
    onImapPortChange: (String) -> Unit,
    smtpHost: String,
    onSmtpHostChange: (String) -> Unit,
    smtpPort: String,
    onSmtpPortChange: (String) -> Unit,
    onAutodiscover: () -> Unit,
    onAddPassword: () -> Unit,
    oauthProvider: String,
    onOauthProviderChange: (String) -> Unit,
    oauthEmail: String,
    onOauthEmailChange: (String) -> Unit,
    oauthClientId: String,
    onOauthClientIdChange: (String) -> Unit,
    oauthClientSecret: String,
    onOauthClientSecretChange: (String) -> Unit,
    oauthRedirectUri: String,
    onOauthRedirectUriChange: (String) -> Unit,
    oauthAuthorizationCode: String,
    oauthAccessToken: String,
    onOauthAccessTokenChange: (String) -> Unit,
    oauthRefreshToken: String,
    onOauthRefreshTokenChange: (String) -> Unit,
    oauthExpiresAt: String,
    onOauthExpiresAtChange: (String) -> Unit,
    onLaunchOAuth: () -> Unit,
    onExchangeOAuth: () -> Unit,
    onAddOAuth: () -> Unit,
    rssFeedUrl: String,
    onRssFeedUrlChange: (String) -> Unit,
    rssDisplayName: String,
    onRssDisplayNameChange: (String) -> Unit,
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
                0 -> {
                    item {
                        SetupCard(title = "IMAP / SMTP account") {
                            SetupField(displayName, onDisplayNameChange, "Display name", placeholder = "Work")
                            SetupField(senderName, onSenderNameChange, "Sender name", placeholder = "Jane Doe")
                            SetupField(email, onEmailChange, "Email", placeholder = "you@example.com")
                            SetupField(username, onUsernameChange, "Username", placeholder = "you@example.com")
                            SetupField(password, onPasswordChange, "Password", isPassword = true)
                            OutlinedButton(onClick = onAutodiscover, modifier = Modifier.fillMaxWidth()) {
                                Text("Find mail settings")
                            }
                            SetupField(host, onHostChange, "IMAP host", placeholder = "imap.example.com")
                            SetupField(imapPort, onImapPortChange, "IMAP port", placeholder = "993")
                            SetupField(smtpHost, onSmtpHostChange, "SMTP host", placeholder = "smtp.example.com")
                            SetupField(smtpPort, onSmtpPortChange, "SMTP port", placeholder = "465")
                            Button(onClick = onAddPassword, modifier = Modifier.fillMaxWidth()) { Text("Add account") }
                        }
                    }
                }

                1 -> {
                    item {
                        SetupCard(title = "Gmail / Outlook") {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                FilterChip(oauthProvider == "gmail", { onOauthProviderChange("gmail") }, { Text("Gmail") })
                                FilterChip(oauthProvider == "outlook", { onOauthProviderChange("outlook") }, { Text("Outlook") })
                            }
                            SetupField(oauthEmail, onOauthEmailChange, "Email", placeholder = "you@gmail.com")
                            SetupField(oauthClientId, onOauthClientIdChange, "Client ID")
                            SetupField(oauthClientSecret, onOauthClientSecretChange, "Client secret (optional)")
                            SetupField(oauthRedirectUri, onOauthRedirectUriChange, "Redirect URI")
                            Button(onClick = onLaunchOAuth, modifier = Modifier.fillMaxWidth()) { Text("Sign in with browser") }
                            if (oauthAuthorizationCode.isNotBlank()) {
                                Text(
                                    "Authorization code received.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.primary,
                                )
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
                }

                else -> {
                    item {
                        SetupCard(title = "RSS feed") {
                            SetupField(rssFeedUrl, onRssFeedUrlChange, "Feed URL", placeholder = "https://example.com/feed.xml")
                            SetupField(rssDisplayName, onRssDisplayNameChange, "Feed name", placeholder = "Example Feed")
                            Button(onClick = onAddRss, modifier = Modifier.fillMaxWidth()) { Text("Add feed") }
                        }
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
internal fun SetupCard(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            content()
        }
    }
}

@Composable
internal fun SetupField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    isPassword: Boolean = false,
    placeholder: String? = null,
) {
    var revealed by remember { mutableStateOf(false) }
    OutlinedTextField(
        value,
        onChange,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        visualTransformation = if (isPassword && !revealed) PasswordVisualTransformation() else VisualTransformation.None,
        trailingIcon =
            if (isPassword) {
                {
                    IconButton(onClick = { revealed = !revealed }) {
                        Icon(
                            if (revealed) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                            contentDescription = if (revealed) "Hide password" else "Show password",
                        )
                    }
                }
            } else {
                null
            },
    )
}
