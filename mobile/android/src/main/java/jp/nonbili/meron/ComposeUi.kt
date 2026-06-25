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
    var overflowOpen by remember { mutableStateOf(false) }
    var showCcBcc by remember { mutableStateOf(cc.isNotBlank() || bcc.isNotBlank()) }
    val hasContent =
        to.isNotBlank() || cc.isNotBlank() || bcc.isNotBlank() ||
            subject.isNotBlank() || body.isNotBlank() || attachments.isNotEmpty()

    if (confirmDiscard) {
        AlertDialog(
            onDismissRequest = { confirmDiscard = false },
            title = { Text(stringResource(R.string.mobile_compose_discard_draft_title)) },
            text = { Text(stringResource(R.string.mobile_compose_discard_draft_text)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDiscard = false
                    onDiscardDraft()
                }) {
                    Text(stringResource(R.string.buttons_discard), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDiscard = false }) { Text(stringResource(R.string.mobile_compose_keep_editing)) }
            },
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.composer_actions_new_message)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.buttons_discard))
                    }
                },
                actions = {
                    IconButton(onClick = onAttach) {
                        Icon(Icons.Filled.AttachFile, contentDescription = stringResource(R.string.composer_actions_attach_files))
                    }
                    IconButton(onClick = onSend, enabled = to.isNotBlank()) {
                        Icon(Icons.AutoMirrored.Filled.Send, contentDescription = stringResource(R.string.buttons_send), tint = MaterialTheme.colorScheme.primary)
                    }
                    Box {
                        IconButton(onClick = { overflowOpen = true }) {
                            Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.common_more))
                        }
                        DropdownMenu(expanded = overflowOpen, onDismissRequest = { overflowOpen = false }) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.composer_actions_save_draft)) },
                                leadingIcon = { Icon(Icons.Outlined.Drafts, contentDescription = null) },
                                onClick = {
                                    overflowOpen = false
                                    onSaveDraft()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.chat_actions_discard_draft), color = MaterialTheme.colorScheme.error) },
                                leadingIcon = { Icon(Icons.Filled.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.error) },
                                onClick = {
                                    overflowOpen = false
                                    if (hasContent) confirmDiscard = true else onDiscardDraft()
                                },
                            )
                        }
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
            Row(verticalAlignment = Alignment.CenterVertically) {
                ComposeField(
                    value = to,
                    onChange = onToChange,
                    label = stringResource(R.string.composer_fields_to),
                    field = "to",
                    suggestions = if (recipientSuggestionField == "to") recipientSuggestions else emptyList(),
                    onFocus = onRecipientFocus,
                    onAcceptSuggestion = onAcceptRecipientSuggestion,
                    modifier = Modifier.weight(1f),
                )
                if (!showCcBcc) {
                    TextButton(onClick = { showCcBcc = true }) { Text(stringResource(R.string.composer_actions_cc_bcc)) }
                }
            }
            if (showCcBcc) {
                ComposeField(
                    value = cc,
                    onChange = onCcChange,
                    label = stringResource(R.string.composer_fields_cc),
                    field = "cc",
                    suggestions = if (recipientSuggestionField == "cc") recipientSuggestions else emptyList(),
                    onFocus = onRecipientFocus,
                    onAcceptSuggestion = onAcceptRecipientSuggestion,
                )
                ComposeField(
                    value = bcc,
                    onChange = onBccChange,
                    label = stringResource(R.string.composer_fields_bcc),
                    field = "bcc",
                    suggestions = if (recipientSuggestionField == "bcc") recipientSuggestions else emptyList(),
                    onFocus = onRecipientFocus,
                    onAcceptSuggestion = onAcceptRecipientSuggestion,
                )
            }
            ComposeField(subject, onSubjectChange, stringResource(R.string.composer_fields_subject))
            OutlinedTextField(
                value = body,
                onValueChange = onBodyChange,
                placeholder = { Text(stringResource(R.string.composer_placeholders_message)) },
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
                TextButton(onClick = onClearAttachments) { Text(stringResource(R.string.mobile_compose_clear_attachments)) }
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
                Text(stringResource(R.string.composer_fields_from), fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(
                    selected?.let { formatSendIdentity(it) } ?: stringResource(R.string.mobile_compose_select_sender),
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
    modifier: Modifier = Modifier,
) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
    onConnectGoogleDeviceAccount: () -> Unit,
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
    var showOAuthAdvanced by remember { mutableStateOf(false) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.accounts_actions_add_account)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.buttons_back))
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
                    listOf(
                        stringResource(R.string.accounts_setup_password_tab),
                        stringResource(R.string.accounts_setup_oauth_tab),
                        stringResource(R.string.accounts_setup_rss_tab),
                    ).forEachIndexed { index, label ->
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
                        SetupCard(title = stringResource(R.string.accounts_setup_imap_smtp_account)) {
                            SetupField(displayName, onDisplayNameChange, stringResource(R.string.accounts_fields_display_name_meron_only), placeholder = "Work")
                            SetupField(senderName, onSenderNameChange, stringResource(R.string.accounts_fields_sender_name_outgoing), placeholder = "Jane Doe")
                            SetupField(email, onEmailChange, stringResource(R.string.accounts_fields_email_address), placeholder = "you@example.com")
                            SetupField(username, onUsernameChange, stringResource(R.string.accounts_fields_username), placeholder = "you@example.com")
                            SetupField(password, onPasswordChange, stringResource(R.string.accounts_fields_password), isPassword = true)
                            OutlinedButton(onClick = onAutodiscover, modifier = Modifier.fillMaxWidth()) {
                                Text(stringResource(R.string.mobile_accounts_find_mail_settings))
                            }
                            SetupField(host, onHostChange, stringResource(R.string.accounts_fields_imap_host), placeholder = "imap.example.com")
                            SetupField(imapPort, onImapPortChange, stringResource(R.string.accounts_fields_imap_port), placeholder = "993")
                            SetupField(smtpHost, onSmtpHostChange, stringResource(R.string.accounts_fields_smtp_host), placeholder = "smtp.example.com")
                            SetupField(smtpPort, onSmtpPortChange, stringResource(R.string.accounts_fields_smtp_port), placeholder = "465")
                            Button(onClick = onAddPassword, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.accounts_actions_add_account)) }
                        }
                    }
                }

                1 -> {
                    item {
                        SetupCard(title = if (oauthProvider == "gmail") stringResource(R.string.accounts_providers_gmail_description) else stringResource(R.string.accounts_oauth_outlook_account)) {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                FilterChip(oauthProvider == "gmail", { onOauthProviderChange("gmail") }, { Text(stringResource(R.string.accounts_providers_gmail_name)) })
                                FilterChip(oauthProvider == "outlook", { onOauthProviderChange("outlook") }, { Text(stringResource(R.string.accounts_providers_outlook_name)) })
                            }
                            SetupField(oauthEmail, onOauthEmailChange, stringResource(R.string.accounts_fields_email_address), placeholder = "you@gmail.com")
                            Button(
                                onClick = if (oauthProvider == "gmail") onConnectGoogleDeviceAccount else onLaunchOAuth,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text(
                                    stringResource(
                                        R.string.accounts_oauth_sign_in_with_provider,
                                        if (oauthProvider == "gmail") {
                                            stringResource(R.string.accounts_providers_google_name)
                                        } else {
                                            stringResource(R.string.accounts_providers_outlook_name)
                                        },
                                    ),
                                )
                            }
                            if (oauthAuthorizationCode.isNotBlank()) {
                                Text(
                                    stringResource(R.string.accounts_oauth_finishing_sign_in),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.primary,
                                )
                            }
                            TextButton(onClick = { showOAuthAdvanced = !showOAuthAdvanced }) {
                                Text(if (showOAuthAdvanced) stringResource(R.string.accounts_oauth_hide_advanced_options) else stringResource(R.string.accounts_oauth_advanced_options))
                            }
                            if (showOAuthAdvanced) {
                                SetupField(oauthClientId, onOauthClientIdChange, stringResource(R.string.mobile_accounts_oauth_client_id))
                                SetupField(oauthClientSecret, onOauthClientSecretChange, stringResource(R.string.mobile_accounts_oauth_client_secret_optional))
                                SetupField(oauthRedirectUri, onOauthRedirectUriChange, stringResource(R.string.mobile_accounts_redirect_uri))
                                if (oauthAuthorizationCode.isNotBlank()) {
                                    Button(onClick = onExchangeOAuth, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.accounts_oauth_finish_sign_in)) }
                                }
                                HorizontalDivider()
                                Text(stringResource(R.string.accounts_oauth_paste_tokens_manually), style = MaterialTheme.typography.labelLarge)
                                SetupField(oauthAccessToken, onOauthAccessTokenChange, stringResource(R.string.mobile_accounts_access_token))
                                SetupField(oauthRefreshToken, onOauthRefreshTokenChange, stringResource(R.string.mobile_accounts_refresh_token))
                                SetupField(oauthExpiresAt, onOauthExpiresAtChange, stringResource(R.string.mobile_accounts_token_expires_at))
                                Button(onClick = onAddOAuth, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.accounts_oauth_add_with_tokens)) }
                            }
                        }
                    }
                }

                else -> {
                    item {
                        SetupCard(title = stringResource(R.string.feeds_fallback_name)) {
                            SetupField(rssFeedUrl, onRssFeedUrlChange, stringResource(R.string.feeds_url), placeholder = "https://example.com/feed.xml")
                            SetupField(rssDisplayName, onRssDisplayNameChange, stringResource(R.string.accounts_rss_feed_name), placeholder = "Example Feed")
                            Button(onClick = onAddRss, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.feeds_actions_add_feed)) }
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
